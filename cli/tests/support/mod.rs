#![allow(dead_code)]

use logcove::{
    auth::Clock,
    credentials::CredentialStore,
    error::{Error, Result},
};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Default)]
pub struct MemoryStore(pub Rc<RefCell<Stored>>);

#[derive(Default)]
pub struct Stored {
    pub token: Option<String>,
    pub saves: usize,
    pub fail_save: bool,
    pub fail_delete: bool,
}

impl MemoryStore {
    pub fn with_token(token: &str) -> Self {
        Self(Rc::new(RefCell::new(Stored {
            token: Some(token.into()),
            ..Default::default()
        })))
    }
}

impl CredentialStore for MemoryStore {
    fn read(&self) -> Result<Option<String>> {
        Ok(self.0.borrow().token.clone())
    }
    fn save(&self, token: &str) -> Result<()> {
        let mut state = self.0.borrow_mut();
        if state.fail_save {
            return Err(Error::new("CREDENTIAL_STORE_ERROR", "Mock save failed"));
        }
        state.token = Some(token.into());
        state.saves += 1;
        Ok(())
    }
    fn delete(&self) -> Result<()> {
        let mut state = self.0.borrow_mut();
        if state.fail_delete {
            return Err(Error::new("CREDENTIAL_STORE_ERROR", "Mock delete failed"));
        }
        state.token = None;
        Ok(())
    }
}

#[derive(Default)]
pub struct FakeClock {
    pub elapsed: Duration,
    pub waits: Vec<u64>,
}

impl Clock for FakeClock {
    fn elapsed(&self) -> Duration {
        self.elapsed
    }
    fn sleep(&mut self, duration: Duration) {
        self.waits.push(duration.as_secs());
        self.elapsed += duration;
    }
}

pub struct Step {
    pub method: &'static str,
    pub path: String,
    pub token: Option<&'static str>,
    pub body: Option<Value>,
    pub status: u16,
    pub response: String,
    pub headers: Vec<(String, String)>,
}

impl Step {
    pub fn json(
        method: &'static str,
        path: &str,
        token: Option<&'static str>,
        status: u16,
        response: Value,
    ) -> Self {
        Self {
            method,
            path: path.into(),
            token,
            body: None,
            status,
            response: response.to_string(),
            headers: vec![],
        }
    }
    pub fn body(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

pub struct Server {
    pub origin: reqwest::Url,
    handle: thread::JoinHandle<()>,
}

pub struct Directory(pub std::path::PathBuf);

impl Default for Directory {
    fn default() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "logcove-case-{}-{id}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Directory {
    pub fn write(&self, name: &str, content: &str) -> std::path::PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Server {
    pub fn start(steps: Vec<Step>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin =
            reqwest::Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let handle = thread::spawn(move || {
            for step in steps {
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            thread::sleep(Duration::from_millis(5))
                        }
                        Err(e) => panic!("Expected HTTP request was not received: {e}"),
                    }
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert_eq!(
                    line.trim(),
                    format!("{} {} HTTP/1.1", step.method, step.path)
                );
                let mut headers = HashMap::new();
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    let (name, value) = line.split_once(':').unwrap();
                    headers.insert(name.to_lowercase(), value.trim().to_owned());
                }
                let expected_auth = step.token.map(|token| format!("Bearer {token}"));
                assert!(
                    headers.get("authorization") == expected_auth.as_ref(),
                    "Unexpected Authorization header"
                );
                assert!(!headers.contains_key("cookie"));
                assert!(!headers.contains_key("origin"));
                let size: usize = headers
                    .get("content-length")
                    .map(|v| v.parse().unwrap())
                    .unwrap_or(0);
                let mut body = vec![0; size];
                reader.read_exact(&mut body).unwrap();
                if let Some(expected) = step.body {
                    assert!(
                        serde_json::from_slice::<Value>(&body).unwrap() == expected,
                        "Unexpected request JSON"
                    );
                }
                write!(socket, "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n", step.status, step.response.len()).unwrap();
                for (name, value) in step.headers {
                    write!(socket, "{name}: {value}\r\n").unwrap();
                }
                write!(socket, "\r\n{}", step.response).unwrap();
                socket.flush().unwrap();
            }
        });
        Self { origin, handle }
    }
    pub fn finish(self) {
        self.handle.join().unwrap();
    }
}
