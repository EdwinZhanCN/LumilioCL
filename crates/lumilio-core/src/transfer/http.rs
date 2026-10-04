use super::USER_AGENT;
use super::transport::{
    HttpMethod, HttpRequest, Transport, TransportError, TransportFuture, TransportResponse,
};
use futures_util::stream;
use futures_util::stream::StreamExt;
use reqwest::Client;
use std::fmt::Debug;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use url::Url;

#[derive(Clone, Debug)]
pub struct HttpTransport {
    pub(super) client: Client,
}

impl HttpTransport {
    pub fn new() -> Result<Self, TransportError> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(|error| TransportError::permanent(error.to_string()))?;
        Ok(Self { client })
    }

    #[must_use]
    pub const fn from_client(client: Client) -> Self {
        Self { client }
    }

    #[must_use]
    pub const fn client(&self) -> &Client {
        &self.client
    }
    pub(super) fn respond(
        result: Result<reqwest::Response, reqwest::Error>,
    ) -> Result<TransportResponse, TransportError> {
        let response = result.map_err(|error| {
            if error.is_timeout() || error.is_connect() {
                TransportError::transient(error.to_string())
            } else {
                TransportError::permanent(error.to_string())
            }
        })?;
        let status = response.status().as_u16();
        let content_length = response.content_length();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                Some((name.as_str().to_owned(), value.to_str().ok()?.to_owned()))
            })
            .collect();
        let body = response.bytes_stream().map(|chunk| {
            chunk
                .map(|bytes| bytes.to_vec())
                .map_err(|error| TransportError::transient(error.to_string()))
        });
        Ok(TransportResponse::new(status, content_length, Box::pin(body)).with_headers(headers))
    }
}

impl Transport for HttpTransport {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move { Self::respond(self.client.get(source).send().await) })
    }

    fn post_json<'a>(&'a self, source: &'a str, body: Vec<u8>) -> TransportFuture<'a> {
        Box::pin(async move {
            Self::respond(
                self.client
                    .post(source)
                    .header("content-type", "application/json")
                    .body(body)
                    .send()
                    .await,
            )
        })
    }

    fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        Box::pin(async move {
            let mut builder = match request.method {
                HttpMethod::Get => self.client.get(&request.url),
                HttpMethod::Post => self.client.post(&request.url),
            };
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }
            if let Some(body) = request.body {
                builder = builder.body(body);
            }
            Self::respond(builder.send().await)
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FileTransport;

impl Transport for FileTransport {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move {
            let url =
                Url::parse(source).map_err(|error| TransportError::permanent(error.to_string()))?;
            if url.scheme() != "file" {
                return Err(TransportError::permanent(format!(
                    "unsupported local source scheme: {}",
                    url.scheme()
                )));
            }
            let path = url
                .to_file_path()
                .map_err(|()| TransportError::permanent("invalid file URL"))?;
            let file = File::open(&path)
                .await
                .map_err(|error| TransportError::permanent(error.to_string()))?;
            let content_length = file
                .metadata()
                .await
                .map_err(|error| TransportError::permanent(error.to_string()))?
                .len();
            let body = stream::unfold(Some(file), |state| async move {
                let mut file = state?;
                let mut buffer = vec![0_u8; 64 * 1024];
                match file.read(&mut buffer).await {
                    Ok(0) => None,
                    Ok(read) => {
                        buffer.truncate(read);
                        Some((Ok(buffer), Some(file)))
                    }
                    Err(error) => Some((Err(TransportError::permanent(error.to_string())), None)),
                }
            });
            Ok(TransportResponse::new(
                200,
                Some(content_length),
                Box::pin(body),
            ))
        })
    }
}

#[derive(Clone, Debug)]
pub struct DefaultTransport {
    pub(super) http: HttpTransport,
    pub(super) file: FileTransport,
}

impl DefaultTransport {
    pub fn new() -> Result<Self, TransportError> {
        Ok(Self {
            http: HttpTransport::new()?,
            file: FileTransport,
        })
    }
}

impl Transport for DefaultTransport {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        if Url::parse(source).is_ok_and(|url| url.scheme() == "file") {
            self.file.get(source)
        } else {
            self.http.get(source)
        }
    }

    fn post_json<'a>(&'a self, source: &'a str, body: Vec<u8>) -> TransportFuture<'a> {
        self.http.post_json(source, body)
    }

    fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        self.http.send(request)
    }
}
