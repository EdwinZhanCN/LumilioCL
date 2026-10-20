use super::TranslationError;
use crate::transfer::{HttpMethod, HttpRequest, Transport};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

const RESPONSE_LIMIT: usize = 4 * 1024 * 1024;

async fn request<T: Transport>(
    transport: &T,
    url: &str,
    body: Value,
) -> Result<Value, TranslationError> {
    let body = serde_json::to_vec(&body).map_err(|_| TranslationError::Protocol)?;
    for attempt in 0..3_u32 {
        let sent = tokio::time::timeout(
            Duration::from_secs(20),
            transport.send(HttpRequest {
                method: HttpMethod::Post,
                url: url.to_owned(),
                headers: vec![("Content-Type".into(), "application/json".into())],
                body: Some(body.clone()),
            }),
        )
        .await;
        let response = match sent {
            Ok(Ok(response)) => response,
            Ok(Err(error)) if !error.is_retryable() => return Err(TranslationError::Network),
            _ if attempt < 2 => {
                tokio::time::sleep(backoff(attempt)).await;
                continue;
            }
            _ => return Err(TranslationError::Network),
        };
        let status = response.status();
        if !(200..300).contains(&status) {
            if attempt < 2 && (status == 429 || status >= 500) {
                let delay = response
                    .header("retry-after")
                    .and_then(retry_after)
                    .unwrap_or_else(|| backoff(attempt));
                // Very long server cooldowns are surfaced instead of keeping
                // a page task alive for minutes; never retry earlier than asked.
                if delay > Duration::from_secs(30) {
                    return Err(TranslationError::Http(status));
                }
                tokio::time::sleep(delay).await;
                continue;
            }
            return Err(TranslationError::Http(status));
        }
        if response
            .content_length()
            .is_some_and(|size| size > RESPONSE_LIMIT as u64)
        {
            return Err(TranslationError::TooLarge);
        }
        let read = async {
            let mut stream = response.into_body();
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| TranslationError::Network)?;
                if bytes.len() + chunk.len() > RESPONSE_LIMIT {
                    return Err(TranslationError::TooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            serde_json::from_slice(&bytes).map_err(|_| TranslationError::Protocol)
        };
        let result = tokio::time::timeout(Duration::from_secs(20), read)
            .await
            .unwrap_or(Err(TranslationError::Network));
        if result == Err(TranslationError::Network) && attempt < 2 {
            tokio::time::sleep(backoff(attempt)).await;
            continue;
        }
        return result;
    }
    Err(TranslationError::Network)
}

fn backoff(attempt: u32) -> Duration {
    let mut bytes = [0_u8; 1];
    let _ = getrandom::fill(&mut bytes);
    Duration::from_millis((500_u64 << attempt) + u64::from(bytes[0]))
}

fn retry_after(value: &str) -> Option<Duration> {
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let time = chrono::DateTime::parse_from_rfc2822(value)
        .ok()?
        .timestamp();
    Some(Duration::from_secs(
        u64::try_from(time.saturating_sub(chrono::Utc::now().timestamp())).unwrap_or(0),
    ))
}

pub(super) async fn edge<T: Transport>(
    transport: &T,
    texts: &[String],
) -> Result<Vec<String>, TranslationError> {
    let answer = request(
        transport,
        "https://edge.microsoft.com/translate/translatetext?to=zh-Hans&isEnterpriseClient=false",
        json!(texts),
    )
    .await?;
    let rows = answer
        .as_array()
        .filter(|rows| rows.len() == texts.len())
        .ok_or(TranslationError::Protocol)?;
    rows.iter()
        .zip(texts)
        .map(|(row, original)| {
            let rows = row["translations"]
                .as_array()
                .filter(|rows| rows.len() == 1)
                .ok_or(TranslationError::Protocol)?;
            if rows[0]["to"] != "zh-Hans" {
                return Err(TranslationError::Protocol);
            }
            rows[0]["text"]
                .as_str()
                .filter(|text| !text.trim().is_empty())
                .map(|text| {
                    let leading = &original[..original.len() - original.trim_start().len()];
                    let trailing = &original[original.trim_end().len()..];
                    format!("{leading}{}{trailing}", text.trim())
                })
                .ok_or(TranslationError::Protocol)
        })
        .collect()
}

/// Where translated summaries are asked for, in order: MCIM, then our own
/// Worker for what MCIM could not answer (plan translation-fallback). Both
/// speak MCIM's `/translate/{platform}` protocol.
pub(super) const SUMMARY_SERVICES: [(&str, &str); 2] = [
    ("mcim", "https://mod.mcimirror.top"),
    ("lumilio", "https://launcher.lumilio.org"),
];

/// One batch of `(original, translated)` summaries by ID from a service in
/// [`SUMMARY_SERVICES`]; IDs it has nothing for are left out.
pub(super) async fn summaries<T: Transport>(
    transport: &T,
    base: &str,
    platform: &str,
    ids: &[String],
) -> Result<BTreeMap<String, (String, String)>, TranslationError> {
    let body = match platform {
        "modrinth" => json!({"project_ids": ids}),
        "curseforge" => {
            json!({"modids": ids.iter().map(|id| id.parse::<i32>()).collect::<Result<Vec<_>, _>>().map_err(|_| TranslationError::Protocol)?})
        }
        _ => return Err(TranslationError::Protocol),
    };
    let answer = request(transport, &format!("{base}/translate/{platform}"), body).await?;
    let rows = answer.as_array().ok_or(TranslationError::Protocol)?;
    let mut result = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        let id = match platform {
            "modrinth" => row["project_id"].as_str().map(str::to_owned),
            _ => row["modid"].as_i64().map(|id| id.to_string()),
        }
        .ok_or(TranslationError::Protocol)?;
        if !ids.contains(&id) || !seen.insert(id.clone()) {
            return Err(TranslationError::Protocol);
        }
        let original = row["original"]
            .as_str()
            .ok_or(TranslationError::Protocol)?
            .to_owned();
        let translated = row["translated"]
            .as_str()
            .ok_or(TranslationError::Protocol)?
            .to_owned();
        result.insert(id, (original, translated));
    }
    Ok(result)
}
