use log::{debug, error};
use reqwest::header;
use crate::model::config::{MessagingConfig};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum MsgKind {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "stats")]
    Stats,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "watch")]
    Watch,
}

fn is_enabled(kind: &MsgKind, cfg: &MessagingConfig) -> bool {
    cfg.notify_on.contains(kind)
}

pub fn send_message(kind: &MsgKind, cfg: &Option<MessagingConfig>, msg: &str) {
    if let Some(messaging) = cfg {
        if is_enabled(kind, messaging) {
            if let Some(telegram) = &messaging.telegram {
                for chat_id in &telegram.chat_ids {
                    let bot = rustelebot::create_instance(&telegram.bot_token, chat_id);
                    match rustelebot::send_message(&bot, msg, None)
                    {
                        Ok(()) => debug!("Text message sent successfully to {}", chat_id),
                        Err(e) => error!("Text message wasn't sent to {} because of: {}", chat_id, e)
                    }
                };
            }

            if let Some(rest) = &messaging.rest {
                let url = rest.url.clone();
                let data = msg.to_owned();
                actix_rt::spawn(async move {
                    let client = reqwest::Client::new();
                    match client.post(&url)
                        .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
                        .body(data)
                        .send()
                        .await {
                        Ok(_) => debug!("Text message sent successfully to rest api"),
                        Err(e) => error!("Text message wasn't sent to rest api because of: {}", e)
                    }
                });
            }
        }
    }
}

