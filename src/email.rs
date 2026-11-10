use crate::models::EmailConfig;
use anyhow::{Context, Result};
use base64::Engine;
use reqwest::Client;
use serde_json::json;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::{error, info};
use rusqlite::Connection;
use crate::db;
use uuid::Uuid;

// Note: Address and lettre types are no longer needed since we handle recipients via strings/HTTP JSON.

pub fn parse_recipient_list(value: &str, field_name: &str) -> Result<Vec<String>> {
    let recipients: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(String::from)
        .collect();

    if recipients.is_empty() {
        anyhow::bail!("Missing '{}' email", field_name);
    }

    Ok(recipients)
}

pub fn normalize_recipient_list(value: &str, field_name: &str) -> Result<String> {
    let recipients = parse_recipient_list(value, field_name)?;
    Ok(recipients.join(", "))
}

// Kept for backward compatibility with existing tests
fn build_epub_message(config: &EmailConfig, filename: &str, filebody: Vec<u8>) -> Result<serde_json::Value> {
    let email_from = if config.email_address.trim().is_empty() {
        env::var("EMAIL_FROM").unwrap_or_else(|_| "RSSPub <onboarding@resend.dev>".to_string())
    } else {
        config.email_address.clone()
    };

    let recipients = parse_recipient_list(&config.to_email, "to")?;
    let encoded_file = base64::engine::general_purpose::STANDARD.encode(&filebody);

    Ok(json!({
        "from": email_from,
        "to": recipients,
        "subject": format!("RSS Digest: {}", filename),
        "html": "Here is your requested RSS Digest EPUB.",
        "attachments": [
            {
                "filename": filename,
                "content": encoded_file
            }
        ]
    }))
}

fn debug_dump_email(payload: &serde_json::Value) {
    if env::var_os("RSSPUB_DEBUG_EMAIL_DUMP").as_deref() != Some("1".as_ref()) {
        return;
    }

    let dump_path = env::temp_dir().join(format!("rsspub-email-{}.json", Uuid::new_v4()));
    match fs::write(&dump_path, payload.to_string()) {
        Ok(()) => info!("Wrote debug email dump to {}", dump_path.display()),
        Err(err) => error!("Failed to write debug email dump: {}", err),
    }
}

pub async fn send_epub(config: &EmailConfig, epub_path: &Path) -> Result<()> {
    info!("Preparing to send email to {}", config.to_email);

    let filename = epub_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("digest.epub");

    let filebody = fs::read(epub_path).context("Failed to read EPUB file")?;
    let payload = build_epub_message(config, filename, filebody)?;
    debug_dump_email(&payload);

    let api_key = env::var("RESEND_API_KEY")
        .context("RESEND_API_KEY environment variable not set")?;

    let client = Client::new();

    info!("Sending email via Resend HTTP API...");

    let res = client.post("https://api.resend.com/emails")
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
        .context("Failed to connect to Resend API")?;

    if res.status().is_success() {
        info!("Email sent successfully via HTTP!");
    } else {
        let err_text = res.text().await.unwrap_or_default();
        anyhow::bail!("Resend API error: {}", err_text);
    }

    Ok(())
}

pub async fn check_and_send_email(
    db: Arc<Mutex<Connection>>,
    filename: &String,
    override_to_email: Option<&str>,
) -> Result<()> {
    let send_email = {
        let conn = db.lock().map_err(|_| anyhow::anyhow!("DB lock failed"))?;
        match db::get_email_config(&conn)? {
            Some(config) => config.enable_auto_send,
            None => false,
        }
    };

    if send_email {
        info!("Auto-send enabled. Sending email...");

        let config_opt = {
            let conn = db.lock().map_err(|_| anyhow::anyhow!("DB lock failed"))?;
            db::get_email_config(&conn)?
        };

        if let Some(config) = config_opt {
            let mut config = config;
            if let Some(override_to_email) = override_to_email {
                config.to_email = override_to_email.to_string();
            }
            let epub_path = std::path::Path::new(crate::util::EPUB_OUTPUT_DIR).join(&filename);
            if let Err(e) = send_epub(&config, &epub_path).await {
                error!("Failed to auto-send email: {}", e);
            } else {
                info!("Auto-send email sent successfully.");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_epub_message, normalize_recipient_list};
    use crate::models::EmailConfig;

    fn test_email_config() -> EmailConfig {
        EmailConfig {
            smtp_host: "".to_string(),
            smtp_port: 0,
            smtp_password: "".to_string(),
            email_address: "sender@example.com".to_string(),
            smtp_username: "".to_string(),
            to_email: "kindle@example.com".to_string(),
            enable_auto_send: false,
        }
    }

    #[test]
    fn serializes_kindle_friendly_epub_email_payload() {
        let payload = build_epub_message(
            &test_email_config(),
            "digest.epub",
            b"dummy epub bytes".to_vec(),
        )
        .expect("payload should build");

        assert_eq!(payload["from"], "sender@example.com");
        assert_eq!(payload["to"][0], "kindle@example.com");
        assert_eq!(payload["subject"], "RSS Digest: digest.epub");
        assert!(payload["attachments"][0]["content"].is_string());
        assert_eq!(payload["attachments"][0]["filename"], "digest.epub");
    }

    #[test]
    fn serializes_multiple_recipients() {
        let mut config = test_email_config();
        config.to_email = "kindle@example.com, backup@example.com".to_string();

        let payload = build_epub_message(&config, "digest.epub", b"dummy epub bytes".to_vec())
            .expect("payload should build");

        let recipients = payload["to"].as_array().unwrap();
        assert_eq!(recipients.len(), 2);
        assert_eq!(recipients[0], "kindle@example.com");
        assert_eq!(recipients[1], "backup@example.com");
    }

    #[test]
    fn normalizes_recipient_lists() {
        let normalized =
            normalize_recipient_list(" kindle@example.com , , backup@example.com ", "to")
                .expect("recipient list should normalize");

        assert_eq!(normalized, "kindle@example.com, backup@example.com");
    }
}
