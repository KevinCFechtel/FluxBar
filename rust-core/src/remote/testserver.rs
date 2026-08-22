//! Fake Miniflux HTTP server for adapter tests (`#[cfg(test)]` only).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::thread;

pub struct FakeServer {
    pub base_url: String,
    requests: Receiver<RecordedRequest>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        // Unblock a worker waiting on the final accept() so join() returns.
        if let Some(worker) = self.worker.take() {
            let url = self.base_url.replacen("http://", "", 1);
            if let Ok(mut stream) = std::net::TcpStream::connect(url) {
                let _ = stream.write_all(b"QUIT\r\n\r\n");
                let _ = stream.flush();
            }
            let _ = worker.join();
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub auth_token: Option<String>,
}

impl FakeServer {
    /// Serves `(status, body)` responses in order, one per connection.
    pub fn start(responses: Vec<(u16, String)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake server");
        let port = listener.local_addr().unwrap().port();
        let (sender, receiver) = channel();
        let worker = thread::spawn(move || {
            'serve: for (status, body) in responses {
                let Ok((stream, _)) = listener.accept() else {
                    break;
                };
                if stream.peer_addr().is_err() {
                    continue 'serve;
                }
                if let Some(request) = serve_connection(stream, status, &body) {
                    if sender.send(request).is_err() {
                        break;
                    }
                }
            }
        });
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            requests: receiver,
            worker: Some(worker),
        }
    }

    pub fn next_request(&self) -> RecordedRequest {
        self.requests
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("recorded request")
    }
}

fn serve_connection(
    stream: std::net::TcpStream,
    status: u16,
    body: &str,
) -> Option<RecordedRequest> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();

    let mut content_length = 0usize;
    let mut auth_token: Option<String> = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("X-Auth-Token:") {
            auth_token = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    if content_length > 0 {
        let mut sink = vec![0u8; content_length];
        reader.read_exact(&mut sink).ok()?;
    }

    let mut stream = stream;
    let response = format!(
        "HTTP/1.1 {status} TEST\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).ok()?;
    stream.flush().ok()?;
    Some(RecordedRequest {
        method,
        path,
        auth_token,
    })
}
