use std::io::{Read, Write};
use std::net::TcpListener;
use std::task::{Context, Poll, Wake, Waker};

use super::*;

struct Ping(std::sync::mpsc::Sender<()>);
impl Wake for Ping {
    fn wake(self: Arc<Self>) {
        let _ = self.0.send(());
    }
}

/// Drives a future without a runtime, the way GPUI's executor would.
fn block_on<T>(future: impl Future<Output = T>) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    let waker = Waker::from(Arc::new(Ping(tx)));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    }
}

fn serve_once(reply: &'static [u8]) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/icon.png", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut seen = [0_u8; 2048];
        let read = stream.read(&mut seen).unwrap();
        stream.write_all(reply).unwrap();
        String::from_utf8_lossy(&seen[..read]).into_owned()
    });
    (address, handle)
}

#[test]
fn fetches_the_bytes_and_identifies_the_launcher() {
    let (address, server) = serve_once(
        b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\ncontent-type: image/png\r\nconnection: close\r\n\r\nhello",
    );
    let client = ImageClient::new().unwrap();
    let response = block_on(client.get(&address, AsyncBody::empty(), true)).unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let mut body = response.into_body();
    let mut bytes = Vec::new();
    block_on(async {
        use futures::AsyncReadExt as _;
        body.read_to_end(&mut bytes).await.unwrap();
    });
    assert_eq!(bytes, b"hello");
    let request = server.join().unwrap().to_lowercase();
    assert!(request.contains("user-agent: lumiliocl/"), "{request}");
}

#[test]
fn a_missing_image_is_a_status_not_a_crash() {
    let (address, _server) =
        serve_once(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
    let client = ImageClient::new().unwrap();
    let response = block_on(client.get(&address, AsyncBody::empty(), true)).unwrap();
    assert_eq!(response.status().as_u16(), 404);
}
