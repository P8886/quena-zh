//! Continuous Find: traffic captured after the initial job, delayed bodies and lifecycle.

use quena_app_core::find::{FindOptions, FindResult};
use quena_app_core::{AppCore, Paths};
use quena_jobs::JobStatus;
use quena_model::{Headers, MarkColor, RequestHead, ResponseHead, SessionDetail, SessionKind, SessionState};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

struct Fixture {
    core: Arc<AppCore>,
    _dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), r#"{"proxy":{"port":0,"actAsSystemProxy":false,"captureOnStartup":false,"useSystemUpstream":false}}"#).unwrap();
        let core = AppCore::new(Paths::at(dir.path().to_path_buf()), quena_app_core::logbuf::LogBuffer::new(100)).unwrap();
        core.start_ticker();
        Self { core, _dir: dir }
    }

    fn find(&self, options: FindOptions) -> FindResult {
        self.core.capture().index.tick();
        let job = self.core.find_sessions(options).unwrap();
        assert_eq!(self.core.jobs.wait(job, Duration::from_secs(10)).unwrap().status, JobStatus::Done);
        self.core.find_result(job).unwrap()
    }

    fn color(&self, id: u64) -> Option<MarkColor> {
        self.core.capture().index.get(id).unwrap().color
    }

    fn add(&self, url: &str, headers: &[(&str, &str)], body: &[u8]) -> u64 {
        let cap = self.core.capture();
        let detail = SessionDetail {
            request: RequestHead { url: url.into(), method: "GET".into(), ..Default::default() },
            response: Some(ResponseHead { status: 200, headers: Headers(headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()), ..Default::default() }),
            summary: quena_model::SessionSummary { state: SessionState::Done, ..Default::default() },
            ..Default::default()
        };
        cap.insert(detail, cap.bodies.store_bytes(b""), cap.bodies.store_bytes(body))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) { self.core.shutdown(); }
}

fn options(text: &str) -> FindOptions {
    serde_json::from_value(serde_json::json!({"text":text,"scope":"all","examine":"all","mark":"gold","decode":true,"continuous":true})).unwrap()
}

fn until(mut check: impl FnMut() -> bool) {
    let start = Instant::now();
    while !check() {
        assert!(start.elapsed() < Duration::from_secs(10), "continuous find did not settle");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn matches_later_requests_and_preserves_regex_and_case_options() {
    let f = Fixture::new();
    let first = f.add("https://example.test/tracked/1", &[], b"");
    let mut o = options(r"/tracked/\d+$");
    o.scope = "urls".into();
    o.regex = true;
    o.match_case = true;
    assert_eq!(f.find(o).ids, [first]);
    let cap = f.core.capture();
    let next = cap.begin(SessionKind::Http, |d| {
        d.request.url = "https://example.test/tracked/2".into();
        d.request.method = "POST".into();
        d.summary.state = SessionState::RequestHeaders;
    });
    let miss = f.add("https://example.test/TRACKED/3", &[], b"");
    until(|| f.color(next.id) == Some(MarkColor::Gold));
    assert_eq!(f.color(first), Some(MarkColor::Gold));
    assert_eq!(f.color(miss), None);
    assert_eq!(f.core.live_find_status().unwrap().matches, 2);
    // A later update must not undo an explicit user mark on an already matched row.
    f.core.mark(vec![next.id], Some(MarkColor::Blue));
    next.update(|d| d.summary.state = SessionState::Done);
    next.finish();
    until(|| cap.live(next.id).is_none());
    assert_eq!(f.color(next.id), Some(MarkColor::Blue));
}

#[test]
fn finds_a_delayed_response_body_through_the_real_proxy() {
    let f = Fixture::new();
    let mut o = options("late-body-needle");
    o.scope = "responses".into();
    o.examine = "bodies".into();
    assert!(f.find(o).ids.is_empty());
    let engine = quena_app_core::engine::ProxyEngine::new(&f.core).unwrap();
    f.core.set_proxy_engine(engine.clone());
    f.core.start_capture().unwrap();
    let proxy = engine.proxy.listen_addrs().into_iter().find(|a| a.is_ipv4()).unwrap();
    let origin = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin_addr = origin.local_addr().unwrap();
    let (release, hold) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = origin.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut head = Vec::new();
        let mut byte = [0];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nhello\r\n").unwrap();
        stream.flush().unwrap();
        hold.recv_timeout(Duration::from_secs(10)).unwrap();
        let tail = b"late-body-needle";
        write!(stream, "{:x}\r\n", tail.len()).unwrap();
        stream.write_all(tail).unwrap();
        stream.write_all(b"\r\n0\r\n\r\n").unwrap();
    });
    let mut client = TcpStream::connect(proxy).unwrap();
    client.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(client, "GET http://{origin_addr}/stream HTTP/1.1\r\nHost: {origin_addr}\r\nConnection: close\r\n\r\n").unwrap();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        client.read_to_end(&mut bytes).unwrap();
        bytes
    });
    until(|| f.core.capture().index.find_all(|s| s.response_body_len >= 5).len() == 1);
    let id = f.core.capture().index.find_all(|_| true)[0];
    assert_eq!(f.color(id), None, "headers and the first chunk contain no match");
    release.send(()).unwrap();
    until(|| f.color(id) == Some(MarkColor::Gold));
    assert!(String::from_utf8_lossy(&reader.join().unwrap()).contains("late-body-needle"));
    server.join().unwrap();
    assert_eq!(f.core.live_find_status().unwrap().matches, 1);
}

#[test]
fn replacement_stop_and_mcp_searches_have_separate_lifetimes() {
    let f = Fixture::new();
    f.find(options("/first"));
    let mcp = f.core.find_sessions_for(options("/mcp"), "test-client").unwrap();
    assert_eq!(f.core.jobs.wait(mcp, Duration::from_secs(10)).unwrap().status, JobStatus::Done);
    assert_eq!(f.core.live_find_status().unwrap().text, "/first");
    let first = f.add("https://example.test/first", &[], b"");
    let mcp_only = f.add("https://example.test/mcp", &[], b"");
    until(|| f.color(first) == Some(MarkColor::Gold));
    assert_eq!(f.color(mcp_only), None, "MCP searches remain one-shot even with continuous=true");
    let mut next = options("/second");
    next.mark = Some(MarkColor::Purple);
    f.find(next);
    let mut invalid = options("[");
    invalid.regex = true;
    assert!(f.core.find_sessions(invalid).is_err());
    assert_eq!(f.core.live_find_status().unwrap().text, "/second");
    let old = f.add("https://example.test/first-later", &[], b"");
    let second = f.add("https://example.test/second", &[], b"");
    until(|| f.color(second) == Some(MarkColor::Purple));
    assert_eq!(f.color(old), None, "replaced conditions no longer apply");
    assert_eq!(f.color(first), Some(MarkColor::Gold), "existing highlights remain");
    f.core.stop_find();
    let stopped = f.add("https://example.test/second-after-stop", &[], b"");
    assert!(f.core.live_find_status().is_none());
    assert_eq!(f.color(stopped), None);
    assert_eq!(f.color(second), Some(MarkColor::Purple));
}

#[test]
fn clearing_and_switching_captures_keep_conditions_without_reusing_old_matches() {
    let f = Fixture::new();
    f.find(options("/match"));
    let first = f.add("https://example.test/match", &[], b"");
    until(|| f.color(first) == Some(MarkColor::Gold));
    f.core.remove_all();
    let reused = f.add("https://example.test/miss", &[], b"");
    assert_eq!(reused, first);
    let matched = f.add("https://example.test/match", &[], b"");
    until(|| f.color(matched) == Some(MarkColor::Gold));
    assert_eq!(f.color(reused), None, "a cleared match must not colour a reused id");
    assert_eq!(f.core.live_find_status().unwrap().matches, 1);
    let cap = quena_store::Capture::open(f._dir.path().join("replacement"), f.core.settings().bodies.to_config(), true).unwrap();
    f.core.switch_capture(cap);
    let reused = f.add("https://example.test/miss-again", &[], b"");
    let matched = f.add("https://example.test/match-again", &[], b"");
    until(|| f.color(matched) == Some(MarkColor::Gold));
    assert_eq!(f.color(reused), None);
    assert_eq!(f.core.live_find_status().unwrap().matches, 1);
}

#[test]
fn one_shot_selected_only_and_select_only_searches_do_not_follow_new_traffic() {
    let f = Fixture::new();
    let first = f.add("https://example.test/match", &[], b"");
    let mut one_shot = options("/match");
    one_shot.continuous = false;
    assert_eq!(f.find(one_shot).ids, [first]);
    assert!(f.core.live_find_status().is_none());
    let next = f.add("https://example.test/match-later", &[], b"");
    let mut selected = options("/match");
    selected.ids = vec![first];
    selected.mark = Some(MarkColor::Blue);
    assert_eq!(f.find(selected).ids, [first]);
    assert_eq!(f.color(first), Some(MarkColor::Blue));
    assert_eq!(f.color(next), None);
    assert!(f.core.live_find_status().is_none());
    let mut select_only = options("/match");
    select_only.mark = None;
    assert_eq!(f.find(select_only).ids.len(), 2);
    assert!(f.core.live_find_status().is_none());
    assert_eq!(f.color(next), None);
}

#[test]
fn later_bodies_use_the_same_decoding_scope_and_size_limit() {
    let f = Fixture::new();
    let mut o = options("Grüße");
    o.scope = "responses".into();
    o.examine = "bodies".into();
    o.max_body_mb = 1;
    f.find(o);
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all("Grüße aus dem späteren Response".as_bytes()).unwrap();
    let compressed = f.add("https://example.test/gzip", &[("Content-Type", "text/plain; charset=utf-8"), ("Content-Encoding", "gzip")], &gzip.finish().unwrap());
    let latin1 = f.add("https://example.test/latin1", &[("Content-Type", "text/plain; charset=windows-1252")], b"Gr\xfc\xdfe");
    let mut large = vec![b'x'; (1 << 20) + 1];
    large.extend_from_slice("Grüße".as_bytes());
    let skipped = f.add("https://example.test/large", &[("Content-Type", "text/plain")], &large);
    let headers_only = f.add("https://example.test/header", &[("X-Message", "Grüße")], b"no match");
    until(|| f.color(compressed) == Some(MarkColor::Gold) && f.color(latin1) == Some(MarkColor::Gold));
    assert_eq!(f.color(skipped), None);
    assert_eq!(f.color(headers_only), None, "body-only scope does not search headers");
    assert_eq!(f.core.live_find_status().unwrap().matches, 2);
}

#[test]
fn follows_filter_changes_and_drains_more_than_one_batch_of_new_sessions() {
    let f = Fixture::new();
    f.core.set_filters(quena_query::FilterSettings { enabled: true, expression: "host == allowed.test".into(), ..Default::default() }).unwrap();
    f.find(options("/match"));
    let hidden = f.add("https://hidden.test/match", &[], b"");
    let allowed = f.add("https://allowed.test/match", &[], b"");
    until(|| f.color(allowed) == Some(MarkColor::Gold));
    assert_eq!(f.color(hidden), None);
    f.core.set_filters(Default::default()).unwrap();
    until(|| f.color(hidden) == Some(MarkColor::Gold));
    for i in 0..600 {
        f.add(&format!("https://example.test/match/{i}"), &[], b"");
    }
    until(|| f.core.live_find_status().unwrap().matches == 602);
    f.core.capture().index.remove(&std::collections::HashSet::from([hidden]));
    until(|| f.core.live_find_status().unwrap().matches == 601);
}
