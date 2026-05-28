use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    models::kb::{
        ChunkListResponse, TextUploadRequest, UploadFileResult, UploadResponse, UploadedDocument,
    },
    services::kb_ingest::IngestError,
    state::AppState,
};

pub async fn upload_knowledge_base(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let mut files = Vec::new();
    let mut documents_processed = 0usize;
    let mut chunks_created = 0usize;
    let mut had_internal_error = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        let Some(file_name) = field.file_name().map(str::to_string) else {
            continue;
        };

        let bytes = field.bytes().await.map_err(|error| {
            ApiError::bad_request(format!("failed to read {file_name}: {error}"))
        })?;

        match state
            .ingest
            .ingest_document(UploadedDocument {
                source_name: file_name.clone(),
                bytes: bytes.to_vec(),
                tags: Vec::new(),
            })
            .await
        {
            Ok(summary) => {
                tracing::info!(
                    source_name = %summary.source_name,
                    document_id = %summary.document_id,
                    deleted_chunks = summary.deleted_chunks,
                    chunk_count = summary.chunk_count,
                    "ingested uploaded knowledge-base file"
                );

                documents_processed += 1;
                chunks_created += summary.chunk_count;
                files.push(UploadFileResult {
                    source_name: summary.source_name,
                    chunk_count: summary.chunk_count,
                    error: None,
                });
            }
            Err(IngestError::Validation(message)) => {
                tracing::warn!(source_name = %file_name, error = %message, "rejected uploaded file");
                files.push(UploadFileResult {
                    source_name: file_name,
                    chunk_count: 0,
                    error: Some(message),
                });
            }
            Err(IngestError::Internal(error)) => {
                had_internal_error = true;
                tracing::error!(source_name = %file_name, error = ?error, "knowledge-base ingestion failed");
                files.push(UploadFileResult {
                    source_name: file_name,
                    chunk_count: 0,
                    error: Some(error.to_string()),
                });
            }
        }
    }

    if files.is_empty() {
        return Err(ApiError::bad_request(
            "no files were found in the upload request",
        ));
    }

    let status = if documents_processed == files.len() {
        "success"
    } else if documents_processed > 0 {
        "partial_success"
    } else {
        "error"
    };

    let message = match status {
        "success" => format!(
            "processed {} document(s) into {} chunk(s)",
            documents_processed, chunks_created
        ),
        "partial_success" => format!(
            "processed {} document(s) into {} chunk(s); some files failed",
            documents_processed, chunks_created
        ),
        _ if had_internal_error => "knowledge-base ingestion failed".to_string(),
        _ => "no documents were ingested".to_string(),
    };

    tracing::info!(
        collection = %state.chroma.collection_name(),
        status,
        total_files = files.len(),
        documents_processed,
        chunks_created,
        "knowledge-base upload finished"
    );

    let response = UploadResponse {
        status: status.to_string(),
        message,
        collection: state.chroma.collection_name().to_string(),
        documents_processed,
        chunks_created,
        files,
    };

    let status_code = if documents_processed > 0 {
        StatusCode::OK
    } else if had_internal_error {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::BAD_REQUEST
    };

    Ok((status_code, Json(response)))
}

pub async fn upload_text(
    State(state): State<AppState>,
    Json(request): Json<TextUploadRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let source_name = request.source_name.trim();
    if source_name.is_empty() {
        return Err(ApiError::bad_request("source_name is required"));
    }
    if request.text.trim().is_empty() {
        return Err(ApiError::bad_request("text is empty"));
    }

    let source_name = format!("text:{source_name}");

    match state
        .ingest
        .ingest_text(source_name.clone(), request.text, request.tags)
        .await
    {
        Ok(summary) => {
            tracing::info!(
                source_name = %summary.source_name,
                document_id = %summary.document_id,
                chunk_count = summary.chunk_count,
                "ingested text input"
            );

            let response = UploadResponse {
                status: "success".to_string(),
                message: format!(
                    "processed 1 document into {} chunk(s)",
                    summary.chunk_count
                ),
                collection: state.chroma.collection_name().to_string(),
                documents_processed: 1,
                chunks_created: summary.chunk_count,
                files: vec![UploadFileResult {
                    source_name: summary.source_name,
                    chunk_count: summary.chunk_count,
                    error: None,
                }],
            };

            Ok((StatusCode::OK, Json(response)))
        }
        Err(IngestError::Validation(message)) => Err(ApiError::bad_request(message)),
        Err(IngestError::Internal(error)) => {
            tracing::error!(source_name = %source_name, error = ?error, "text ingestion failed");
            Err(ApiError::internal(error.to_string()))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ChunkListParams {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_page_size")]
    page_size: usize,
    pub source: Option<String>,
}

fn default_page() -> usize {
    1
}
fn default_page_size() -> usize {
    20
}

pub async fn list_chunks(
    State(state): State<AppState>,
    Query(params): Query<ChunkListParams>,
) -> Result<Json<ChunkListResponse>, ApiError> {
    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, 100);
    let source_filter = params.source.as_deref();

    let total = state
        .chroma
        .count_chunks(source_filter)
        .await
        .map_err(|error| ApiError::internal(format!("failed to count chunks: {error}")))?;

    let offset = (page - 1) * page_size;
    let items = state
        .chroma
        .list_chunks_paginated(offset, page_size, source_filter)
        .await
        .map_err(|error| ApiError::internal(format!("failed to load chunks: {error}")))?;

    Ok(Json(ChunkListResponse {
        total,
        page,
        page_size,
        items,
    }))
}

#[derive(Debug, Deserialize)]
pub struct DeleteChunksRequest {
    pub ids: Option<Vec<String>>,
    pub source: Option<String>,
}

pub async fn delete_chunks(
    State(state): State<AppState>,
    Json(request): Json<DeleteChunksRequest>,
) -> Result<Json<Value>, ApiError> {
    let deleted = if let Some(ids) = request.ids {
        if ids.is_empty() {
            return Err(ApiError::bad_request("ids cannot be empty"));
        }
        state
            .chroma
            .delete_chunks_by_ids(&ids)
            .await
            .map_err(|error| ApiError::internal(format!("failed to delete chunks: {error}")))?
    } else if let Some(source) = request.source {
        if source.trim().is_empty() {
            return Err(ApiError::bad_request("source cannot be empty"));
        }
        state
            .chroma
            .delete_chunks_by_source(&source)
            .await
            .map_err(|error| ApiError::internal(format!("failed to delete chunks by source: {error}")))?
    } else {
        return Err(ApiError::bad_request("either ids or source must be provided"));
    };

    tracing::info!(deleted_chunks = deleted, "deleted chunks");
    Ok(Json(json!({ "deleted": deleted })))
}

pub async fn list_sources(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    state
        .chroma
        .list_sources()
        .await
        .map_err(|error| ApiError::internal(format!("failed to list sources: {error}")))
        .map(Json)
}

#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "status": "error",
                "message": self.message,
            })),
        )
            .into_response()
    }
}
