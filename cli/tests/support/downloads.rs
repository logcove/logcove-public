use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
    time::Duration,
};

#[derive(Clone, Default)]
pub struct Stats {
    pub started: Vec<usize>,
    pub finished: Vec<usize>,
    pub active: usize,
    pub peak: usize,
    pub connections: usize,
}

#[derive(Default)]
pub struct Monitor {
    stats: Mutex<Stats>,
    changed: Condvar,
}

impl Monitor {
    pub fn snapshot(&self) -> Stats {
        self.stats.lock().unwrap().clone()
    }

    pub fn wait_for(&self, ready: impl Fn(&Stats) -> bool) {
        let (stats, _) = self
            .changed
            .wait_timeout_while(self.stats.lock().unwrap(), Duration::from_secs(5), |s| {
                !ready(s)
            })
            .unwrap();
        assert!(
            ready(&stats),
            "Expected concurrent download activity did not occur"
        );
    }

    fn change(&self, update: impl FnOnce(&mut Stats)) {
        update(&mut self.stats.lock().unwrap());
        self.changed.notify_all();
    }
}

pub struct Storage {
    pub origin: reqwest::Url,
    pub monitor: Arc<Monitor>,
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
}

impl Storage {
    pub fn start(
        handler: impl Fn(usize, usize, &Monitor) -> (u16, String) + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin =
            reqwest::Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let monitor = Arc::new(Monitor::default());
        let stop = Arc::new(AtomicBool::new(false));
        let state = monitor.clone();
        let stopped = stop.clone();
        let handle = thread::spawn(move || {
            thread::scope(|scope| {
                let handler = &handler;
                while !stopped.load(Ordering::Relaxed) {
                    let socket = match listener.accept() {
                        Ok((socket, _)) => socket,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(e) => panic!("Storage accept failed: {e}"),
                    };
                    state.change(|s| s.connections += 1);
                    let state = &state;
                    scope.spawn(move || {
                    socket.set_nonblocking(false).unwrap();
                    socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                    let mut reader = BufReader::new(socket);
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap() == 0 { break; }
                        let path = line.split_whitespace().nth(1).unwrap();
                        let (index, signature) = path.trim_start_matches('/').split_once('?').unwrap();
                        assert_eq!(signature, "signature=private-signature");
                        let index: usize = index.parse().unwrap();
                        loop {
                            line.clear();
                            assert!(reader.read_line(&mut line).unwrap() > 0);
                            if line == "\r\n" { break; }
                            let name = line.split_once(':').unwrap().0.to_ascii_lowercase();
                            assert!(!["authorization", "cookie", "origin"].contains(&name.as_str()));
                        }
                        let mut attempt = 0;
                        state.change(|s| {
                            s.started.push(index);
                            attempt = s.started.iter().filter(|i| **i == index).count();
                            s.active += 1;
                            s.peak = s.peak.max(s.active);
                        });
                        let (status, body) = handler(index, attempt, state);
                        state.change(|s| { s.active -= 1; s.finished.push(index); });
                        write!(reader.get_mut(), "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nETag: \"file-etag\"\r\nConnection: keep-alive\r\n\r\n{body}", body.len()).unwrap();
                        reader.get_mut().flush().unwrap();
                    }
                });
                }
            })
        });
        Self {
            origin,
            monitor,
            stop,
            handle,
        }
    }

    pub fn finish(self) -> Stats {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.join().unwrap();
        self.monitor.snapshot()
    }
}
