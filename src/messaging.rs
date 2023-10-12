use log::{debug, error};
use crate::model::config::{MessagingConfig};

pub(crate) fn send_message(cfg: &Option<MessagingConfig>, msg: &str) {
    if let Some(messaging) = cfg {
        if let Some(telegram) = &messaging.telegram {
            for chat_id in &telegram.chat_ids {
                let bot = rustelebot::create_instance(&telegram.bot_token, chat_id);
                match rustelebot::send_message(&bot, msg, None)
                {
                    Ok(_) => debug!("Text message sent successfully to {}", chat_id),
                    Err(e) => error!("Text message wasn't sent to {} because of: {}", chat_id, e)
                }
            };
        }
    }
}
