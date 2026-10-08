use super::*;
use crate::transfer::{TransportError, TransportFuture, TransportResponse};
use std::{collections::VecDeque, sync::Mutex};

#[derive(Default)]
struct Script {
    replies: Mutex<VecDeque<(u16, Vec<u8>)>>,
    requests: Mutex<Vec<HttpRequest>>,
}
impl Script {
    fn answer(&self, status: u16, body: impl Into<Vec<u8>>) {
        self.replies
            .lock()
            .unwrap()
            .push_back((status, body.into()));
    }
}
impl Transport for Script {
    fn get<'a>(&'a self, url: &'a str) -> TransportFuture<'a> {
        self.send(HttpRequest {
            method: HttpMethod::Get,
            url: url.into(),
            headers: vec![],
            body: None,
        })
    }
    fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        self.requests.lock().unwrap().push(request);
        let reply = self.replies.lock().unwrap().pop_front();
        Box::pin(async move {
            reply
                .map(|(status, bytes)| TransportResponse::from_bytes(status, bytes))
                .ok_or_else(|| TransportError::transient("offline"))
        })
    }
}

const PROFILE: &str = r#"{"id":"123e4567e89b12d3a456426614174000","name":"Player","skins":[{"id":"skin","state":"ACTIVE","url":"http://textures.minecraft.net/texture/skin","variant":"SLIM"}],"capes":[{"id":"owned","state":"ACTIVE","url":"http://textures.minecraft.net/texture/cape","alias":"Minecon"}]}"#;

pub(crate) fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba(color));
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

#[tokio::test]
async fn profile_and_textures_keep_the_model_and_never_send_tokens_to_the_cdn() {
    let script = Script::default();
    script.answer(200, PROFILE);
    script.answer(200, png(64, 64, [1, 2, 3, 255]));
    script.answer(200, png(64, 32, [4, 5, 6, 255]));
    let client = MojangClient::new(&script);
    let profile = client.profile(&Secret::new("private-token")).await.unwrap();
    assert_eq!(profile.active_skin().unwrap().model, SkinModel::Slim);
    assert_eq!(profile.active_cape().unwrap().alias, "Minecon");
    let look = client.look(&profile).await.unwrap();
    assert_eq!(look.model, SkinModel::Slim);
    assert_eq!(&look.skin.unwrap().rgba[..4], &[1, 2, 3, 255]);
    assert_eq!(&look.cape.unwrap().rgba[..4], &[4, 5, 6, 255]);
    let requests = script.requests.lock().unwrap();
    assert_eq!(requests[0].headers[0].1, "Bearer private-token");
    for request in &requests[1..] {
        assert!(request.url.starts_with("https://textures.minecraft.net/"));
        assert!(request.headers.is_empty());
    }
}

#[tokio::test]
async fn legacy_upload_is_a_normalized_png_in_a_complete_multipart_body() {
    let script = Script::default();
    script.answer(200, PROFILE); // Still-old profile is a successful write.
    let update = MojangClient::new(&script)
        .change(
            &Secret::new("token"),
            AppearanceChange::Upload {
                png: png(64, 32, [12, 34, 56, 255]),
                model: SkinModel::Wide,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        update.profile.unwrap().active_skin().unwrap().model,
        SkinModel::Slim
    );
    let requests = script.requests.lock().unwrap();
    let request = &requests[0];
    assert_eq!(request.method, HttpMethod::Post);
    assert_eq!(request.url, format!("{PROFILE_URL}/skins"));
    let content_type = &request
        .headers
        .iter()
        .find(|(name, _)| name == "content-type")
        .unwrap()
        .1;
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    let body = request.body.as_ref().unwrap();
    assert!(body.starts_with(format!("--{boundary}\r\n").as_bytes()));
    assert!(body.ends_with(format!("\r\n--{boundary}--\r\n").as_bytes()));
    let text = String::from_utf8_lossy(body);
    assert!(text.contains("name=\"variant\"\r\n\r\nclassic\r\n"));
    assert!(text.contains("filename=\"skin.png\"\r\nContent-Type: image/png"));
    let at = body
        .windows(8)
        .position(|bytes| bytes == b"\x89PNG\r\n\x1a\n")
        .unwrap();
    let image = image::load_from_memory(&body[at..]).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (64, 64));
    assert_eq!(image.get_pixel(20, 52).0, [12, 34, 56, 255]);
}

#[tokio::test]
async fn bad_images_are_refused_before_any_request() {
    let script = Script::default();
    for bytes in [
        png(63, 64, [0; 4]),
        png(128, 128, [0; 4]),
        b"not png".to_vec(),
    ] {
        let error = MojangClient::new(&script)
            .change(
                &Secret::new("token"),
                AppearanceChange::Upload {
                    png: bytes,
                    model: SkinModel::Wide,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, AppearanceError::Picture(_)));
    }
    assert!(script.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn capes_must_be_owned_and_each_operation_uses_its_protocol_method() {
    let script = Script::default();
    let client = MojangClient::new(&script);
    let token = Secret::new("token");
    script.answer(200, PROFILE);
    assert_eq!(
        client
            .change(&token, AppearanceChange::Cape(Some("unknown".into())))
            .await
            .unwrap_err(),
        AppearanceError::CapeNotOwned
    );
    script.answer(200, PROFILE);
    script.answer(204, Vec::new());
    assert!(
        client
            .change(&token, AppearanceChange::Cape(Some("owned".into())))
            .await
            .unwrap()
            .profile
            .is_none()
    );
    script.answer(204, Vec::new());
    client
        .change(&token, AppearanceChange::Cape(None))
        .await
        .unwrap();
    script.answer(200, b"no profile".to_vec());
    assert!(
        client
            .change(&token, AppearanceChange::DefaultSkin)
            .await
            .unwrap()
            .profile
            .is_none()
    );
    let requests = script.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.method)
            .collect::<Vec<_>>(),
        [
            HttpMethod::Get,
            HttpMethod::Get,
            HttpMethod::Put,
            HttpMethod::Delete,
            HttpMethod::Delete
        ]
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(requests[2].body.as_ref().unwrap()).unwrap(),
        serde_json::json!({"capeId":"owned"})
    );
    assert!(requests[3].url.ends_with("/capes/active"));
    assert!(requests[4].url.ends_with("/skins/active"));
}

#[tokio::test]
async fn ownership_token_picture_and_network_failures_stay_distinct_and_redacted() {
    let script = Script::default();
    let client = MojangClient::new(&script);
    let token = Secret::new("private-token");
    for (status, expected) in [
        (404, AppearanceError::NoGameOwnership),
        (401, AppearanceError::SignInRequired),
        (403, AppearanceError::Refused(403)),
    ] {
        script.answer(status, "private-token");
        let error = client.profile(&token).await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains(token.expose()));
    }
    script.answer(400, "bad image");
    assert!(matches!(
        client
            .change(
                &token,
                AppearanceChange::Upload {
                    png: png(64, 64, [0; 4]),
                    model: SkinModel::Slim
                }
            )
            .await,
        Err(AppearanceError::Picture(_))
    ));
    assert!(matches!(
        client.profile(&token).await,
        Err(AppearanceError::Network(_))
    ));
    script.answer(200, vec![0; 1024 * 1024 + 1]);
    assert!(matches!(
        client.profile(&token).await,
        Err(AppearanceError::Protocol(_))
    ));
}
