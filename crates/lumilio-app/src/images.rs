//! The HTTP client GPUI uses to fetch images by address: project icons, gallery
//! pictures and pictures inside descriptions.
//!
//! GPUI ships no client of its own on desktop, so `img("https://…")` would show
//! nothing. This one runs the request on a small tokio runtime (reqwest needs
//! one) and hands the bytes back; it only supports plain GET, which is all an
//! image needs.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use gpui_kit::http_client::{
    AsyncBody, HttpClient, Request, Response, Result, Url, anyhow, http::HeaderValue,
};
use lumilio_core::USER_AGENT;
use tokio::runtime::{Builder, Runtime};

pub struct ImageClient {
    runtime: Runtime,
    client: reqwest::Client,
    user_agent: HeaderValue,
}

impl ImageClient {
    pub fn new() -> Result<Arc<Self>, String> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("lumilio-images")
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let client = {
            let _guard = runtime.enter();
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|error| error.to_string())?
        };
        Ok(Arc::new(Self {
            runtime,
            client,
            user_agent: HeaderValue::from_static(USER_AGENT),
        }))
    }
}

type Answer = Pin<Box<dyn Future<Output = Result<Response<AsyncBody>>> + Send + 'static>>;

impl HttpClient for ImageClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        Some(&self.user_agent)
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(&self, request: Request<AsyncBody>) -> Answer {
        let client = self.client.clone();
        let url = request.uri().to_string();
        let work = self.runtime.spawn(async move {
            let response = client.get(&url).send().await?;
            let status = response.status().as_u16();
            let bytes = response.bytes().await?;
            Ok::<_, reqwest::Error>((status, bytes.to_vec()))
        });
        Box::pin(async move {
            let (status, bytes) = work
                .await
                .map_err(|error| anyhow!("image request stopped: {error}"))?
                .map_err(|error| anyhow!("image request failed: {error}"))?;
            Ok(Response::builder()
                .status(status)
                .body(AsyncBody::from(bytes))?)
        })
    }
}

#[cfg(test)]
mod tests {
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
}
