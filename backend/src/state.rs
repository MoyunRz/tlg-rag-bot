use std::sync::Arc;

use anyhow::{Context, Result};

use crate::{
    config::AppConfig,
    services::{
        chroma::ChromaService, embed::EmbeddingService, kb_ingest::KbIngestService,
        llm::LlmService, ocr::OcrService, rag::RagService,
    },
};

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: AppConfig,
    pub chroma: Arc<ChromaService>,
    pub ingest: Arc<KbIngestService>,
    pub rag: Arc<RagService>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Result<Self> {
        let http_client = reqwest::Client::new();
        let chroma_http_client = build_chroma_client(&config.chroma_url)?;
        let chroma = Arc::new(ChromaService::new(
            chroma_http_client,
            config.chroma_url.clone(),
            config.chroma_collection.clone(),
        ));
        let embedder = Arc::new(EmbeddingService::try_new(
            config.kb_ingestion.embedding_model.clone(),
            config.kb_ingestion.embed_batch_size,
        )?);
        let ocr = config
            .ocr
            .enabled
            .then(|| Arc::new(OcrService::new(http_client.clone(), config.ocr.clone())));
        let llm = Arc::new(LlmService::new(http_client, config.llm.clone()));
        let ingest = Arc::new(KbIngestService::new(
            config.kb_ingestion.clone(),
            chroma.clone(),
            embedder.clone(),
            ocr,
        ));
        let rag = Arc::new(RagService::new(
            config.rag.clone(),
            chroma.clone(),
            embedder,
            llm,
        ));

        Ok(Self {
            config,
            chroma,
            ingest,
            rag,
        })
    }
}

fn build_chroma_client(chroma_url: &str) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder();

    if should_bypass_proxy(chroma_url) {
        builder = builder.no_proxy();
    }

    builder
        .build()
        .context("failed to build Chroma HTTP client")
}

fn should_bypass_proxy(chroma_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(chroma_url) else {
        return false;
    };

    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };

    matches!(host.as_str(), "localhost" | "0.0.0.0" | "::1" | "[::1]") || host.starts_with("127.")
}

#[cfg(test)]
mod tests {
    use super::should_bypass_proxy;

    #[test]
    fn bypasses_proxy_for_local_chroma_urls() {
        assert!(should_bypass_proxy("http://127.0.0.1:8600"));
        assert!(should_bypass_proxy("http://127.42.0.1:8600"));
        assert!(should_bypass_proxy("http://localhost:8600"));
        assert!(should_bypass_proxy("http://[::1]:8600"));
        assert!(should_bypass_proxy("http://0.0.0.0:8600"));
    }

    #[test]
    fn keeps_proxy_for_non_local_chroma_urls() {
        assert!(!should_bypass_proxy("http://chroma:8000"));
        assert!(!should_bypass_proxy("https://example.com"));
        assert!(!should_bypass_proxy("not a url"));
    }
}
