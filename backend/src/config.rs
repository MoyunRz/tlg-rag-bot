use std::env;

use anyhow::{ensure, Context, Result};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub telegram_enabled: bool,
    pub telegram_bot_token: String,
    pub chroma_url: String,
    pub chroma_collection: String,
    pub kb_ingestion: KbIngestionConfig,
    pub ocr: OcrConfig,
    pub llm: LlmConfig,
    pub rag: RagConfig,
}

#[derive(Clone, Debug)]
pub struct KbIngestionConfig {
    pub embedding_model: String,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub embed_batch_size: usize,
    pub max_upload_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct OcrConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: u64,
    pub max_tokens: usize,
}

#[derive(Clone, Debug)]
pub struct LlmConfig {
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

    fn from_env(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "minimax" => Ok(Self::Minimax),
            other => anyhow::bail!("unsupported LLM_PROVIDER: {other}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RagConfig {
    pub top_k: usize,
    pub score_threshold: Option<f32>,
    pub max_context_chars: usize,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let chunk_size = read_env_usize("KB_CHUNK_SIZE", 500);
        let chunk_overlap =
            read_env_usize("KB_CHUNK_OVERLAP", 100).min(chunk_size.saturating_sub(1));
        let embed_batch_size = read_env_usize("KB_EMBED_BATCH_SIZE", 1);
        let llm_provider = LlmProvider::from_env(
            &env::var("LLM_PROVIDER").unwrap_or_else(|_| "minimax".to_string()),
        )?;
        let telegram_enabled = read_env_bool("TELEGRAM_ENABLED", true)?;
        let telegram_bot_token = env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default();
        let chroma_collection =
            env::var("CHROMA_COLLECTION").unwrap_or_else(|_| "tlg-rag-bot-model2vec".to_string());
        let embedding_model = env::var("KB_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "minishlab/potion-multilingual-128M".to_string());
        let llm_api_key = env::var("LLM_API_KEY")
            .context("LLM_API_KEY is required for Minimax RAG generation")?;
        let ocr_api_key = env::var("OCR_API_KEY").unwrap_or_default();
        let ocr_enabled = read_env_bool("OCR_ENABLED", !ocr_api_key.trim().is_empty())?;
        let ocr_base_url = env::var("OCR_BASE_URL")
            .unwrap_or_else(|_| "https://api.xiaomimimo.com/anthropic".to_string());
        let ocr_model = env::var("OCR_MODEL").unwrap_or_else(|_| "mimo-v2.5".to_string());
        let ocr_timeout_ms = read_env_u64("OCR_TIMEOUT_MS", 60_000);
        let ocr_max_tokens = read_env_usize("OCR_MAX_TOKENS", 4_096);
        let rag_top_k = read_env_usize("RAG_TOP_K", 4);
        let rag_score_threshold = read_env_f32_optional("RAG_SCORE_THRESHOLD")?;
        let rag_max_context_chars = read_env_usize("RAG_MAX_CONTEXT_CHARS", 4_000);

        ensure!(chunk_size > 0, "KB_CHUNK_SIZE must be greater than 0");
        ensure!(
            embed_batch_size > 0,
            "KB_EMBED_BATCH_SIZE must be greater than 0"
        );
        ensure!(
            !chroma_collection.trim().is_empty(),
            "CHROMA_COLLECTION must not be empty"
        );
        ensure!(
            !embedding_model.trim().is_empty(),
            "KB_EMBEDDING_MODEL must not be empty"
        );
        ensure!(rag_top_k > 0, "RAG_TOP_K must be greater than 0");
        ensure!(
            !telegram_enabled || !telegram_bot_token.trim().is_empty(),
            "TELEGRAM_BOT_TOKEN is required when TELEGRAM_ENABLED=true"
        );
        ensure!(
            rag_max_context_chars > 0,
            "RAG_MAX_CONTEXT_CHARS must be greater than 0"
        );
        ensure!(ocr_timeout_ms > 0, "OCR_TIMEOUT_MS must be greater than 0");
        ensure!(ocr_max_tokens > 0, "OCR_MAX_TOKENS must be greater than 0");

        if let Some(score_threshold) = rag_score_threshold {
            ensure!(
                (0.0..=1.0).contains(&score_threshold),
                "RAG_SCORE_THRESHOLD must be between 0.0 and 1.0"
            );
        }

        if ocr_enabled {
            ensure!(
                !ocr_api_key.trim().is_empty(),
                "OCR_API_KEY is required when OCR_ENABLED=true"
            );
            ensure!(
                !ocr_base_url.trim().is_empty(),
                "OCR_BASE_URL must not be empty when OCR_ENABLED=true"
            );
            ensure!(
                !ocr_model.trim().is_empty(),
                "OCR_MODEL must not be empty when OCR_ENABLED=true"
            );
        }

        Ok(Self {
            host: env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("APP_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(4000),
            telegram_enabled,
            telegram_bot_token,
            chroma_url: env::var("CHROMA_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8600".to_string()),
            chroma_collection,
            kb_ingestion: KbIngestionConfig {
                embedding_model,
                chunk_size,
                chunk_overlap,
                embed_batch_size,
                max_upload_bytes: read_env_usize("KB_MAX_UPLOAD_BYTES", 10 * 1024 * 1024),
            },
            ocr: OcrConfig {
                enabled: ocr_enabled,
                base_url: ocr_base_url,
                api_key: ocr_api_key,
                model: ocr_model,
                timeout_ms: ocr_timeout_ms,
                max_tokens: ocr_max_tokens,
            },
            llm: LlmConfig {
                provider: llm_provider,
                base_url: env::var("LLM_BASE_URL")
                    .unwrap_or_else(|_| "https://api.minimaxi.com/anthropic".to_string()),
                api_key: llm_api_key,
                model: env::var("LLM_MODEL").unwrap_or_else(|_| "MiniMax-M2.7".to_string()),
                timeout_ms: read_env_u64("LLM_TIMEOUT_MS", 30_000),
                max_tokens: read_env_usize("LLM_MAX_TOKENS", 512),
                temperature: read_env_f32("LLM_TEMPERATURE", 0.2)?,
            },
            rag: RagConfig {
                top_k: rag_top_k,
                score_threshold: rag_score_threshold,
                max_context_chars: rag_max_context_chars,
            },
        })
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn read_env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn read_env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn read_env_bool(key: &str, default: bool) -> Result<bool> {
    env::var(key)
        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => anyhow::bail!("{key} must be a boolean"),
        })
        .unwrap_or(Ok(default))
}

fn read_env_f32(key: &str, default: f32) -> Result<f32> {
    env::var(key)
        .map(|value| {
            value
                .parse()
                .with_context(|| format!("{key} must be a number"))
        })
        .unwrap_or(Ok(default))
}

fn read_env_f32_optional(key: &str) -> Result<Option<f32>> {
    env::var(key)
        .map(|value| {
            value
                .parse()
                .with_context(|| format!("{key} must be a number"))
                .map(Some)
        })
        .unwrap_or(Ok(None))
}

#[cfg(test)]
mod tests {
    use std::{env, sync::Mutex};

    use super::AppConfig;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const COMMON_ENV_KEYS: &[&str] = &[
        "TELEGRAM_ENABLED",
        "TELEGRAM_BOT_TOKEN",
        "LLM_API_KEY",
        "KB_EMBED_BATCH_SIZE",
        "OCR_ENABLED",
        "OCR_API_KEY",
        "OCR_BASE_URL",
        "OCR_MODEL",
        "OCR_TIMEOUT_MS",
        "OCR_MAX_TOKENS",
    ];

    #[test]
    fn telegram_disabled_does_not_require_token() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::new(COMMON_ENV_KEYS);

        env::set_var("TELEGRAM_ENABLED", "false");
        env::remove_var("TELEGRAM_BOT_TOKEN");
        env::set_var("LLM_API_KEY", "dummy");
        env::set_var("OCR_ENABLED", "false");
        env::remove_var("OCR_API_KEY");

        let config = AppConfig::from_env().expect("config should load when telegram is disabled");
        assert!(!config.telegram_enabled);
        assert!(config.telegram_bot_token.is_empty());
    }

    #[test]
    fn telegram_enabled_requires_token() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::new(COMMON_ENV_KEYS);

        env::set_var("TELEGRAM_ENABLED", "true");
        env::remove_var("TELEGRAM_BOT_TOKEN");
        env::set_var("LLM_API_KEY", "dummy");
        env::set_var("OCR_ENABLED", "false");
        env::remove_var("OCR_API_KEY");

        let error = AppConfig::from_env().expect_err("config should reject missing token");
        assert!(error
            .to_string()
            .contains("TELEGRAM_BOT_TOKEN is required when TELEGRAM_ENABLED=true"));
    }

    #[test]
    fn embed_batch_size_must_be_greater_than_zero() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::new(COMMON_ENV_KEYS);

        env::set_var("TELEGRAM_ENABLED", "false");
        env::remove_var("TELEGRAM_BOT_TOKEN");
        env::set_var("LLM_API_KEY", "dummy");
        env::set_var("KB_EMBED_BATCH_SIZE", "0");
        env::set_var("OCR_ENABLED", "false");
        env::remove_var("OCR_API_KEY");

        let error = AppConfig::from_env().expect_err("config should reject zero embed batch size");
        assert!(error
            .to_string()
            .contains("KB_EMBED_BATCH_SIZE must be greater than 0"));
    }

    #[test]
    fn ocr_enabled_requires_api_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::new(COMMON_ENV_KEYS);

        env::set_var("TELEGRAM_ENABLED", "false");
        env::remove_var("TELEGRAM_BOT_TOKEN");
        env::set_var("LLM_API_KEY", "dummy");
        env::set_var("OCR_ENABLED", "true");
        env::remove_var("OCR_API_KEY");

        let error = AppConfig::from_env().expect_err("config should reject missing OCR key");
        assert!(error
            .to_string()
            .contains("OCR_API_KEY is required when OCR_ENABLED=true"));
    }

    #[test]
    fn ocr_auto_enables_when_api_key_is_present() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::new(COMMON_ENV_KEYS);

        env::set_var("TELEGRAM_ENABLED", "false");
        env::remove_var("TELEGRAM_BOT_TOKEN");
        env::set_var("LLM_API_KEY", "dummy");
        env::remove_var("OCR_ENABLED");
        env::set_var("OCR_API_KEY", "dummy-ocr-key");

        let config = AppConfig::from_env().expect("config should auto-enable OCR");
        assert!(config.ocr.enabled);
        assert_eq!(config.ocr.model, "mimo-v2.5");
    }

    struct EnvRestore {
        values: Vec<(String, Option<String>)>,
    }

    impl EnvRestore {
        fn new(keys: &[&str]) -> Self {
            Self {
                values: keys
                    .iter()
                    .map(|key| ((*key).to_string(), env::var(key).ok()))
                    .collect(),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }
}
