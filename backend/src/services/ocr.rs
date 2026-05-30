use std::{path::Path, time::Duration};

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use serde::{Deserialize, Serialize};

use crate::config::OcrConfigInternal;

const OCR_NO_TEXT_SENTINEL: &str = "[[NO_TEXT]]";
const OCR_SYSTEM_PROMPT: &str = "You are an OCR engine. Extract only the readable text from the provided file. Preserve the original reading order. Do not summarize, answer questions, translate, or infer missing words. If there is no readable text, return exactly [[NO_TEXT]].";
const OCR_USER_PROMPT: &str =
    "Extract the readable text from this file and return only the extracted text.";

#[derive(Debug, Clone)]
pub struct OcrService {
    client: reqwest::Client,
    config: OcrConfigInternal,
}

impl OcrService {
    pub fn new(client: reqwest::Client, config: OcrConfigInternal) -> Self {
        Self { client, config }
    }

    pub fn model(&self) -> &str {
        &self.config.model
    }

    pub async fn extract_text(&self, source_name: &str, bytes: &[u8]) -> Result<String> {
        let media_type = media_type_for_source(source_name)?;
        self.extract_text_from_media(source_name, bytes, media_type)
            .await
    }

    pub async fn extract_text_from_media(
        &self,
        source_name: &str,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<String> {
        let block_type = if media_type.starts_with("image/") {
            "image"
        } else {
            "document"
        };
        let request = AnthropicMessagesRequest {
            model: self.config.model.clone(),
            system: OCR_SYSTEM_PROMPT.to_string(),
            messages: vec![AnthropicRequestMessage {
                role: "user".to_string(),
                content: vec![
                    AnthropicInputBlock {
                        kind: "text".to_string(),
                        text: Some(OCR_USER_PROMPT.to_string()),
                        source: None,
                    },
                    AnthropicInputBlock {
                        kind: block_type.to_string(),
                        text: None,
                        source: Some(AnthropicBase64Source {
                            kind: "base64".to_string(),
                            media_type: media_type.to_string(),
                            data: BASE64_STANDARD.encode(bytes),
                        }),
                    },
                ],
            }],
            temperature: 0.0,
            max_tokens: self.config.max_tokens,
        };

        let response = self
            .client
            .post(anthropic_messages_url(&self.config.base_url))
            .header("api-key", &self.config.api_key)
            .header("authorization", format!("Bearer {}", self.config.api_key))
            .header("anthropic-version", "2023-06-01")
            .timeout(Duration::from_millis(self.config.timeout_ms))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("failed to call OCR provider for {source_name}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .with_context(|| format!("failed to read OCR response body for {source_name}"))?;

        if !status.is_success() {
            return Err(anyhow!(
                "OCR request failed with status {} for {}: {}",
                status,
                source_name,
                body
            ));
        }

        let payload: AnthropicMessagesResponse = serde_json::from_str(&body)
            .with_context(|| format!("failed to decode OCR response for {source_name}: {body}"))?;

        let text = extract_anthropic_text(payload)?;
        if text == OCR_NO_TEXT_SENTINEL {
            bail!("OCR result for {source_name} did not contain extractable text");
        }

        if text.is_empty() {
            bail!("OCR result for {source_name} was empty");
        }

        Ok(text)
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
    content: Vec<AnthropicInputBlock>,
}

#[derive(Debug, Serialize)]
struct AnthropicInputBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<AnthropicBase64Source>,
}

#[derive(Debug, Serialize)]
struct AnthropicBase64Source {
    #[serde(rename = "type")]
    kind: String,
    media_type: String,
    data: String,
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

fn media_type_for_source(source_name: &str) -> Result<&'static str> {
    let extension = Path::new(source_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .ok_or_else(|| anyhow!("{source_name} has no supported file extension"))?;

    match extension.as_str() {
        "png" => Ok("image/png"),
        "jpg" | "jpeg" => Ok("image/jpeg"),
        "pdf" => Ok("application/pdf"),
        "doc" => Ok("application/msword"),
        "docx" => Ok("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        other => bail!("unsupported OCR media type for {source_name}: .{other}"),
    }
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
        return Err(anyhow!("OCR response did not contain any text blocks"));
    }

    Ok(strip_think_tags(&text))
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
    use super::{
        anthropic_messages_url, extract_anthropic_text, media_type_for_source,
        AnthropicMessagesResponse,
    };

    #[test]
    fn anthropic_messages_url_appends_v1_for_base_url() {
        assert_eq!(
            anthropic_messages_url("https://api.xiaomimimo.com/anthropic"),
            "https://api.xiaomimimo.com/anthropic/v1/messages"
        );
    }

    #[test]
    fn media_type_for_source_maps_supported_extensions() {
        assert_eq!(media_type_for_source("scan.png").unwrap(), "image/png");
        assert_eq!(
            media_type_for_source("paper.pdf").unwrap(),
            "application/pdf"
        );
        assert_eq!(
            media_type_for_source("report.docx").unwrap(),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
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
}
