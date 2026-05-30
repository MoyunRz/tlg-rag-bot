use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use teloxide::prelude::*;
use teloxide::payloads::SendMessageSetters;
use teloxide::types::ParseMode;
use teloxide::Bot;
use teloxide::types::Recipient;

use crate::routes::telegram::{build_telegram_reply_messages, strip_citation_markers};
use crate::state::AppState;

/// Webhook endpoint for Telegram updates
/// Path: /api/telegram/webhook
pub async fn telegram_webhook(
    State(state): State<AppState>,
    Json(update): Json<TelegramWebhookUpdate>,
) -> &'static str {
    tracing::debug!(update_id = update.update_id, "received telegram webhook update");

    // Extract message from update
    let message = match update.message {
        Some(msg) => msg,
        None => {
            tracing::debug!(update_id = update.update_id, "ignored non-message update");
            return "OK";
        }
    };

    let text = match message.text {
        Some(t) => t,
        None => {
            tracing::debug!(chat_id = ?message.chat.id, "ignored message without text");
            return "OK";
        }
    };

    let question = text.trim().to_string();
    if question.is_empty() {
        tracing::debug!(chat_id = ?message.chat.id, "ignored empty telegram text message");
        return "OK";
    }

    tracing::info!(
        chat_id = ?message.chat.id,
        update_id = update.update_id,
        "received telegram webhook message"
    );

    let bot_token = state.config.telegram_bot_token.clone();
    let chat_id = message.chat.id;

    // Spawn async task to process the update (don't block the response)
    tokio::spawn(async move {
        let reply = match state.rag.answer_question(&question).await {
            Ok(answer) => {
                tracing::info!(
                    chat_id = ?chat_id,
                    provider = %answer.provider,
                    model = %answer.model,
                    retrieval_count = answer.retrieval_count,
                    "generated telegram RAG reply via webhook"
                );
                answer.final_answer
            }
            Err(error) => {
                tracing::error!(chat_id = ?chat_id, error = ?error, "telegram RAG query failed");
                "我现在暂时无法处理这个问题，请稍后再试，或联系人工客服进一步确认。".to_string()
            }
        };

        // Send reply via Telegram Bot API
        let bot = Bot::new(bot_token);
        let reply_text = strip_citation_markers(&reply);
        let messages = build_telegram_reply_messages(&reply_text);

        for part in messages {
            let recipient: Recipient = ChatId(chat_id).into();
            if let Err(e) = bot
                .send_message(recipient, part)
                .parse_mode(ParseMode::MarkdownV2)
                .await
            {
                tracing::error!(chat_id = ?chat_id, error = ?e, "failed to send telegram reply");
            }
        }
    });

    "OK"
}

/// Telegram webhook payload structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramWebhookUpdate {
    pub update_id: u64,
    #[serde(default)]
    pub message: Option<TelegramMessage>,
    #[serde(default)]
    pub edited_message: Option<TelegramMessage>,
    #[serde(default)]
    pub channel_post: Option<TelegramMessage>,
    #[serde(default)]
    pub edited_channel_post: Option<TelegramMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramMessage {
    pub message_id: u64,
    pub chat: TelegramChat,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

/// Health check endpoint for webhook setup verification
pub async fn webhook_health() -> &'static str {
    "OK"
}