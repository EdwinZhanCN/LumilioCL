use super::*;
use crate::transfer::{TransportError, TransportFuture, TransportResponse};
use std::{collections::VecDeque, sync::Mutex};

#[derive(Default)]
struct Script {
    replies: Mutex<VecDeque<(u16, Vec<u8>)>>,
    urls: Mutex<Vec<String>>,
}
impl Transport for Script {
    fn get<'a>(&'a self, url: &'a str) -> TransportFuture<'a> {
        self.urls.lock().unwrap().push(url.into());
        let reply = self.replies.lock().unwrap().pop_front();
        Box::pin(async move {
            reply
                .map(|(status, bytes)| TransportResponse::from_bytes(status, bytes))
                .ok_or_else(|| TransportError::transient("offline"))
        })
    }
}
fn png(height: u32, color: [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(64, height, image::Rgba(color));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}
const ID: &str = "123e4567e89b12d3a456426614174000";
fn profile(id: &str, skin: &str) -> Vec<u8> {
    let textures = serde_json::json!({"textures":{"SKIN":{"url":skin,"metadata":{"model":"slim"}}, "CAPE":{"url":"https://example.test/cape"}}});
    serde_json::json!({"id":id,"properties":[{"name":"textures","value":STANDARD.encode(textures.to_string())}]}).to_string().into_bytes()
}

#[tokio::test]
async fn third_party_session_loads_slim_skin_and_cape_without_authentication() {
    let script = Script::default();
    script.replies.lock().unwrap().extend([
        (200, profile(ID, "https://example.test/skin")),
        (200, png(64, [1, 2, 3, 255])),
        (200, png(32, [4, 5, 6, 255])),
    ]);
    let look = load(
        &script,
        "https://example.test/api/",
        ProfileId::parse(ID).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(look.model, SkinModel::Slim);
    assert_eq!(look.skin.unwrap().rgba[..4], [1, 2, 3, 255]);
    assert_eq!(look.cape.unwrap().rgba[..4], [4, 5, 6, 255]);
    assert_eq!(
        script.urls.lock().unwrap().as_slice(),
        [
            format!("https://example.test/api/sessionserver/session/minecraft/profile/{ID}"),
            "https://example.test/skin".into(),
            "https://example.test/cape".into()
        ]
    );
}

#[tokio::test]
async fn missing_profile_wrong_identity_and_invalid_texture_urls_are_distinct() {
    let id = ProfileId::parse(ID).unwrap();
    let script = Script::default();
    script.replies.lock().unwrap().extend([
        (204, Vec::new()),
        (200, profile("other", "https://example.test/skin")),
        (200, profile(ID, "file:///secret")),
        (200, b"not JSON".to_vec()),
    ]);
    assert_eq!(
        load(&script, "https://example.test/", id).await.unwrap(),
        AccountLook::default()
    );
    assert!(matches!(
        load(&script, "https://example.test/", id).await,
        Err(SkinError::Malformed(_))
    ));
    assert!(matches!(
        load(&script, "https://example.test/", id).await,
        Err(SkinError::InvalidApi(_))
    ));
    assert!(matches!(
        load(&script, "https://example.test/", id).await,
        Err(SkinError::Malformed(_))
    ));
    assert_eq!(script.urls.lock().unwrap().len(), 4);
}
