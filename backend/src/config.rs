use std::env;
use std::path::Path;

use anyhow::{ensure, Context, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct AppConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub telegram: TelegramConfig,
    #[serde(default)]
    pub chroma: ChromaConfig,
    #[serde(default)]
    pub kb_ingestion: KbIngestionConfig,
    #[serde(default)]
    pub ocr: OcrConfig,
    pub llm: LlmConfig,
    pub rag: RagConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TelegramConfig {
    #[serde(default = "default_telegram_enabled")]
    pub enabled: bool,
    pub bot_token: Option<String>,
}

fn default_telegram_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChromaConfig {
    pub url: Option<String>,
    pub collection: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct KbIngestionConfig {
    pub embedding_model: Option<String>,
    pub chunk_size: Option<usize>,
    pub chunk_overlap: Option<usize>,
    pub embed_batch_size: Option<usize>,
    pub max_upload_bytes: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OcrConfig {
    #[serde(default)]
    pub enabled: bool,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub timeout_ms: Option<u64>,
    pub max_tokens: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LlmConfig {
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub timeout_ms: Option<u64>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RagConfig {
    pub top_k: Option<usize>,
    pub score_threshold: Option<f32>,
    pub max_context_chars: Option<usize>,
    #[serde(default)]
    pub bot_prompt: BotPromptConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BotPromptConfig {
    pub system: Option<String>,
    pub user_template: Option<String>,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bot_token: None,
        }
    }
}

impl Default for ChromaConfig {
    fn default() -> Self {
        Self {
            url: None,
            collection: None,
        }
    }
}

impl Default for KbIngestionConfig {
    fn default() -> Self {
        Self {
            embedding_model: None,
            chunk_size: None,
            chunk_overlap: None,
            embed_batch_size: None,
            max_upload_bytes: None,
        }
    }
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: None,
            api_key: None,
            model: None,
            timeout_ms: None,
            max_tokens: None,
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: None,
            base_url: None,
            api_key: None,
            model: None,
            timeout_ms: None,
            max_tokens: None,
            temperature: None,
        }
    }
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            top_k: None,
            score_threshold: None,
            max_context_chars: None,
            bot_prompt: BotPromptConfig::default(),
        }
    }
}

impl Default for BotPromptConfig {
    fn default() -> Self {
        Self {
            system: None,
            user_template: None,
        }
    }
}

// 解析后的运行时配置
#[derive(Clone, Debug)]
pub struct AppConfigInternal {
    pub host: String,
    pub port: u16,
    pub telegram_enabled: bool,
    pub telegram_bot_token: String,
    pub chroma_url: String,
    pub chroma_collection: String,
    pub kb_ingestion: KbIngestionConfigInternal,
    pub ocr: OcrConfigInternal,
    pub llm: LlmConfigInternal,
    pub rag: RagConfigInternal,
}

#[derive(Clone, Debug)]
pub struct KbIngestionConfigInternal {
    pub embedding_model: String,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub embed_batch_size: usize,
    pub max_upload_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct OcrConfigInternal {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: u64,
    pub max_tokens: usize,
}

#[derive(Clone, Debug)]
pub struct LlmConfigInternal {
    pub provider: LlmProvider,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: u64,
    pub max_tokens: usize,
    pub temperature: f32,
}

#[derive(Clone, Debug)]
pub enum LlmProvider {
    Minimax,
}

impl LlmProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Minimax => "minimax",
        }
    }

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "minimax" => Ok(Self::Minimax),
            other => anyhow::bail!("unsupported LLM_PROVIDER: {other}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RagConfigInternal {
    pub top_k: usize,
    pub score_threshold: Option<f32>,
    pub max_context_chars: usize,
    pub bot_prompt: BotPromptConfigInternal,
}

#[derive(Clone, Debug)]
pub struct BotPromptConfigInternal {
    pub system: String,
    pub user_template: String,
}

impl AppConfig {
    /// 从配置文件和环境变量加载配置
    /// 环境变量优先级高于 YAML 配置
    pub fn load(config_path: &Path) -> Result<AppConfigInternal> {
        // 加载 dotenv
        let _ = dotenvy::dotenv();

        // 读取 YAML 配置
        let yaml_config = if config_path.exists() {
            let content = std::fs::read_to_string(config_path)
                .with_context(|| format!("failed to read config file: {}", config_path.display()))?;
            serde_yaml::from_str::<AppConfig>(&content)
                .with_context(|| format!("failed to parse config file: {}", config_path.display()))?
        } else {
            tracing::warn!("config file not found at {}, using defaults from env vars", config_path.display());
            AppConfig {
                host: None,
                port: None,
                telegram: TelegramConfig::default(),
                chroma: ChromaConfig::default(),
                kb_ingestion: KbIngestionConfig::default(),
                ocr: OcrConfig::default(),
                llm: LlmConfig::default(),
                rag: RagConfig::default(),
            }
        };

        yaml_config.into_internal()
    }
}

impl AppConfig {
    fn into_internal(self) -> Result<AppConfigInternal> {
        let chunk_size = self.kb_ingestion.chunk_size.unwrap_or(500);
        let chunk_overlap = self
            .kb_ingestion
            .chunk_overlap
            .unwrap_or(100)
            .min(chunk_size.saturating_sub(1));
        let embed_batch_size = self.kb_ingestion.embed_batch_size.unwrap_or(1);

        let llm_provider_str = env::var("LLM_PROVIDER")
            .unwrap_or_else(|_| self.llm.provider.unwrap_or_else(|| "minimax".to_string()));
        let llm_provider = LlmProvider::from_str(&llm_provider_str)?;

        let telegram_enabled = env::var("TELEGRAM_ENABLED")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(self.telegram.enabled);

        let telegram_bot_token = env::var("TELEGRAM_BOT_TOKEN")
            .unwrap_or_else(|_| self.telegram.bot_token.clone().unwrap_or_default());

        let chroma_collection = env::var("CHROMA_COLLECTION")
            .unwrap_or_else(|_| self.chroma.collection.clone().unwrap_or_default());

        let embedding_model = env::var("KB_EMBEDDING_MODEL")
            .unwrap_or_else(|_| self.kb_ingestion.embedding_model.clone().unwrap_or_default());

        let llm_api_key = env::var("LLM_API_KEY")
            .unwrap_or_else(|_| self.llm.api_key.clone().unwrap_or_default());
        ensure!(
            !llm_api_key.trim().is_empty(),
            "LLM_API_KEY is required for Minimax RAG generation"
        );

        let ocr_api_key = env::var("OCR_API_KEY")
            .unwrap_or_else(|_| self.ocr.api_key.clone().unwrap_or_default());
        let ocr_enabled = self.ocr.enabled || !ocr_api_key.trim().is_empty();

        let ocr_base_url = env::var("OCR_BASE_URL")
            .unwrap_or_else(|_| self.ocr.base_url.clone().unwrap_or_default());

        let ocr_model = env::var("OCR_MODEL")
            .unwrap_or_else(|_| self.ocr.model.clone().unwrap_or_default());

        let ocr_timeout_ms = self.ocr.timeout_ms.unwrap_or(60_000);
        let ocr_max_tokens = self.ocr.max_tokens.unwrap_or(4_096);

        let rag_top_k = self.rag.top_k.unwrap_or(4);
        let rag_score_threshold = self.rag.score_threshold;
        let rag_max_context_chars = self.rag.max_context_chars.unwrap_or(4_000);

        // Bot prompt 配置
        let default_system_prompt = "你是一个中文客服知识库助手。你只能根据提供的知识库片段回答用户问题。如果知识库里没有足够信息，请明确说明当前知识库不足以判断，并建议用户联系人工客服。不要编造政策、时效、价格或承诺。输出要求：使用简洁、稳定、Telegram 可渲染的 Markdown；优先使用短段落、列表、`**加粗**`、`` `行内代码` `` 和 fenced code block；关键结论尽量用 [1]、[2] 这类编号引用对应知识库片段；禁止在答案中提及\"来源\"、\"来源信息\"、\"出自\"、\"根据哪篇\"等文字，也不要暴露片段来源名称；保证普通纯文本阅读也自然，不要过度排版。".to_string();
        let default_user_template = "用户问题：\n{question}\n\n知识库片段：\n{context}\n\n请严格遵守以下要求：\n1. 只根据以上知识库片段回答，不要补充片段之外的事实。\n2. 用简洁、Telegram 可直接渲染的 Markdown 作答，优先使用短段落、列表、`**加粗**`、`` `行内代码` `` 和 fenced code block。\n3. 关键结论尽量在句末标注对应片段编号，例如 [1]、[2]。\n4. 禁止在答案中提及\"来源\"、\"来源信息\"、\"出自\"、\"根据哪篇\"、\"来自\"、\"在知识库中\"等字样，不要暴露任何片段的来源名称。\n5. 避免表格、复杂嵌套和不稳定 Markdown 语法；不要过度排版。\n6. 如果片段不足，请直接说明知识库不足，不要编造。".to_string();

        let bot_prompt_system = env::var("BOT_PROMPT_SYSTEM")
            .unwrap_or_else(|_| self.rag.bot_prompt.system.clone().unwrap_or_default());
        let bot_prompt_system = if bot_prompt_system.is_empty() {
            default_system_prompt
        } else {
            bot_prompt_system
        };

        let bot_prompt_user_template = env::var("BOT_PROMPT_USER_TEMPLATE")
            .unwrap_or_else(|_| self.rag.bot_prompt.user_template.clone().unwrap_or_default());
        let bot_prompt_user_template = if bot_prompt_user_template.is_empty() {
            default_user_template
        } else {
            bot_prompt_user_template
        };

        // 验证
        ensure!(chunk_size > 0, "KB_CHUNK_SIZE must be greater than 0");
        ensure!(embed_batch_size > 0, "KB_EMBED_BATCH_SIZE must be greater than 0");
        ensure!(!chroma_collection.trim().is_empty(), "CHROMA_COLLECTION must not be empty");
        ensure!(!embedding_model.trim().is_empty(), "KB_EMBEDDING_MODEL must not be empty");
        ensure!(rag_top_k > 0, "RAG_TOP_K must be greater than 0");
        ensure!(!telegram_enabled || !telegram_bot_token.trim().is_empty(),
            "TELEGRAM_BOT_TOKEN is required when TELEGRAM_ENABLED=true");
        ensure!(rag_max_context_chars > 0, "RAG_MAX_CONTEXT_CHARS must be greater than 0");
        ensure!(ocr_timeout_ms > 0, "OCR_TIMEOUT_MS must be greater than 0");
        ensure!(ocr_max_tokens > 0, "OCR_MAX_TOKENS must be greater than 0");

        if let Some(score_threshold) = rag_score_threshold {
            ensure!(
                (0.0..=1.0).contains(&score_threshold),
                "RAG_SCORE_THRESHOLD must be between 0.0 and 1.0"
            );
        }

        if ocr_enabled {
            ensure!(!ocr_api_key.trim().is_empty(), "OCR_API_KEY is required when OCR_ENABLED=true");
            ensure!(!ocr_base_url.trim().is_empty(), "OCR_BASE_URL must not be empty when OCR_ENABLED=true");
            ensure!(!ocr_model.trim().is_empty(), "OCR_MODEL must not be empty when OCR_ENABLED=true");
        }

        Ok(AppConfigInternal {
            host: env::var("APP_HOST")
                .unwrap_or_else(|_| self.host.unwrap_or_else(|| "127.0.0.1".to_string())),
            port: env::var("APP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(self.port.unwrap_or(4000)),
            telegram_enabled,
            telegram_bot_token,
            chroma_url: env::var("CHROMA_URL")
                .unwrap_or_else(|_| self.chroma.url.unwrap_or_else(|| "http://127.0.0.1:8600".to_string())),
            chroma_collection,
            kb_ingestion: KbIngestionConfigInternal {
                embedding_model,
                chunk_size,
                chunk_overlap,
                embed_batch_size,
                max_upload_bytes: self.kb_ingestion.max_upload_bytes.unwrap_or(10 * 1024 * 1024),
            },
            ocr: OcrConfigInternal {
                enabled: ocr_enabled,
                base_url: ocr_base_url,
                api_key: ocr_api_key,
                model: ocr_model,
                timeout_ms: ocr_timeout_ms,
                max_tokens: ocr_max_tokens,
            },
            llm: LlmConfigInternal {
                provider: llm_provider,
                base_url: env::var("LLM_BASE_URL")
                    .unwrap_or_else(|_| self.llm.base_url.unwrap_or_else(|| "https://api.minimaxi.com/anthropic".to_string())),
                api_key: llm_api_key,
                model: env::var("LLM_MODEL")
                    .unwrap_or_else(|_| self.llm.model.unwrap_or_else(|| "MiniMax-M2.7".to_string())),
                timeout_ms: self.llm.timeout_ms.unwrap_or(30_000),
                max_tokens: self.llm.max_tokens.unwrap_or(512),
                temperature: self.llm.temperature.unwrap_or(0.2),
            },
            rag: RagConfigInternal {
                top_k: rag_top_k,
                score_threshold: rag_score_threshold,
                max_context_chars: rag_max_context_chars,
                bot_prompt: BotPromptConfigInternal {
                    system: bot_prompt_system,
                    user_template: bot_prompt_user_template,
                },
            },
        })
    }
}

impl AppConfigInternal {
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_config_loads_bot_prompt() {
        let yaml = r#"
llm:
  api_key: test-key
rag:
  bot_prompt:
    system: "自定义系统提示"
    user_template: "自定义用户模板: {question}"
"#;
        let config: AppConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.rag.bot_prompt.system, Some("自定义系统提示".to_string()));
        assert_eq!(config.rag.bot_prompt.user_template, Some("自定义用户模板: {question}".to_string()));
    }

    #[test]
    fn yaml_config_with_minimal_fields() {
        let yaml = r#"
llm:
  api_key: test-key
"#;
        let config: AppConfig = serde_yaml::from_str(yaml).unwrap();
        // 应该使用默认值
        assert!(config.rag.bot_prompt.system.is_none());
        assert!(config.rag.bot_prompt.user_template.is_none());
    }
}