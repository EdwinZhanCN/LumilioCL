use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FetchMethod {
    Get,
    Post,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FetchRequest {
    pub method: FetchMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl FetchRequest {
    #[must_use]
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: FetchMethod::Get,
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    #[must_use]
    pub fn json(url: impl Into<String>, body: Vec<u8>) -> Self {
        Self {
            method: FetchMethod::Post,
            url: url.into(),
            headers: vec![("content-type".into(), "application/json".into())],
            body: Some(body),
        }
    }
}
