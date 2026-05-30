use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{LlmConfigInternal, LlmProvider};

#[derive(Debug, Clone)]
pub struct LlmService {
    client: reqwest::Client,
    config: LlmConfigInternal,
}

impl LlmService {
    pub fn new(client: reqwest::Client, config: LlmConfigInternal) -> Self {
        Self { client, config }
    }

    pub fn provider(&self) -> &'static str {
        self.config.provider.as_str()
    }

    pub fn model(&self) -> &str {
        &self.config.model
    }

    pub async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        match self.config.provider {
            LlmProvider::Minimax => {
                self.chat_anthropic_compatible(system_prompt, user_prompt)
                    .await
            }
        }
    }

    async fn chat_anthropic_compatible(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String> {
        let request = AnthropicMessagesRequest {
            model: self.config.model.clone(),
            system: system_prompt.to_string(),
            messages: vec![AnthropicRequestMessage {
                role: "user".to_string(),
                content: vec![AnthropicTextBlock {
                    kind: "text".to_string(),
                    text: user_prompt.to_string(),
                }],
            }],
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
        };

        let response = self
            .client
            .post(anthropic_messages_url(&self.config.base_url))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .timeout(Duration::from_millis(self.config.timeout_ms))
            .json(&request)
            .send()
            .await
            .context("failed to call LLM provider")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read LLM response body")?;

        if !status.is_success() {
            return Err(anyhow!(
                "LLM request failed with status {}: {}",
                status,
                body
            ));
        }

        let payload: AnthropicMessagesResponse = serde_json::from_str(&body)
            .with_context(|| format!("failed to decode LLM response: {body}"))?;

        extract_anthropic_text(payload)
    }
}

#[derive(Debug, Serialize)]
struct AnthropicMessagesRequest {
    model: String,
    system: String,
    messages: Vec<AnthropicRequestMessage>,
    temperature: f32,
    max_tokens: usize,
}

#[derive(Debug, Serialize)]
struct AnthropicRequestMessage {
    role: String,
    content: Vec<AnthropicTextBlock>,
}

#[derive(Debug, Serialize)]
struct AnthropicTextBlock {
    #[serde(rename = "type")]
    kind: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessagesResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

fn anthropic_messages_url(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.ends_with("/v1") {
        format!("{base_url}/messages")
    } else {
        format!("{base_url}/v1/messages")
    }
}

fn extract_anthropic_text(payload: AnthropicMessagesResponse) -> Result<String> {
    let text = payload
        .content
        .into_iter()
        .filter(|block| block.kind == "text")
        .filter_map(|block| block.text)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if text.is_empty() {
        return Err(anyhow!("LLM response did not contain any text blocks"));
    }

    let cleaned = strip_think_tags(&text);
    if cleaned.is_empty() {
        return Err(anyhow!("LLM response content was empty"));
    }

    Ok(cleaned)
}

fn strip_think_tags(content: &str) -> String {
    let mut cleaned = String::new();
    let mut remaining = content;

    while let Some(start) = remaining.find("<think>") {
        cleaned.push_str(&remaining[..start]);
        let after_start = &remaining[start + "<think>".len()..];
        if let Some(end) = after_start.find("</think>") {
            remaining = &after_start[end + "</think>".len()..];
        } else {
            remaining = "";
            break;
        }
    }

    cleaned.push_str(remaining);
    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{anthropic_messages_url, extract_anthropic_text, AnthropicMessagesResponse};

    #[test]
    fn anthropic_messages_url_appends_v1_for_anthropic_base() {
        assert_eq!(
            anthropic_messages_url("https://api.minimaxi.com/anthropic"),
            "https://api.minimaxi.com/anthropic/v1/messages"
        );
    }

    #[test]
    fn anthropic_messages_url_reuses_existing_v1_suffix() {
        assert_eq!(
            anthropic_messages_url("https://api.minimaxi.com/anthropic/v1"),
            "https://api.minimaxi.com/anthropic/v1/messages"
        );
    }

    #[test]
    fn extract_anthropic_text_reads_text_blocks() {
        let payload: AnthropicMessagesResponse = serde_json::from_str(
            r#"{
                "content": [
                    {"type": "thinking", "thinking": "..."},
                    {"type": "text", "text": "hello"},
                    {"type": "text", "text": "world"}
                ]
            }"#,
        )
        .expect("payload should decode");

        let text = extract_anthropic_text(payload).expect("should extract text blocks");
        assert_eq!(text, "hello\n\nworld");
    }

    #[test]
    fn extract_anthropic_text_rejects_missing_text_blocks() {
        let payload: AnthropicMessagesResponse = serde_json::from_str(
            r#"{
                "content": [
                    {"type": "thinking", "thinking": "..."}
                ]
            }"#,
        )
        .expect("payload should decode");

        let error = extract_anthropic_text(payload).expect_err("should reject empty text content");
        assert!(error
            .to_string()
            .contains("LLM response did not contain any text blocks"));
    }
}
