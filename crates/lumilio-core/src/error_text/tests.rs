use super::with_causes;
use std::fmt;

#[derive(Debug)]
struct Layer(&'static str, Option<Box<Layer>>);

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Layer {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.1.as_deref().map(|layer| layer as _)
    }
}

#[test]
fn an_error_says_what_caused_it_once() {
    let error = Layer(
        "error sending request for url (https://example.invalid/)",
        Some(Box::new(Layer(
            "client error (Connect)",
            Some(Box::new(Layer(
                "Connect",
                Some(Box::new(Layer("dns error: no such host", None))),
            ))),
        ))),
    );
    assert_eq!(
        with_causes(&error),
        "error sending request for url (https://example.invalid/): client error (Connect): dns error: no such host"
    );
}

#[tokio::test]
async fn a_refused_connection_says_why() {
    // A port nothing listens on: bound, then closed.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let error = reqwest::get(format!("http://127.0.0.1:{port}/"))
        .await
        .expect_err("nothing listens there");
    let text = with_causes(&error);
    assert!(text.len() > error.to_string().len(), "{text}");
}
