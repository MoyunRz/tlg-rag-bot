use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct TextUploadRequest {
    pub source_name: String,
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UploadedDocument {
    pub source_name: String,
    pub bytes: Vec<u8>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ChunkRecord {
    pub id: String,
    pub source_name: String,
    pub document_id: String,
    pub chunk_index: usize,
    pub text: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadFileResult {
    pub source_name: String,
    pub chunk_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadResponse {
    pub status: String,
    pub message: String,
    pub collection: String,
    pub documents_processed: usize,
    pub chunks_created: usize,
    pub files: Vec<UploadFileResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChunkItem {
    pub id: String,
    pub source_name: String,
    pub text: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChunkListResponse {
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub items: Vec<ChunkItem>,
}

#[derive(Debug, Clone)]
pub struct DocumentIngestSummary {
    pub source_name: String,
    pub document_id: String,
    pub chunk_count: usize,
    pub deleted_chunks: usize,
}

#[derive(Debug, Clone)]
pub struct EmbeddingBatch {
    pub vectors: Vec<Vec<f32>>,
    pub dimension: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetrievedChunk {
    pub id: String,
    pub source_name: String,
    pub document_id: String,
    pub chunk_index: usize,
    pub text: String,
    pub tags: Vec<String>,
    pub score: f32,
    pub distance: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RagTimings {
    pub embed: u64,
    pub retrieve: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate: Option<u64>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RagCitation {
    pub index: usize,
    pub source_name: String,
    pub document_id: String,
    pub chunk_index: usize,
    pub excerpt: String,
    pub tags: Vec<String>,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RagAnswer {
    pub question: String,
    pub faq_hit: bool,
    pub retrieved_chunks: Vec<String>,
    pub retrieved_chunk_items: Vec<RetrievedChunk>,
    pub final_answer: String,
    pub provider: String,
    pub model: String,
    pub top_k: usize,
    pub retrieval_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<RagCitation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timings_ms: Option<RagTimings>,
}
