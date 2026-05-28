use std::{path::Path, sync::Arc};

use anyhow::{bail, Context, Result as AnyResult};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    config::KbIngestionConfig,
    models::kb::{ChunkRecord, DocumentIngestSummary, UploadedDocument},
    services::{chroma::ChromaService, embed::EmbeddingService, ocr::OcrService},
};

const SUPPORTED_UPLOAD_TYPES: &str = ".txt, .md, .pdf, .png, .jpg, .jpeg, .doc, .docx";

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IngestError {
    fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}

#[derive(Debug, Clone)]
pub struct KbIngestService {
    settings: KbIngestionConfig,
    chroma: Arc<ChromaService>,
    embedder: Arc<EmbeddingService>,
    ocr: Option<Arc<OcrService>>,
}

impl KbIngestService {
    pub fn new(
        settings: KbIngestionConfig,
        chroma: Arc<ChromaService>,
        embedder: Arc<EmbeddingService>,
        ocr: Option<Arc<OcrService>>,
    ) -> Self {
        Self {
            settings,
            chroma,
            embedder,
            ocr,
        }
    }

    pub async fn ingest_document(
        &self,
        document: UploadedDocument,
    ) -> Result<DocumentIngestSummary, IngestError> {
        let source_name = document.source_name.clone();
        let prepared = self.prepare_document(document).await?;

        tracing::info!(
            source_name = %source_name,
            text_chars = prepared.text_chars,
            chunk_size = self.settings.chunk_size,
            chunk_overlap = self.settings.chunk_overlap,
            chunk_count = prepared.chunks.len(),
            "chunked knowledge-base document"
        );

        if let Some(first_chunk) = prepared.chunks.first() {
            tracing::debug!(
                source_name = %source_name,
                preview = %preview(first_chunk, 120),
                "first chunk preview"
            );
        }

        let embeddings = self
            .embedder
            .embed_chunks(&source_name, &prepared.chunks)
            .await
            .map_err(IngestError::from)?;

        if embeddings.vectors.len() != prepared.chunks.len() {
            return Err(IngestError::Internal(anyhow::anyhow!(
                "embedding count mismatch for {}",
                source_name
            )));
        }

        let document_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        let chunks = prepared
            .chunks
            .into_iter()
            .enumerate()
            .map(|(chunk_index, text)| ChunkRecord {
                id: format!("{document_id}:{chunk_index}"),
                source_name: source_name.clone(),
                document_id: document_id.clone(),
                chunk_index,
                text,
                tags: prepared.tags.clone(),
                created_at: created_at.clone(),
            })
            .collect::<Vec<_>>();

        let deleted_chunks = self
            .chroma
            .replace_source_chunks(&source_name, &chunks, &embeddings.vectors)
            .await
            .map_err(IngestError::from)?;

        tracing::info!(
            source_name = %source_name,
            document_id = %document_id,
            deleted_chunks,
            inserted_chunks = chunks.len(),
            "completed knowledge-base ingestion"
        );

        Ok(DocumentIngestSummary {
            source_name,
            document_id,
            chunk_count: chunks.len(),
            deleted_chunks,
        })
    }

    pub async fn ingest_text(
        &self,
        source_name: String,
        text: String,
        tags: Vec<String>,
    ) -> Result<DocumentIngestSummary, IngestError> {
        let settings = self.settings.clone();
        let source_name_clone = source_name.clone();
        let tags_clone = tags.clone();

        let prepared = tokio::task::spawn_blocking(move || {
            prepare_text_document(source_name_clone, tags_clone, text, settings)
        })
        .await
        .context("document parsing worker panicked")
        .map_err(IngestError::from)?
        .map_err(|error| IngestError::validation(error.to_string()))?;

        tracing::info!(
            source_name = %source_name,
            text_chars = prepared.text_chars,
            chunk_count = prepared.chunks.len(),
            "chunked text input"
        );

        let embeddings = self
            .embedder
            .embed_chunks(&source_name, &prepared.chunks)
            .await
            .map_err(IngestError::from)?;

        if embeddings.vectors.len() != prepared.chunks.len() {
            return Err(IngestError::Internal(anyhow::anyhow!(
                "embedding count mismatch for {}",
                source_name
            )));
        }

        let document_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        let chunks = prepared
            .chunks
            .into_iter()
            .enumerate()
            .map(|(chunk_index, text)| ChunkRecord {
                id: format!("{document_id}:{chunk_index}"),
                source_name: source_name.clone(),
                document_id: document_id.clone(),
                chunk_index,
                text,
                tags: tags.clone(),
                created_at: created_at.clone(),
            })
            .collect::<Vec<_>>();

        let deleted_chunks = self
            .chroma
            .replace_source_chunks(&source_name, &chunks, &embeddings.vectors)
            .await
            .map_err(IngestError::from)?;

        tracing::info!(
            source_name = %source_name,
            document_id = %document_id,
            deleted_chunks,
            inserted_chunks = chunks.len(),
            "completed text ingestion"
        );

        Ok(DocumentIngestSummary {
            source_name,
            document_id,
            chunk_count: chunks.len(),
            deleted_chunks,
        })
    }

    async fn prepare_document(
        &self,
        document: UploadedDocument,
    ) -> Result<PreparedDocument, IngestError> {
        if document.bytes.is_empty() {
            return Err(IngestError::validation(format!(
                "{} is empty",
                document.source_name
            )));
        }

        if document.bytes.len() > self.settings.max_upload_bytes {
            return Err(IngestError::validation(format!(
                "{} exceeds the upload limit of {} bytes",
                document.source_name, self.settings.max_upload_bytes
            )));
        }

        tracing::info!(
            source_name = %document.source_name,
            bytes = document.bytes.len(),
            embedding_model = %self.embedder.model(),
            "accepted knowledge-base upload"
        );

        let raw_text = self
            .extract_document_text(&document.source_name, &document.bytes)
            .await?;
        let settings = self.settings.clone();
        let source_name = document.source_name.clone();
        let tags = document.tags.clone();

        tokio::task::spawn_blocking(move || {
            prepare_text_document(source_name, tags, raw_text, settings)
        })
        .await
        .context("document parsing worker panicked")
        .map_err(IngestError::from)?
        .map_err(|error| IngestError::validation(error.to_string()))
    }

    async fn extract_document_text(
        &self,
        source_name: &str,
        bytes: &[u8],
    ) -> Result<String, IngestError> {
        let extension = source_extension(source_name)?;

        match extension.as_str() {
            "txt" => {
                self.extract_text_locally(source_name, bytes, extract_utf8_text)
                    .await
            }
            "md" => self.extract_markdown_text(source_name, bytes).await,
            "pdf" => self.extract_pdf_text(source_name, bytes).await,
            "png" | "jpg" | "jpeg" | "doc" | "docx" => {
                self.extract_text_with_ocr(source_name, bytes).await
            }
            _ => Err(IngestError::validation(unsupported_file_type_message(
                source_name,
            ))),
        }
    }

    async fn extract_pdf_text(
        &self,
        source_name: &str,
        bytes: &[u8],
    ) -> Result<String, IngestError> {
        match self
            .extract_text_locally(source_name, bytes, extract_pdf_text_native)
            .await
        {
            Ok(text) if !normalize_text(&text).is_empty() => Ok(text),
            Ok(_) => self
                .extract_text_with_ocr(source_name, bytes)
                .await
                .or_else(|error| {
                    if self.ocr.is_some() {
                        Err(error)
                    } else {
                        Ok(String::new())
                    }
                }),
            Err(error) => {
                if self.ocr.is_some() {
                    tracing::warn!(
                        source_name = %source_name,
                        error = %error,
                        "native PDF extraction failed, falling back to OCR"
                    );
                    self.extract_text_with_ocr(source_name, bytes).await
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn extract_markdown_text(
        &self,
        source_name: &str,
        bytes: &[u8],
    ) -> Result<String, IngestError> {
        let markdown = self
            .extract_text_locally(source_name, bytes, extract_utf8_text)
            .await?;
        let Some(ocr) = &self.ocr else {
            return Ok(markdown);
        };

        let inline_images = extract_inline_markdown_data_images(&markdown);
        if inline_images.is_empty() {
            return Ok(markdown);
        }

        let mut sections = vec![markdown];

        for (index, image) in inline_images.into_iter().enumerate() {
            match ocr
                .extract_text_from_media(
                    &format!("{source_name}#embedded-image-{}", index + 1),
                    &image.bytes,
                    &image.media_type,
                )
                .await
            {
                Ok(text) if !text.trim().is_empty() => sections.push(format!(
                    "Embedded image {} OCR:\n{}",
                    index + 1,
                    text.trim()
                )),
                Ok(_) => {}
                Err(error) => tracing::warn!(
                    source_name = %source_name,
                    image_index = index + 1,
                    error = ?error,
                    "embedded markdown image OCR failed"
                ),
            }
        }

        Ok(sections.join("\n\n"))
    }

    async fn extract_text_locally<F>(
        &self,
        source_name: &str,
        bytes: &[u8],
        extractor: F,
    ) -> Result<String, IngestError>
    where
        F: FnOnce(&str, &[u8]) -> AnyResult<String> + Send + 'static,
    {
        let source_name = source_name.to_string();
        let bytes = bytes.to_vec();

        tokio::task::spawn_blocking(move || extractor(&source_name, &bytes))
            .await
            .context("document extraction worker panicked")
            .map_err(IngestError::from)?
            .map_err(IngestError::from)
    }

    async fn extract_text_with_ocr(
        &self,
        source_name: &str,
        bytes: &[u8],
    ) -> Result<String, IngestError> {
        let Some(ocr) = &self.ocr else {
            return Err(IngestError::validation(format!(
                "{} requires OCR support. Enable OCR and configure OCR_API_KEY before uploading this file type",
                source_name
            )));
        };

        tracing::info!(
            source_name = %source_name,
            ocr_model = %ocr.model(),
            "extracting upload text with OCR"
        );

        ocr.extract_text(source_name, bytes).await.map_err(|error| {
            IngestError::Internal(error.context(format!("OCR extraction failed for {source_name}")))
        })
    }
}

#[derive(Debug)]
struct PreparedDocument {
    tags: Vec<String>,
    text_chars: usize,
    chunks: Vec<String>,
}

#[derive(Debug)]
struct InlineMarkdownImage {
    media_type: String,
    bytes: Vec<u8>,
}

fn prepare_text_document(
    source_name: String,
    tags: Vec<String>,
    raw_text: String,
    settings: KbIngestionConfig,
) -> AnyResult<PreparedDocument> {
    let normalized_text = normalize_text(&raw_text);
    let text_chars = normalized_text.chars().count();

    if text_chars == 0 {
        bail!("{} does not contain extractable text", source_name);
    }

    let chunks = chunk_text(
        &normalized_text,
        settings.chunk_size,
        settings.chunk_overlap,
    );
    if chunks.is_empty() {
        bail!("{} did not produce any chunks", source_name);
    }

    Ok(PreparedDocument {
        tags,
        text_chars,
        chunks,
    })
}

fn source_extension(source_name: &str) -> Result<String, IngestError> {
    Path::new(source_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .ok_or_else(|| {
            IngestError::validation(format!("{} has no supported file extension", source_name))
        })
}

fn unsupported_file_type_message(source_name: &str) -> String {
    format!(
        "unsupported file type for {}. supported types are {}",
        source_name, SUPPORTED_UPLOAD_TYPES
    )
}

fn extract_utf8_text(_source_name: &str, bytes: &[u8]) -> AnyResult<String> {
    std::str::from_utf8(bytes)
        .context("text documents must be UTF-8 encoded")
        .map(str::to_string)
}

fn extract_pdf_text_native(_source_name: &str, bytes: &[u8]) -> AnyResult<String> {
    pdf_extract::extract_text_from_mem(bytes).context("failed to extract text from PDF")
}

fn extract_inline_markdown_data_images(markdown: &str) -> Vec<InlineMarkdownImage> {
    let mut images = Vec::new();
    let mut remaining = markdown;

    while let Some(start) = remaining.find("data:image/") {
        let candidate = &remaining[start..];
        let end = candidate
            .find(|character: char| {
                character == ')'
                    || character == '"'
                    || character == '\''
                    || character.is_whitespace()
            })
            .unwrap_or(candidate.len());
        let data_uri = &candidate[..end];

        if let Some(image) = decode_data_uri_image(data_uri) {
            images.push(image);
        }

        remaining = &candidate[end..];
    }

    images
}

fn decode_data_uri_image(data_uri: &str) -> Option<InlineMarkdownImage> {
    let (metadata, data) = data_uri.split_once(',')?;
    if !metadata.starts_with("data:image/") || !metadata.contains(";base64") {
        return None;
    }

    let media_type = metadata
        .strip_prefix("data:")?
        .split(';')
        .next()?
        .to_string();
    let bytes = BASE64_STANDARD.decode(data).ok()?;

    Some(InlineMarkdownImage { media_type, bytes })
}

fn normalize_text(text: &str) -> String {
    let normalized_newlines = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut paragraphs = Vec::new();
    let mut current_paragraph = Vec::new();

    for line in normalized_newlines.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            if !current_paragraph.is_empty() {
                paragraphs.push(current_paragraph.join("\n"));
                current_paragraph.clear();
            }
            continue;
        }

        current_paragraph.push(trimmed_line.to_string());
    }

    if !current_paragraph.is_empty() {
        paragraphs.push(current_paragraph.join("\n"));
    }

    paragraphs.join("\n\n").trim().to_string()
}

fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let characters = text.chars().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < characters.len() {
        let end_limit = (start + chunk_size).min(characters.len());
        let end = choose_chunk_end(&characters, start, end_limit, chunk_size);
        let chunk = characters[start..end]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();

        if !chunk.is_empty() {
            chunks.push(chunk);
        }

        if end >= characters.len() {
            break;
        }

        let mut next_start = end.saturating_sub(overlap);
        if next_start <= start {
            next_start = end;
        }

        while next_start < characters.len() && characters[next_start].is_whitespace() {
            next_start += 1;
        }

        start = next_start;
    }

    chunks
}

fn choose_chunk_end(
    characters: &[char],
    start: usize,
    end_limit: usize,
    chunk_size: usize,
) -> usize {
    if end_limit >= characters.len() {
        return characters.len();
    }

    let min_break = (start + (chunk_size / 2)).min(end_limit);

    find_paragraph_break(characters, min_break, end_limit)
        .or_else(|| find_line_break(characters, min_break, end_limit))
        .or_else(|| find_sentence_break(characters, min_break, end_limit))
        .unwrap_or(end_limit)
}

fn find_paragraph_break(characters: &[char], start: usize, end: usize) -> Option<usize> {
    if end <= start + 1 {
        return None;
    }

    for index in (start + 1..end).rev() {
        if characters[index - 1] == '\n' && characters[index] == '\n' {
            return Some(index + 1);
        }
    }

    None
}

fn find_line_break(characters: &[char], start: usize, end: usize) -> Option<usize> {
    for index in (start..end).rev() {
        if characters[index] == '\n' {
            return Some(index + 1);
        }
    }

    None
}

fn find_sentence_break(characters: &[char], start: usize, end: usize) -> Option<usize> {
    const SENTENCE_BREAKS: [char; 8] = ['。', '！', '？', '!', '?', '；', ';', '.'];

    for index in (start..end).rev() {
        if SENTENCE_BREAKS.contains(&characters[index]) {
            return Some(index + 1);
        }
    }

    None
}

fn preview(text: &str, max_chars: usize) -> String {
    let truncated = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decode_data_uri_image, extract_inline_markdown_data_images, unsupported_file_type_message,
    };

    #[test]
    fn extract_inline_markdown_data_images_reads_base64_images() {
        let markdown = "![scan](data:image/png;base64,aGVsbG8=)";
        let images = extract_inline_markdown_data_images(markdown);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].media_type, "image/png");
        assert_eq!(images[0].bytes, b"hello");
    }

    #[test]
    fn decode_data_uri_image_rejects_non_base64_image() {
        assert!(decode_data_uri_image("data:image/png,plain-text").is_none());
    }

    #[test]
    fn unsupported_file_type_message_lists_new_types() {
        let message = unsupported_file_type_message("notes.csv");
        assert!(message.contains(".png"));
        assert!(message.contains(".docx"));
    }
}
