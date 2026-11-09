//! Saved AND/OR rules must hold actual traffic, not just highlight matching rows.
use quena_app_core::rules::{BreakpointPhase, BreakpointRule, BreakpointState, Rules};
use quena_app_core::{AppCore, Paths};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn rule(id: &str) -> BreakpointRule {
    BreakpointRule { id: id.into(), name: "phone API".into(), enabled: true, ..Default::default() }
}

#[test]
fn persists_validated_rules_and_keeps_temporary_breakpoints_independent() {
    let dir = tempfile::tempdir().unwrap();
    let rules = Rules::new(dir.path());
    rules.set_breakpoints(BreakpointState { all_requests: true, method: Some("GET".into()), ..Default::default() });
    let mut r = rule("one");
    r.client_ip = " ::ffff:192.168.1.10 ".into();
    r.host = " *.Example.COM:8080 ".into();
    r.method = " post ".into();
    rules.set_breakpoint_rules(vec![r.clone()], false).unwrap();
    assert!(rules.breakpoints().all_requests);
    let saved = rules.breakpoints().rules;
    assert_eq!((&saved[0].client_ip, &saved[0].host, &saved[0].method), (&"192.168.1.10".to_string(), &"*.example.com:8080".to_string(), &"POST".to_string()));
    let loaded = Rules::new(dir.path()).breakpoints();
    assert_eq!(loaded.rules, saved);
    assert!(!loaded.all_requests && loaded.method.is_none(), "temporary breakpoints are not saved");
    rules.set_breakpoint_rules(saved.clone(), true).unwrap();
    assert_eq!(rules.breakpoints(), BreakpointState { rules: saved, ..Default::default() });

    let path = dir.path().join("breakpoints.json");
    let file_before = std::fs::read(&path).unwrap();
    let state_before = rules.breakpoints();
    let mut invalid = vec![vec![rule("")], vec![rule("same"), rule("same")]];
    for (field, value) in [("ip", "192.168.1.10:8888"), ("host", "https://example.com/api"), ("method", "GET POST")] {
        let mut bad = rule("bad");
        match field { "ip" => bad.client_ip = value.into(), "host" => bad.host = value.into(), _ => bad.method = value.into() }
        invalid.push(vec![bad]);
    }
    let mut bad = rule("bad");
    bad.status = Some(999);
    bad.phase = BreakpointPhase::Response;
    invalid.push(vec![bad]);
    let mut bad = rule("bad");
    bad.status = Some(200);
    invalid.push(vec![bad]);
    for bad in invalid {
        assert!(rules.set_breakpoint_rules(bad, true).is_err());
        assert_eq!(rules.breakpoints(), state_before);
        assert_eq!(std::fs::read(&path).unwrap(), file_before);
    }
    std::fs::write(&path, b"broken JSON").unwrap();
    assert!(Rules::new(dir.path()).breakpoints().rules.is_empty());
    assert!(std::fs::read_dir(dir.path()).unwrap().flatten().any(|entry| std::fs::read(entry.path()).ok().as_deref() == Some(b"broken JSON")));
}

fn wait_pause(rules: &Rules, phase: &str) -> u64 {
    let start = Instant::now();
    loop {
        if let Some(p) = rules.paused().into_iter().find(|p| p.phase == phase) { return p.id; }
        assert!(start.elapsed() < Duration::from_secs(10), "no {phase} pause");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn and_or_phase_and_status_conditions_block_the_network() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), r#"{"proxy":{"port":0,"actAsSystemProxy":false,"captureOnStartup":false,"useSystemUpstream":false}}"#).unwrap();
    let core = AppCore::new(Paths::at(dir.path().to_path_buf()), quena_app_core::logbuf::LogBuffer::new(100)).unwrap();
    let engine = quena_app_core::engine::ProxyEngine::new(&core).unwrap();
    core.set_proxy_engine(engine.clone());
    core.start_capture().unwrap();
    let proxy = format!("http://{}", engine.proxy.listen_addrs().into_iter().find(|a| a.is_ipv4()).unwrap());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, origin) = mpsc::channel();
    std::thread::spawn(move || {
        for mut socket in listener.incoming().flatten() {
            let mut r = BufReader::new(socket.try_clone().unwrap());
            let mut first = String::new();
            r.read_line(&mut first).unwrap();
            loop { let mut line = String::new(); if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" { break; } }
            tx.send(first.clone()).unwrap();
            let status = if first.contains("/teapot") { "418 Teapot" } else { "200 OK" };
            let _ = write!(socket, "HTTP/1.1 {status}\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
        }
    });
    let rules = core.rules.as_ref().unwrap();
    let mut a = rule("phone");
    a.client_ip = "127.0.0.1".into();
    a.host = format!("127.0.0.1:{port}");
    a.url = "/orders".into();
    a.method = "POST".into();
    let mut b = rule("other");
    b.url = "/alternate".into();
    b.method = "GET".into();
    rules.set_breakpoint_rules(vec![a.clone(), b], true).unwrap();
    let send = |path: &str, method: &str| {
        let mut child = Command::new("curl").args(["-sS", "-i", "--noproxy", "", "--max-time", "15", "--proxy", &proxy, "-X", method, &format!("http://127.0.0.1:{port}{path}")]).stdout(Stdio::piped()).spawn().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = [0; 512];
            while let Ok(n) = stdout.read(&mut buffer) { if n == 0 || tx.send(buffer[..n].to_vec()).is_err() { break; } }
        });
        (child, rx)
    };
    // All four fields must match. A different method or path passes immediately.
    for (path, method) in [("/orders", "GET"), ("/elsewhere", "POST")] {
        let (mut child, _out) = send(path, method);
        origin.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(child.wait().unwrap().success());
        assert!(rules.paused().is_empty());
    }
    // Either rule may match, and the request must not reach the server before Go.
    for (path, method) in [("/orders", "POST"), ("/alternate", "GET")] {
        let (mut child, out) = send(path, method);
        let id = wait_pause(rules, "request");
        std::thread::sleep(Duration::from_millis(60));
        assert!(matches!(origin.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(matches!(out.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(core.edit_session(id, Default::default()).is_err(), "F2 cannot edit a live session");
        assert_eq!(rules.go_all(), 1);
        origin.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(child.wait().unwrap().success());
    }
    // A mismatching client or port, or a disabled rule, must not pause.
    for change in [0, 1, 2] {
        let mut r = a.clone();
        match change { 0 => r.client_ip = "192.0.2.123".into(), 1 => r.host = format!("127.0.0.1:{}", port as u32 + 1), _ => r.enabled = false }
        rules.set_breakpoint_rules(vec![r], false).unwrap();
        let (mut child, _out) = send("/orders", "POST");
        origin.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(child.wait().unwrap().success());
        assert!(rules.paused().is_empty());
    }
    a.phase = BreakpointPhase::Response;
    a.status = Some(418);
    a.url = "".into();
    a.method = "GET".into();
    rules.set_breakpoint_rules(vec![a], false).unwrap();
    let (mut child, _out) = send("/normal", "GET");
    origin.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(child.wait().unwrap().success());
    assert!(rules.paused().is_empty(), "status 200 does not match 418");
    let (mut child, out) = send("/teapot", "GET");
    origin.recv_timeout(Duration::from_secs(5)).unwrap();
    wait_pause(rules, "response");
    std::thread::sleep(Duration::from_millis(60));
    assert!(matches!(out.try_recv(), Err(mpsc::TryRecvError::Empty)), "response headers leaked before Go");
    rules.go_all();
    assert!(child.wait().unwrap().success());
    assert!(!out.recv_timeout(Duration::from_secs(5)).unwrap().is_empty());
    core.shutdown();
}
