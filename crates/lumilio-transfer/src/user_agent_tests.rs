use super::USER_AGENT;
use super::http::{FileTransport, HttpTransport};
use super::transport::{HttpMethod, HttpRequest, Transport};
use futures_util::stream::StreamExt;

use std::io::{Read, Write};
use std::net::TcpListener;

#[tokio::test]
async fn http_requests_identify_the_launcher() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0_u8; 4096];
        let read = stream.read(&mut buffer).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
        String::from_utf8_lossy(&buffer[..read]).into_owned()
    });
    let transport = HttpTransport::new().unwrap();
    let response = transport.get(&format!("http://{address}/")).await.unwrap();
    assert_eq!(response.status(), 200);
    let head = server.join().unwrap().to_ascii_lowercase();
    assert!(
        head.contains(&format!("user-agent: {}", USER_AGENT.to_ascii_lowercase())),
        "request head was: {head}"
    );
    assert!(USER_AGENT.starts_with("LumilioCL/"));
}

#[tokio::test]
async fn post_json_sends_the_body_with_a_json_content_type() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        // Headers and body may arrive in separate reads; read the lot.
        let mut received = Vec::new();
        let mut buffer = [0_u8; 4096];
        while !received.ends_with(b"}") {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "connection closed early");
            received.extend_from_slice(&buffer[..read]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
        String::from_utf8_lossy(&received).into_owned()
    });
    let transport = HttpTransport::new().unwrap();
    let response = transport
        .post_json(&format!("http://{address}/x"), br#"{"a":1}"#.to_vec())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let head = server.join().unwrap().to_ascii_lowercase();
    assert!(head.starts_with("post /x "), "{head}");
    assert!(head.contains("content-type: application/json"), "{head}");
    assert!(head.ends_with(r#"{"a":1}"#), "{head}");
}

#[tokio::test]
async fn send_carries_headers_and_body_and_returns_error_statuses_with_their_text() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut received = Vec::new();
        let mut buffer = [0_u8; 4096];
        while !received.ends_with(b"a=1&b=2") {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "connection closed early");
            received.extend_from_slice(&buffer[..read]);
        }
        let body = br#"{"error":"authorization_pending"}"#;
        let head = format!(
            "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        String::from_utf8_lossy(&received).into_owned()
    });
    let transport = HttpTransport::new().unwrap();
    let response = transport
        .send(HttpRequest {
            method: HttpMethod::Post,
            url: format!("http://{address}/token"),
            headers: vec![
                (
                    "content-type".into(),
                    "application/x-www-form-urlencoded".into(),
                ),
                ("authorization".into(), "Bearer abc".into()),
            ],
            body: Some(b"a=1&b=2".to_vec()),
        })
        .await
        .unwrap();
    // A refusal is an answer, not a transport error: its text explains it.
    assert_eq!(response.status(), 400);
    let mut body = response.into_body();
    let mut text = Vec::new();
    while let Some(chunk) = body.next().await {
        text.extend(chunk.unwrap());
    }
    assert!(String::from_utf8_lossy(&text).contains("authorization_pending"));
    let head = server.join().unwrap().to_ascii_lowercase();
    assert!(head.starts_with("post /token "), "{head}");
    assert!(head.contains("authorization: bearer abc"), "{head}");
    assert!(head.contains("application/x-www-form-urlencoded"), "{head}");
}

#[tokio::test]
async fn transports_without_send_support_fail_permanently() {
    let error = FileTransport
        .send(HttpRequest {
            method: HttpMethod::Get,
            url: "file:///x".into(),
            headers: Vec::new(),
            body: None,
        })
        .await
        .unwrap_err();
    assert!(!error.is_retryable());
}

#[tokio::test]
async fn appearance_methods_reach_the_server_for_both_transport_routes() {
    for method in [HttpMethod::Put, HttpMethod::Delete] {
        for no_redirect in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut received = Vec::new();
                let mut buffer = [0u8; 4096];
                while !received.ends_with(b"}") {
                    let read = stream.read(&mut buffer).unwrap();
                    assert!(read > 0);
                    received.extend_from_slice(&buffer[..read]);
                }
                stream
                    .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                    .unwrap();
                String::from_utf8(received).unwrap()
            });
            let transport = HttpTransport::new().unwrap();
            let request = HttpRequest {
                method,
                url: format!("http://{address}/capes/active"),
                headers: vec![("content-type".into(), "application/json".into())],
                body: Some(br#"{"capeId":"owned"}"#.to_vec()),
            };
            let response = if no_redirect {
                transport.send_no_redirect(request).await
            } else {
                transport.send(request).await
            }
            .unwrap();
            assert_eq!(response.status(), 204);
            let wire = server.join().unwrap();
            let verb = if method == HttpMethod::Put {
                "PUT"
            } else {
                "DELETE"
            };
            assert!(
                wire.starts_with(&format!("{verb} /capes/active HTTP/1.1\r\n")),
                "{wire}"
            );
            assert!(wire.ends_with(r#"{"capeId":"owned"}"#));
        }
    }
}

#[tokio::test]
async fn transports_without_post_support_fail_permanently() {
    let error = FileTransport
        .post_json("file:///x", Vec::new())
        .await
        .unwrap_err();
    assert!(!error.message().is_empty());
}

#[tokio::test]
async fn scoped_http_returns_the_redirect_without_contacting_its_destination() {
    let origin = TcpListener::bind("127.0.0.1:0").unwrap();
    let destination = TcpListener::bind("127.0.0.1:0").unwrap();
    destination.set_nonblocking(true).unwrap();
    let origin_address = origin.local_addr().unwrap();
    let destination_address = destination.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = origin.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut buffer = [0; 4096];
        let read = stream.read(&mut buffer).unwrap();
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{destination_address}/secret\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(response.as_bytes()).unwrap();
        String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase()
    });
    let response = HttpTransport::new()
        .unwrap()
        .send_no_redirect(HttpRequest {
            method: HttpMethod::Get,
            url: format!("http://{origin_address}/"),
            headers: Vec::new(),
            body: None,
        })
        .await
        .unwrap();
    assert_eq!(response.status(), 302);
    assert_eq!(
        response.header("location"),
        Some(format!("http://{destination_address}/secret").as_str())
    );
    assert!(
        server
            .join()
            .unwrap()
            .contains(&format!("user-agent: {}", USER_AGENT.to_ascii_lowercase()))
    );
    assert_eq!(
        destination.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn transports_without_scoped_support_fail_closed() {
    let request = HttpRequest {
        method: HttpMethod::Get,
        url: "https://example.test/".into(),
        headers: Vec::new(),
        body: None,
    };
    assert!(
        !FileTransport
            .send_no_redirect(request.clone())
            .await
            .unwrap_err()
            .is_retryable()
    );
    let custom = HttpTransport::from_client(reqwest::Client::new());
    assert!(
        !custom
            .send_no_redirect(request)
            .await
            .unwrap_err()
            .is_retryable()
    );
}
