//! Check actual network blocking, including decrypted HTTPS and reused connections.
use bytes::Bytes;
use http_body_util::Full;
use quena_app_core::rules::{BreakpointState, Rules};
use quena_app_core::{AppCore, Paths};
use std::convert::Infallible;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

fn origin(dir: &std::path::Path, tls: bool) -> (u16, mpsc::Receiver<String>) {
    let ca = quena_tls::CertAuthority::load_or_create(dir.join("origin-ca")).unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(ca.server_config("localhost", true).unwrap());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let acceptor = acceptor.clone();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let app = hyper::service::service_fn(move |req: http::Request<hyper::body::Incoming>| {
                        let path = req.uri().path().to_string();
                        tx.send(path.clone()).unwrap();
                        async move { Ok::<_, Infallible>(http::Response::new(Full::new(Bytes::from(path)))) }
                    });
                    if tls {
                        let stream = acceptor.accept(socket).await.unwrap();
                        let _ =
                            hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new()).serve_connection(hyper_util::rt::TokioIo::new(stream), app).await;
                    } else {
                        let _ = hyper::server::conn::http1::Builder::new().serve_connection(hyper_util::rt::TokioIo::new(socket), app).await;
                    }
                });
            }
        });
    });
    (port, rx)
}

fn wait_pause(rules: &Rules, phase: &str, path: &str) -> u64 {
    let start = Instant::now();
    loop {
        if let Some(p) = rules.paused().into_iter().find(|p| p.phase == phase && p.url.ends_with(path)) {
            return p.id;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no {phase} breakpoint for {path}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn check_blocking(tls: bool, h2: bool) {
    let dir = tempfile::tempdir().unwrap();
    let (port, requests) = origin(dir.path(), tls);
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"proxy":{"port":0,"actAsSystemProxy":false,"captureOnStartup":false,"useSystemUpstream":false},"https":{"decrypt":true,"ignoreCertErrors":true}}"#,
    )
    .unwrap();
    let core = AppCore::new(Paths::at(dir.path().to_path_buf()), quena_app_core::logbuf::LogBuffer::new(100)).unwrap();
    let engine = quena_app_core::engine::ProxyEngine::new(&core).unwrap();
    core.set_proxy_engine(engine.clone());
    core.start_capture().unwrap();
    let addr = engine.proxy.listen_addrs().into_iter().find(|a| a.is_ipv4()).unwrap();
    let rules = core.rules.clone().unwrap();
    let base = format!("{}://localhost:{port}", if tls { "https" } else { "http" });

    for phase in ["request", "response"] {
        rules.set_breakpoints(BreakpointState { all_requests: phase == "request", all_responses: phase == "response", ..Default::default() });
        // One curl process requests two URLs: the second reuses the existing proxy/TLS connection.
        // Capture stdout immediately, including headers, so a completed-client assertion cannot
        // accidentally pass when response headers have already leaked through the breakpoint.
        let mut child = Command::new("curl")
            .args([
                "-sS",
                if h2 { "--http2" } else { "--http1.1" },
                "--include",
                "--suppress-connect-headers",
                "--no-buffer",
                "--noproxy",
                "",
                "--max-time",
                "20",
                "--proxy",
                &format!("http://{addr}"),
                "--cacert",
                &engine.ca_info().path,
                &format!("{base}/first"),
                &format!("{base}/second"),
            ])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let output = Arc::new(Mutex::new(Vec::new()));
        let buffer = output.clone();
        let reader = std::thread::spawn(move || {
            let mut chunk = [0; 4096];
            loop {
                let n = stdout.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                buffer.lock().unwrap().extend_from_slice(&chunk[..n]);
            }
        });
        let mut ids = Vec::new();
        for (index, path) in ["/first", "/second"].into_iter().enumerate() {
            let id = wait_pause(&rules, phase, path);
            ids.push(id);
            if phase == "response" {
                assert_eq!(requests.recv_timeout(Duration::from_secs(2)).unwrap(), path);
            }
            // Stay paused over a real interval; neither headers nor body may reach the client.
            std::thread::sleep(Duration::from_millis(200));
            if phase == "request" {
                assert!(matches!(requests.try_recv(), Err(mpsc::TryRecvError::Empty)), "request reached origin before resume");
            }
            let bytes = output.lock().unwrap().clone();
            let text = String::from_utf8_lossy(&bytes);
            assert_eq!(text.lines().filter(|line| line.starts_with("HTTP/")).count(), index, "response headers reached client before resume: {text}");
            assert!(!text.contains(path), "response body reached client before resume");
            assert_eq!(rules.go_all(), 1);
            if phase == "request" {
                assert_eq!(requests.recv_timeout(Duration::from_secs(2)).unwrap(), path);
            }
        }
        assert!(child.wait().unwrap().success());
        reader.join().unwrap();
        let output = output.lock().unwrap();
        let text = String::from_utf8_lossy(&output);
        assert!(text.contains("/first") && text.contains("/second"), "{text}");
        if tls {
            let first = core.capture().detail(ids[0]).unwrap();
            let detail = core.capture().detail(ids[1]).unwrap();
            assert!(detail.summary.has_flag(quena_model::flags::DECRYPTED));
            assert_eq!(detail.request.version, if h2 { quena_model::HttpVersion::Http2 } else { quena_model::HttpVersion::Http11 });
            assert!(first.connection.client_conn_id.is_some());
            assert_eq!(first.connection.client_conn_id, detail.connection.client_conn_id, "second request did not reuse decrypted tunnel");
        }
    }

    // Turning the mode off must leave subsequent traffic free to complete.
    rules.set_breakpoints(BreakpointState::default());
    let result = Command::new("curl")
        .args(["-sS", "--noproxy", "", "--max-time", "5", "--proxy", &format!("http://{addr}"), "--cacert", &engine.ca_info().path, &format!("{base}/off")])
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(result.stdout, b"/off");
    assert!(rules.paused().is_empty());
    core.shutdown();
}

#[test]
fn http_breakpoints_hold_requests_and_response_headers_until_resumed() {
    check_blocking(false, false);
}

#[test]
fn decrypted_https_breakpoints_hold_on_existing_tunnels_too() {
    check_blocking(true, false);
}

#[test]
fn http2_breakpoints_hold_on_existing_decrypted_connections() {
    if !Command::new("curl").arg("-V").output().map(|o| String::from_utf8_lossy(&o.stdout).contains("HTTP2")).unwrap_or(false) {
        eprintln!("curl has no HTTP/2 support; skipping HTTP/2 breakpoint check");
        return;
    }
    check_blocking(true, true);
}
