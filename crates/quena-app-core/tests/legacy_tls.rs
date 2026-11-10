//! Exercise a CBC-only upstream through the real decrypting proxy. Requires curl,
//! Python's standard ssl module and openssl, like the network breakpoint fixtures.
#![cfg(unix)]

use quena_app_core::dto::Part;
use quena_app_core::engine::ProxyEngine;
use quena_app_core::rules::{BreakpointState, Resume};
use quena_app_core::{AppCore, Paths};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

struct Proxy {
    core: Arc<AppCore>,
    engine: Arc<ProxyEngine>,
    _dir: tempfile::TempDir,
}

impl Proxy {
    fn new(insecure: bool, fallback: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let settings = serde_json::json!({
            "proxy": {"port": 0, "actAsSystemProxy": false, "captureOnStartup": false,
                "useSystemUpstream": false, "useSystemPac": false},
            "https": {"decrypt": true, "ignoreCertErrors": insecure, "legacyTlsFallback": fallback}
        });
        std::fs::write(dir.path().join("settings.json"), settings.to_string()).unwrap();
        let core = AppCore::new(Paths::at(dir.path().to_path_buf()), quena_app_core::logbuf::LogBuffer::new(100)).unwrap();
        let engine = ProxyEngine::new(&core).unwrap();
        core.set_proxy_engine(engine.clone());
        core.start_capture().unwrap();
        Self { core, engine, _dir: dir }
    }

    fn curl(&self, url: &str) -> Command {
        let addr = self.engine.proxy.listen_addrs().into_iter().find(|a| a.is_ipv4()).unwrap();
        let mut cmd = Command::new("curl");
        cmd.args([
            "-sS", "--http1.1", "--noproxy", "", "--max-time", "30",
            "--proxy", &format!("http://{addr}"), "--cacert", &self.engine.ca_info().path, url,
        ]);
        cmd
    }

    fn paused(&self, phase: &str) -> u64 {
        let start = Instant::now();
        loop {
            if let Some(p) = self.core.rules.as_ref().unwrap().paused().into_iter().find(|p| p.phase == phase) {
                return p.id;
            }
            assert!(start.elapsed() < Duration::from_secs(15), "no {phase} breakpoint");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Proxy {
    fn drop(&mut self) { self.core.shutdown(); }
}

struct LegacyOrigin {
    child: Child,
    port: u16,
    requests: mpsc::Receiver<serde_json::Value>,
    _dir: tempfile::TempDir,
}

impl LegacyOrigin {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("cert.pem");
        let key = dir.path().join("key.pem");
        let generated = Command::new("openssl").args([
            "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-subj", "/CN=localhost",
            "-addext", "subjectAltName=DNS:localhost", "-out", cert.to_str().unwrap(), "-keyout", key.to_str().unwrap(),
        ]).output().unwrap();
        assert!(generated.status.success(), "{}", String::from_utf8_lossy(&generated.stderr));
        let mut child = Command::new("python3").args(["-u", "-c", r#"
import http.server, json, ssl, sys
class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', '0'))).decode()
        print(json.dumps({'body': body, 'cipher': self.connection.cipher()[0]}), flush=True)
        reply = b'{"result":"original response"}'
        self.send_response(200)
        self.send_header('Content-Type', 'application/json; charset=utf-8')
        self.send_header('Content-Length', str(len(reply)))
        self.send_header('Connection', 'close')
        self.end_headers()
        self.wfile.write(reply)
    def log_message(self, *args):
        pass
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.minimum_version = context.maximum_version = ssl.TLSVersion.TLSv1_2
context.set_ciphers('ECDHE-RSA-AES128-SHA')
context.set_alpn_protocols(['http/1.1'])
context.load_cert_chain(sys.argv[1], sys.argv[2])
server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
server.socket = context.wrap_socket(server.socket, server_side=True)
print(server.server_port, flush=True)
server.serve_forever()
"#, cert.to_str().unwrap(), key.to_str().unwrap()]).stdout(Stdio::piped()).spawn().unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let port = line.trim().parse().expect("Python TLS fixture did not start");
        let (tx, requests) = mpsc::channel();
        std::thread::spawn(move || {
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if tx.send(serde_json::from_str(&line).unwrap()).is_err() { break; }
            }
        });
        Self { child, port, requests, _dir: dir }
    }

    fn url(&self) -> String { format!("https://localhost:{}/orders", self.port) }
}

impl Drop for LegacyOrigin {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn cbc_fallback_keeps_decryption_and_editable_request_and_response_breakpoints() {
    let origin = LegacyOrigin::new();
    let proxy = Proxy::new(true, false);
    let out = proxy.curl(&origin.url()).args(["-i", "--data-binary", "disabled"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let response = String::from_utf8_lossy(&out.stdout);
    assert!(response.contains("502 Bad Gateway") && response.contains("HandshakeFailure"), "{response}");
    assert!(origin.requests.try_recv().is_err(), "disabled fallback sent an HTTP request");

    let mut settings = proxy.core.settings();
    settings.https.legacy_tls_fallback = true;
    proxy.core.update_settings(settings).unwrap();
    let rules = proxy.core.rules.as_ref().unwrap();
    rules.set_breakpoints(BreakpointState { all_requests: true, ..Default::default() });
    let original = r#"{"order":"original request"}"#;
    let child = proxy.curl(&origin.url()).args([
        "-H", "Content-Type: application/json; charset=utf-8", "--data-binary", original,
    ]).stdout(Stdio::piped()).spawn().unwrap();
    let id = proxy.paused("request");
    assert_eq!(proxy.core.editable_body(id, Part::Request, None).unwrap().text, original);
    assert!(origin.requests.recv_timeout(Duration::from_millis(200)).is_err(), "request escaped its breakpoint");
    let edited = r#"{"order":"断点修改后的请求"}"#;
    let resume = |action: &str, body: &str| Resume {
        action: action.into(), head_text: None, body_text: Some(body.into()),
        body_charset: Some("UTF-8".into()), body_file: None, status: None,
    };
    rules.resume(id, resume("breakOnResponse", edited)).unwrap();
    let sent = origin.requests.recv_timeout(Duration::from_secs(15)).unwrap();
    assert_eq!(sent["body"], edited);
    assert_eq!(sent["cipher"], "ECDHE-RSA-AES128-SHA");
    assert_eq!(proxy.paused("response"), id);
    assert_eq!(proxy.core.editable_body(id, Part::Response, None).unwrap().text, r#"{"result":"original response"}"#);
    let detail = proxy.core.capture().detail(id).unwrap();
    let tls = detail.connection.server_tls.unwrap();
    assert_eq!(tls.version, "TLS 1.2");
    assert!(!tls.server_chain_pem.is_empty(), "server certificate was not recorded");
    assert_eq!(tls.alpn.as_deref(), Some("http/1.1"));
    let reply = r#"{"result":"断点修改后的响应"}"#;
    rules.resume(id, resume("continue", reply)).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap(), reply);
    assert!(origin.requests.recv_timeout(Duration::from_millis(200)).is_err(), "fallback duplicated a POST");
}

#[test]
fn cbc_fallback_does_not_bypass_certificate_verification() {
    let origin = LegacyOrigin::new();
    let proxy = Proxy::new(false, true);
    let out = proxy.curl(&origin.url()).args(["-i", "--data-binary", "untrusted"]).output().unwrap();
    assert!(out.status.success());
    let response = String::from_utf8_lossy(&out.stdout);
    assert!(response.contains("502 Bad Gateway") && response.contains("system TLS compatibility retry failed"), "{response}");
    assert!(origin.requests.try_recv().is_err(), "untrusted origin received an HTTP request");
}

#[test]
#[ignore = "requires access to the public legacy-TLS origin wx.10086.cn"]
fn public_legacy_origin_decrypts_with_certificate_verification() {
    let proxy = Proxy::new(false, true);
    let rules = proxy.core.rules.as_ref().unwrap();
    rules.set_breakpoints(BreakpointState { all_responses: true, ..Default::default() });
    let child = proxy.curl("https://wx.10086.cn/").arg("-I").stdout(Stdio::piped()).spawn().unwrap();
    let id = proxy.paused("response");
    let detail = proxy.core.capture().detail(id).unwrap();
    assert_eq!(detail.response.unwrap().status, 200);
    let tls = detail.connection.server_tls.unwrap();
    assert_eq!(tls.version, "TLS 1.2");
    assert!(!tls.server_chain_pem.is_empty());
    eprintln!("wx.10086.cn: HTTP 200, {}, ALPN {:?}, certificate {:?}", tls.version, tls.alpn, tls.subject);
    rules.resume(id, Resume {
        action: "continue".into(), head_text: None, body_text: None,
        body_charset: None, body_file: None, status: None,
    }).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("200 OK"));
}
