use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct WebChild(Child);

impl Drop for WebChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn analytics_endpoint_returns_json_summary_and_distributions() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();

    let exe = env!("CARGO_BIN_EXE_sessionwiki");

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);

    let mut child = WebChild(
        Command::new(exe)
            .env("SESSIONWIKI_DATA", &data_dir)
            .args(["web", "--no-open", "--port", &address.port().to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stream = loop {
        assert!(child.0.try_wait().unwrap().is_none(), "web server exited unexpectedly");
        if let Ok(stream) = TcpStream::connect(address) {
            break stream;
        }
        assert!(Instant::now() < deadline, "web server did not start in time");
        std::thread::sleep(Duration::from_millis(20));
    };

    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();

    write!(
        stream,
        "GET /api/analytics HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.starts_with("HTTP/1.1 200 "), "expected 200 OK, got: {response}");
    let (_, body) = response.split_once("\r\n\r\n").expect("HTTP headers separator");
    let json: serde_json::Value = serde_json::from_str(body).expect("valid JSON response");

    assert!(json.get("summary").is_some(), "response must contain summary");
    assert!(json.get("daily").is_some(), "response must contain daily");
    assert!(json.get("tools").is_some(), "response must contain tools");
    assert!(json.get("hourly").is_some(), "response must contain hourly");
    assert!(json.get("top_files").is_some(), "response must contain top_files");

    let summary = &json["summary"];
    assert!(summary.get("total_sessions").is_some());
    assert!(summary.get("total_messages").is_some());
    assert!(summary.get("active_days").is_some());
}
