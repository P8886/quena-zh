//! Replay: optional concurrency limits, sequential execution and Stop.

use quena_app_core::compose::ReplayOptions;
use quena_app_core::{AppCore, Paths};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Answers after 80 ms; counts requests and the most at once.
fn slow_server(total: Arc<AtomicUsize>, now: Arc<AtomicUsize>, most: Arc<AtomicUsize>) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (total, now, most) = (total.clone(), now.clone(), most.clone());
            std::thread::spawn(move || {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut failed = false;
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if line.starts_with("GET /fail ") { failed = true; }
                }
                let n = now.fetch_add(1, Ordering::SeqCst) + 1;
                most.fetch_max(n, Ordering::SeqCst);
                total.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(80));
                now.fetch_sub(1, Ordering::SeqCst);
                let mut s = s;
                let status = if failed { "503 Unavailable" } else { "200 OK" };
                let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
            });
        }
    });
    port
}

#[test]
fn replays_in_parallel_one_by_one_and_stop() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), r#"{"proxy":{"port":0,"actAsSystemProxy":false,"captureOnStartup":false,"useSystemUpstream":false}}"#).unwrap();
    let core = AppCore::new(Paths::at(dir.path().to_path_buf()), quena_app_core::logbuf::LogBuffer::new(100)).unwrap();
    let engine = quena_app_core::engine::ProxyEngine::new(&core).unwrap();
    core.set_proxy_engine(engine.clone());
    core.start_capture().unwrap();
    let addr = engine.proxy.listen_addrs().into_iter().find(|a| a.is_ipv4()).unwrap();
    let (total, now, most) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let port = slow_server(total.clone(), now.clone(), most.clone());
    let o = Command::new("curl").args(["-sS", "--max-time", "20", "-x", &format!("http://{addr}"), &format!("http://127.0.0.1:{port}/x")]).output().unwrap();
    assert!(o.status.success());
    let id = core.capture().index.find_all(|_| true)[0];
    let wait_total = |n: usize| {
        let t = Instant::now();
        while total.load(Ordering::SeqCst) < n || now.load(Ordering::SeqCst) > 0 {
            assert!(t.elapsed() < Duration::from_secs(30), "{} of {n}", total.load(Ordering::SeqCst));
            std::thread::sleep(Duration::from_millis(20));
        }
    };

    // 20 repeats, 5 at a time.
    most.store(0, Ordering::SeqCst);
    assert_eq!(core.replay(vec![id], ReplayOptions { count: 20, parallel: 5, ..Default::default() }).unwrap(), 20);
    wait_total(21);
    let m = most.load(Ordering::SeqCst);
    assert!((2..=5).contains(&m), "at most 5 at a time, more than one: {m}");

    // One after the other.
    most.store(0, Ordering::SeqCst);
    core.replay(vec![id], ReplayOptions { count: 4, sequential: true, ..Default::default() }).unwrap();
    wait_total(25);
    assert_eq!(most.load(Ordering::SeqCst), 1);

    // Stop: of 1000 repeats only a few go out.
    core.replay(vec![id], ReplayOptions { count: 1000, parallel: 2, ..Default::default() }).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    core.replay_stop();
    std::thread::sleep(Duration::from_millis(400));
    let sent = total.load(Ordering::SeqCst) - 25;
    assert!(sent < 40, "{sent} sent after Stop");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(total.load(Ordering::SeqCst) - 25, sent, "nothing more after Stop");
    core.shutdown();
}

#[test]
fn unrestricted_repeats_start_more_than_100_requests_before_any_response() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), r#"{"proxy":{"port":0,"actAsSystemProxy":false,"captureOnStartup":false,"useSystemUpstream":false}}"#).unwrap();
    let core = AppCore::new(Paths::at(dir.path().to_path_buf()), quena_app_core::logbuf::LogBuffer::new(100)).unwrap();
    let engine = quena_app_core::engine::ProxyEngine::new(&core).unwrap();
    core.set_proxy_engine(engine.clone());
    core.start_capture().unwrap();
    let addr = engine.proxy.listen_addrs().into_iter().find(|a| a.is_ipv4()).unwrap();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let port = listener.local_addr().unwrap().port();
    let (release, gate) = tokio::sync::watch::channel(true);
    let (received, requests) = std::sync::mpsc::channel();
    let (finished, responses) = std::sync::mpsc::channel();
    rt.spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let (received, finished, mut gate) = (received.clone(), finished.clone(), gate.clone());
            tokio::spawn(async move {
                let mut reader = tokio::io::BufReader::new(socket);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                }
                received.send(()).unwrap();
                while !*gate.borrow_and_update() {
                    gate.changed().await.unwrap();
                }
                reader.get_mut().write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").await.unwrap();
                finished.send(()).unwrap();
            });
        }
    });
    let seed = Command::new("curl")
        .args(["-sS", "--noproxy", "", "--max-time", "20", "-x", &format!("http://{addr}"), &format!("http://127.0.0.1:{port}/x")])
        .output()
        .unwrap();
    assert!(seed.status.success(), "{}", String::from_utf8_lossy(&seed.stderr));
    requests.recv_timeout(Duration::from_secs(5)).unwrap();
    responses.recv_timeout(Duration::from_secs(5)).unwrap();
    let id = core.capture().index.find_all(|_| true)[0];

    // Hold every response. Neither a serial fallback nor a hidden 100-request cap can
    // pass: all 128 repeats must reach the origin before the gate is released.
    release.send_replace(false);
    assert_eq!(core.replay(vec![id], ReplayOptions { count: 128, ..Default::default() }).unwrap(), 128);
    let deadline = Instant::now() + Duration::from_secs(10);
    for i in 0..128 {
        requests.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or_else(|_| panic!("only {i} of 128 repeats arrived before any response"));
    }
    assert!(matches!(responses.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));
    assert!(matches!(requests.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)), "repeat count multiplied by concurrency");
    let batch = &core.replay_batches()[0];
    assert_eq!((batch.planned, batch.submitted, batch.running, batch.completed, batch.peak), (128, 128, 128, 0, 128));
    // Removing a captured row must not turn a successful network request into a
    // batch failure. The progress owns the outcome independently of list retention.
    let cap = core.capture();
    let removed: std::collections::HashSet<_> = cap.index.find_all(|s| s.has_flag(quena_model::flags::REPLAYED)).into_iter().take(2).collect();
    assert_eq!(removed.len(), 2);
    cap.remove(&removed);
    release.send_replace(true);
    for _ in 0..128 {
        responses.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    let t = Instant::now();
    while core.replay_batches()[0].completed != 128 {
        assert!(t.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    let batch = &core.replay_batches()[0];
    assert_eq!((batch.submitted, batch.running, batch.completed, batch.failed), (128, 0, 128, 0));
    assert!(batch.submission_done && batch.requests_per_second > 0.0);
    core.shutdown();
}

#[test]
fn batch_stop_and_failures_are_isolated_from_other_replays_and_normal_capture() {
    use quena_model::{HttpVersion, RequestHead, SessionDetail, SessionKind, SessionState};
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), r#"{"proxy":{"port":0,"actAsSystemProxy":false,"captureOnStartup":false,"useSystemUpstream":false}}"#).unwrap();
    let core = AppCore::new(Paths::at(dir.path().to_path_buf()), quena_app_core::logbuf::LogBuffer::new(100)).unwrap();
    let engine = quena_app_core::engine::ProxyEngine::new(&core).unwrap();
    core.set_proxy_engine(engine.clone());
    core.start_capture().unwrap();
    let addr = engine.proxy.listen_addrs().into_iter().find(|a| a.is_ipv4()).unwrap();
    let port = slow_server(Default::default(), Default::default(), Default::default());
    let add = |path: &str| {
        let mut d = SessionDetail::default();
        d.summary.kind = SessionKind::Http;
        d.summary.state = SessionState::Done;
        d.request = RequestHead { method: "GET".into(), url: format!("http://127.0.0.1:{port}{path}"), version: HttpVersion::Http11, headers: Default::default() };
        core.capture().insert(d, quena_body::Body::empty(), quena_body::Body::empty())
    };
    let (ok, fail) = (add("/ok"), add("/fail"));
    core.replay(vec![ok], ReplayOptions { count: 50, parallel: 1, ..Default::default() }).unwrap();
    let stopped_id = core.replay_batches()[0].id;
    core.replay(vec![fail], ReplayOptions { count: 3, parallel: 2, ..Default::default() }).unwrap();
    let failed_id = core.replay_batches()[0].id;
    let t = Instant::now();
    while core.replay_batches().iter().find(|b| b.id == stopped_id).unwrap().submitted < 2 {
        assert!(t.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(core.replay_stop_batch(stopped_id));
    // This ordinary proxied request must not contribute to either batch's counters.
    let out = Command::new("curl").args(["-sS", "--noproxy", "", "--max-time", "10", "-x", &format!("http://{addr}"), &format!("http://127.0.0.1:{port}/normal")]).output().unwrap();
    assert!(out.status.success());
    let batches = loop {
        let batches = core.replay_batches();
        if batches.iter().all(|b| b.submission_done && b.running == 0) { break batches; }
        assert!(t.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    };
    let stopped = batches.iter().find(|b| b.id == stopped_id).unwrap();
    let failed = batches.iter().find(|b| b.id == failed_id).unwrap();
    assert!(stopped.stopped && stopped.submitted >= 2 && stopped.submitted < 50);
    assert_eq!((stopped.completed, stopped.failed, stopped.peak), (stopped.submitted, 0, 1));
    assert_eq!((failed.planned, failed.submitted, failed.completed, failed.failed, failed.peak), (3, 3, 3, 3, 2));
    assert!(!failed.stopped);
    for b in &batches {
        assert_eq!(b.submitted, b.completed + b.running);
        assert!(b.min_ms.unwrap() <= b.mean_ms && b.mean_ms <= b.max_ms.unwrap());
        assert!(b.elapsed_ms > 0 && b.requests_per_second > 0.0);
    }
    std::thread::sleep(Duration::from_millis(25));
    assert_eq!(core.replay_batches(), batches, "finished timings must freeze");
    core.shutdown();
}
