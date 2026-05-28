use std::sync::Arc;

use anyhow::Result;

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
        let chroma = Arc::new(ChromaService::new(
            http_client.clone(),
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
