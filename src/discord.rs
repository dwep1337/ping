use serde_json::{json, Value};

const COLOR_DOWN: u32 = 0xe7_4c_3c;
const COLOR_UP: u32 = 0x2e_cc_71;

pub enum Alert<'a> {
    Down { reason: &'a str },
    Up,
}

fn mention_content(mention_ids: &[String]) -> String {
    mention_ids
        .iter()
        .map(|id| format!("<@{id}>"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub async fn notify(
    webhook: &str,
    url: &str,
    mention_ids: &[String],
    alert: Alert<'_>,
) -> Result<(), String> {
    let (title, description, color, fields) = match &alert {
        Alert::Down { reason } => (
            "API down",
            format!("Health check failed for `{url}`"),
            COLOR_DOWN,
            vec![json!({ "name": "Reason", "value": reason, "inline": false })],
        ),
        Alert::Up => (
            "API recovered",
            format!("Health check succeeded for `{url}`"),
            COLOR_UP,
            Vec::<Value>::new(),
        ),
    };

    let mut embed = json!({
        "title": title,
        "description": description,
        "color": color,
        "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    });

    if !fields.is_empty() {
        embed["fields"] = Value::Array(fields);
    }

    let mut body = json!({
        "embeds": [embed],
    });

    if !mention_ids.is_empty() {
        body["content"] = Value::String(mention_content(mention_ids));
        body["allowed_mentions"] = json!({ "users": mention_ids });
    }

    post_webhook(webhook, body).await
}

async fn post_webhook(webhook: &str, body: Value) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .post(webhook)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Discord webhook failed: HTTP {status} {text}")
            .trim()
            .to_string());
    }

    Ok(())
}
