//! Strictly decoded text edits at real request and response breakpoints.
use quena_app_core::dto::Part;
use quena_app_core::editing::EDIT_LIMIT;
use quena_app_core::{AppCore, Paths};
use quena_body::Body;
use quena_model::{Headers, HttpVersion, RequestHead, ResponseHead, SessionDetail, SessionKind, SessionState};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn core() -> (Arc<AppCore>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), r#"{"proxy":{"port":0,"actAsSystemProxy":false,"captureOnStartup":false,"useSystemUpstream":false}}"#).unwrap();
    let core = AppCore::new(Paths::at(dir.path().to_path_buf()), quena_app_core::logbuf::LogBuffer::new(100)).unwrap();
    (core, dir)
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(bytes).unwrap();
    gz.finish().unwrap()
}

fn add(core: &AppCore, url: &str, encoding: &str, body: Body) -> u64 {
    let mut headers = Headers::new();
    headers.push("Content-Type", "application/json; charset=utf-8");
    if !encoding.is_empty() { headers.push("Content-Encoding", encoding); }
    headers.push("Content-Length", body.len().to_string());
    let mut d = SessionDetail::default();
    d.summary.kind = SessionKind::Http;
    d.summary.state = SessionState::Done;
    d.request = RequestHead { method: "POST".into(), url: url.into(), version: HttpVersion::Http11, headers: headers.clone() };
    d.response = Some(ResponseHead { status: 200, reason: "OK".into(), version: HttpVersion::Http11, headers });
    core.capture().insert(d, body.clone(), body)
}

#[test]
fn refuses_corrupt_truncated_and_oversized_decoded_input() {
    let (core, _dir) = core();
    let cap = core.capture();
    let mut corrupt = gzip(br#"{"keep":"never accept partial decoded output"}"#);
    let trailer = corrupt.len() - 8;
    corrupt[trailer] ^= 0xff;
    let id = add(&core, "https://example.test/corrupt", "gzip", cap.bodies.store_bytes(&corrupt));
    assert!(core.editable_body(id, Part::Response, None).unwrap_err().to_string().contains("could not decode the complete body"));

    let raw = gzip(&vec![b'a'; EDIT_LIMIT + 1]);
    assert!(raw.len() < 20_000, "limit must be checked after decompression");
    let id = add(&core, "https://example.test/big", "gzip", cap.bodies.store_bytes(&raw));
    assert!(core.editable_body(id, Part::Request, None).unwrap_err().to_string().contains("8 MiB"));

    let mut w = cap.bodies.writer_with_limit(4);
    w.write(b"partial recording").unwrap();
    let id = add(&core, "https://example.test/truncated", "", w.finish());
    assert!(core.editable_body(id, Part::Response, None).unwrap_err().to_string().contains("truncated"));
    let id = add(&core, "https://example.test/unknown", "unsupported", cap.bodies.store_bytes(b"opaque"));
    assert!(core.editable_body(id, Part::Request, None).unwrap_err().to_string().contains("unsupported Content-Encoding"));
}

#[test]
fn edits_gzip_request_and_brotli_response_at_real_breakpoints() {
    use quena_app_core::rules::Resume;
    let (core, dir) = core();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let original_response = r#"{"结果":"修改前的响应"}"#;
    let mut response = Vec::new();
    { let mut w = brotli::CompressorWriter::new(&mut response, 4096, 4, 22); w.write_all(original_response.as_bytes()).unwrap(); }
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut r = BufReader::new(socket);
        let mut head = String::new();
        let mut len = 0;
        loop {
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            if line == "\r\n" || line.is_empty() { break; }
            if let Some((k, v)) = line.split_once(':') && k.eq_ignore_ascii_case("content-length") { len = v.trim().parse::<usize>().unwrap(); }
            head.push_str(&line);
        }
        let mut body = vec![0; len];
        r.read_exact(&mut body).unwrap();
        sent.send((head, body)).unwrap();
        write!(r.get_mut(), "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Encoding: br\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len()).unwrap();
        r.get_mut().write_all(&response).unwrap();
    });
    let engine = quena_app_core::engine::ProxyEngine::new(&core).unwrap();
    core.set_proxy_engine(engine.clone());
    core.start_capture().unwrap();
    let addr = engine.proxy.listen_addrs().into_iter().find(|a| a.is_ipv4()).unwrap();
    let rules = core.rules.as_ref().unwrap();
    rules.update_breakpoints(|b| b.all_requests = true);
    let original = r#"{"订单":"修改前的请求"}"#;
    let file = dir.path().join("request.gz");
    std::fs::write(&file, gzip(original.as_bytes())).unwrap();
    let child = std::process::Command::new("curl").args([
        "-sS", "-i", "--noproxy", "", "--max-time", "15", "--proxy", &format!("http://{addr}"),
        "-H", "Content-Type: application/json; charset=utf-8", "-H", "Content-Encoding: gzip", "--data-binary", &format!("@{}", file.display()), &format!("http://127.0.0.1:{port}/orders"),
    ]).stdout(std::process::Stdio::piped()).spawn().unwrap();
    let paused = |phase: &str| {
        let t = Instant::now();
        loop {
            if let Some(p) = rules.paused().into_iter().find(|p| p.phase == phase) { break p.id; }
            assert!(t.elapsed() < Duration::from_secs(10), "no {phase} breakpoint");
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    let id = paused("request");
    assert_eq!(core.editable_body(id, Part::Request, None).unwrap().text, original);
    assert!(received.try_recv().is_err(), "request went out before resume");
    let text = r#"{"订单":"断点修改后的请求"}"#;
    let resume = |action: &str, body: &str| Resume { action: action.into(), head_text: None, body_text: Some(body.into()), body_charset: Some("UTF-8".into()), body_file: None, status: None };
    rules.resume(id, resume("breakOnResponse", text)).unwrap();
    let (head, body) = received.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(body, text.as_bytes());
    assert!(!head.to_ascii_lowercase().contains("content-encoding:"));
    assert_eq!(paused("response"), id);
    assert_eq!(core.editable_body(id, Part::Response, None).unwrap().text, original_response);
    let reply = r#"{"结果":"断点修改后的响应"}"#;
    rules.resume(id, resume("continue", reply)).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let output = String::from_utf8(out.stdout).unwrap();
    let (headers, body) = output.split_once("\r\n\r\n").unwrap();
    assert!(!headers.to_ascii_lowercase().contains("content-encoding:"));
    assert_eq!(body, reply);
    assert!(headers.to_ascii_lowercase().contains(&format!("content-length: {}", reply.len())));
    core.shutdown();
}
