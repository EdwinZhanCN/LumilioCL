use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

use super::server::normalize_address;
use super::*;
use crate::transfer::{TransportError, TransportFuture, TransportResponse};

const ROOT: &str = "https://auth.example/api/yggdrasil/";
const ID: &str = "123e4567e89b12d3a456426614174000";
const OTHER_ID: &str = "223e4567e89b12d3a456426614174000";

type Route = (u16, Vec<(String, String)>, String);

/// Answers from a script keyed by address (the last answer repeats), for both
/// `get` and `send`, and remembers what was sent.
#[derive(Default)]
struct Script {
    routes: Mutex<BTreeMap<String, VecDeque<Route>>>,
    seen: Mutex<Vec<(String, Option<String>)>>,
}

impl Script {
    fn on(&self, url: &str, status: u16, body: &str) {
        self.with_headers(url, status, &[], body);
    }

    fn with_headers(&self, url: &str, status: u16, headers: &[(&str, &str)], body: &str) {
        let headers = headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        self.routes
            .lock()
            .unwrap()
            .entry(url.to_owned())
            .or_default()
            .push_back((status, headers, body.to_owned()));
    }

    fn answer(&self, url: &str, body: Option<String>) -> Result<TransportResponse, TransportError> {
        self.seen.lock().unwrap().push((url.to_owned(), body));
        let mut routes = self.routes.lock().unwrap();
        let queue = routes
            .get_mut(url)
            .ok_or_else(|| TransportError::transient("connection refused"))?;
        let (status, headers, body) = if queue.len() > 1 {
            queue.pop_front().unwrap()
        } else {
            queue.front().cloned().unwrap()
        };
        Ok(TransportResponse::from_bytes(status, body.into_bytes()).with_headers(headers))
    }

    /// The JSON bodies posted to `url`.
    fn posted(&self, url: &str) -> Vec<Value> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|(seen, _)| seen == url)
            .filter_map(|(_, body)| serde_json::from_str(body.as_deref()?).ok())
            .collect()
    }
}

impl Transport for Script {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move { self.answer(source, None) })
    }

    fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        Box::pin(async move {
            let body = request.body.map(|bytes| String::from_utf8(bytes).unwrap());
            self.answer(&request.url, body)
        })
    }
}

fn url(path: &str) -> String {
    format!("{ROOT}{path}")
}

fn session_body(
    client: &str,
    selected: Option<(&str, &str)>,
    available: &[(&str, &str)],
) -> String {
    json!({
        "accessToken": "access-1",
        "clientToken": client,
        "selectedProfile": selected.map(|(id, name)| json!({"id": id, "name": name})),
        "availableProfiles": available.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "user": { "id": "u", "properties": [{"name": "preferredLanguage", "value": "zh_CN"}] },
    })
    .to_string()
}

#[tokio::test]
async fn signing_in_sends_the_agent_and_returns_the_chosen_character() {
    let script = Script::default();
    script.on(
        &url("authserver/authenticate"),
        200,
        &session_body("c1", Some((ID, "Edwin")), &[]),
    );
    let client = YggdrasilClient::new(&script, ROOT);
    let session = client
        .authenticate("me@example.com", "pw", "c1")
        .await
        .unwrap();
    assert_eq!(session.access_token.expose(), "access-1");
    assert_eq!(session.selected.as_ref().unwrap().name, "Edwin");
    assert_eq!(session.user_properties["preferredLanguage"], "zh_CN");
    let sent = &script.posted(&url("authserver/authenticate"))[0];
    assert_eq!(sent["agent"], json!({"name": "Minecraft", "version": 1}));
    assert_eq!(sent["username"], "me@example.com");
    assert_eq!(sent["clientToken"], "c1");
    assert_eq!(sent["requestUser"], true);
}

#[tokio::test]
async fn several_characters_come_back_to_be_chosen_from() {
    let script = Script::default();
    script.on(
        &url("authserver/authenticate"),
        200,
        &session_body("c1", None, &[(ID, "A"), (OTHER_ID, "B")]),
    );
    let session = YggdrasilClient::new(&script, ROOT)
        .authenticate("me", "pw", "c1")
        .await
        .unwrap();
    assert!(session.selected.is_none());
    assert_eq!(session.available.len(), 2);
    assert_eq!(session.available[1].name, "B");
}

#[tokio::test]
async fn refused_credentials_and_oddities_are_told_apart() {
    let script = Script::default();
    let endpoint = url("authserver/authenticate");
    let client = YggdrasilClient::new(&script, ROOT);

    script.on(
        &endpoint,
        403,
        r#"{"error":"ForbiddenOperationException","errorMessage":"Invalid credentials. Invalid username or password."}"#,
    );
    assert_eq!(
        client.authenticate("me", "bad", "c1").await,
        Err(YggdrasilError::InvalidCredentials)
    );

    // Another kind of refusal keeps the server's own words.
    let other = Script::default();
    other.on(
        &endpoint,
        429,
        r#"{"error":"TooManyRequests","errorMessage":"slow down"}"#,
    );
    assert_eq!(
        YggdrasilClient::new(&other, ROOT)
            .authenticate("me", "x", "c1")
            .await,
        Err(YggdrasilError::Remote {
            kind: "TooManyRequests".into(),
            message: Some("slow down".into())
        })
    );

    // A client token that changed on the way is not the session we asked for.
    let changed = Script::default();
    changed.on(
        &endpoint,
        200,
        &session_body("someone-else", Some((ID, "E")), &[]),
    );
    assert!(matches!(
        YggdrasilClient::new(&changed, ROOT)
            .authenticate("me", "x", "c1")
            .await,
        Err(YggdrasilError::Malformed(_))
    ));

    let html = Script::default();
    html.on(&endpoint, 200, "<html>maintenance</html>");
    let Err(YggdrasilError::Malformed(detail)) = YggdrasilClient::new(&html, ROOT)
        .authenticate("me", "x", "c1")
        .await
    else {
        panic!("expected a malformed answer");
    };
    assert!(detail.contains("maintenance"));

    let down = Script::default();
    assert!(matches!(
        YggdrasilClient::new(&down, ROOT)
            .authenticate("me", "x", "c1")
            .await,
        Err(YggdrasilError::Network(_))
    ));
}

#[tokio::test]
async fn choosing_a_character_must_end_with_that_character() {
    let script = Script::default();
    let endpoint = url("authserver/refresh");
    let choice = Profile {
        id: ProfileId::parse(ID).unwrap(),
        name: "A".into(),
    };
    let client = YggdrasilClient::new(&script, ROOT);

    script.on(&endpoint, 200, &session_body("c1", Some((ID, "A")), &[]));
    let session = client
        .refresh(&Secret::new("access-0"), "c1", Some(&choice))
        .await
        .unwrap();
    assert_eq!(session.selected.unwrap().id, choice.id);
    let sent = &script.posted(&endpoint)[0];
    assert_eq!(sent["accessToken"], "access-0");
    assert_eq!(sent["selectedProfile"], json!({"id": ID, "name": "A"}));

    let wrong = Script::default();
    wrong.on(
        &endpoint,
        200,
        &session_body("c1", Some((OTHER_ID, "B")), &[]),
    );
    assert!(matches!(
        YggdrasilClient::new(&wrong, ROOT)
            .refresh(&Secret::new("a"), "c1", Some(&choice))
            .await,
        Err(YggdrasilError::Malformed(_))
    ));

    // Without a choice nothing is asked for.
    let plain = Script::default();
    plain.on(&endpoint, 200, &session_body("c1", Some((ID, "A")), &[]));
    YggdrasilClient::new(&plain, ROOT)
        .refresh(&Secret::new("a"), "c1", None)
        .await
        .unwrap();
    assert!(plain.posted(&endpoint)[0].get("selectedProfile").is_none());
}

#[tokio::test]
async fn validating_says_yes_no_or_fails() {
    let script = Script::default();
    let endpoint = url("authserver/validate");
    let client = YggdrasilClient::new(&script, ROOT);
    script.on(&endpoint, 204, "");
    assert_eq!(client.validate(&Secret::new("a"), "c1").await, Ok(true));
    let refused = Script::default();
    refused.on(
        &endpoint,
        403,
        r#"{"error":"ForbiddenOperationException","errorMessage":"Invalid token."}"#,
    );
    assert_eq!(
        YggdrasilClient::new(&refused, ROOT)
            .validate(&Secret::new("a"), "c1")
            .await,
        Ok(false)
    );
    let broken = Script::default();
    broken.on(&endpoint, 500, r#"{"error":"InternalError"}"#);
    assert!(
        YggdrasilClient::new(&broken, ROOT)
            .validate(&Secret::new("a"), "c1")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn signing_out_tells_the_server_to_forget_the_token() {
    let script = Script::default();
    script.on(&url("authserver/invalidate"), 204, "");
    YggdrasilClient::new(&script, ROOT)
        .invalidate(&Secret::new("a"), "c1")
        .await
        .unwrap();
    assert_eq!(
        script.posted(&url("authserver/invalidate"))[0]["accessToken"],
        "a"
    );
}

#[test]
fn typed_addresses_become_https_urls() {
    assert_eq!(
        normalize_address("littleskin.cn/api/yggdrasil").unwrap(),
        "https://littleskin.cn/api/yggdrasil"
    );
    assert_eq!(
        normalize_address(" http://localhost:8080/x/ ").unwrap(),
        "http://localhost:8080/x/"
    );
    assert!(normalize_address("ftp://example.com").is_err());
    assert!(normalize_address("").is_err());
}

#[tokio::test]
async fn a_server_is_found_through_the_api_location_header() {
    let script = Script::default();
    script.with_headers(
        "https://example.com/",
        200,
        &[("X-Authlib-Injector-API-Location", "/api/yggdrasil/")],
        "<html>home page</html>",
    );
    script.on(
        "https://example.com/api/yggdrasil/",
        200,
        r#"{"meta":{"serverName":"Example Skins","feature.non_email_login":true,"links":{"homepage":"https://example.com","register":"https://example.com/reg","count":3}}}"#,
    );
    let server = locate_server(&script, "example.com").await.unwrap();
    assert_eq!(server.url, "https://example.com/api/yggdrasil/");
    assert_eq!(server.display_name(), "Example Skins");
    assert!(server.non_email_login && !server.is_insecure());
    assert_eq!(server.links.len(), 2, "only text links are kept");
}

#[tokio::test]
async fn a_server_without_a_redirect_or_name_is_still_a_server() {
    let script = Script::default();
    script.on("http://skins.local/api", 200, r#"{"meta":{}}"#);
    // The typed address has no trailing slash; the stored root always has one.
    let server = locate_server(&script, "http://skins.local/api")
        .await
        .unwrap();
    assert_eq!(server.url, "http://skins.local/api/");
    assert_eq!(server.display_name(), "http://skins.local/api/");
    assert!(server.is_insecure() && !server.non_email_login);

    // A header that points back at the same place is not followed again.
    let same = Script::default();
    same.with_headers(
        "https://a.example/y/",
        200,
        &[("x-authlib-injector-api-location", "/y")],
        r#"{"meta":{"serverName":"A"}}"#,
    );
    assert_eq!(
        locate_server(&same, "https://a.example/y/")
            .await
            .unwrap()
            .display_name(),
        "A"
    );
    assert_eq!(same.seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn an_unreachable_or_nonsense_server_is_an_error() {
    let down = Script::default();
    assert!(matches!(
        locate_server(&down, "nowhere.example").await,
        Err(YggdrasilError::Network(_))
    ));
    let page = Script::default();
    page.on("https://web.example/", 200, "<html>just a website</html>");
    assert!(matches!(
        locate_server(&page, "web.example").await,
        Err(YggdrasilError::Malformed(_))
    ));
    let missing = Script::default();
    missing.on("https://gone.example/", 404, "");
    assert!(matches!(
        locate_server(&missing, "gone.example").await,
        Err(YggdrasilError::Network(_))
    ));
}

#[test]
fn little_skin_is_built_in() {
    let server = little_skin();
    assert_eq!(server.url, "https://littleskin.cn/api/yggdrasil/");
    assert_eq!(server.display_name(), "LittleSkin");
}
