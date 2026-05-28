use std::{collections::HashMap, sync::Mutex, time::Instant};

use anyhow::{anyhow, Context, Result};
use reqwest::Response;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::kb::{ChunkItem, ChunkRecord, RetrievedChunk};

const DEFAULT_TENANT: &str = "default_tenant";
const DEFAULT_DATABASE: &str = "default_database";
const DEFAULT_LIST_LIMIT: usize = 10_000;

#[derive(Debug)]
pub struct ChromaService {
    client: reqwest::Client,
    base_url: String,
    collection_name: String,
    collection_id: Mutex<Option<String>>,
}

impl ChromaService {
    pub fn new(client: reqwest::Client, base_url: String, collection_name: String) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            collection_name,
            collection_id: Mutex::new(None),
        }
    }

    pub fn collection_name(&self) -> &str {
        &self.collection_name
    }

    pub async fn replace_source_chunks(
        &self,
        source_name: &str,
        chunks: &[ChunkRecord],
        embeddings: &[Vec<f32>],
    ) -> Result<usize> {
        let started_at = Instant::now();
        let collection_id = self.ensure_collection_id().await?;
        let deleted_chunks = self
            .delete_source_chunks(&collection_id, source_name)
            .await?;

        let payload = json!({
            "ids": chunks.iter().map(|chunk| chunk.id.as_str()).collect::<Vec<_>>(),
            "documents": chunks.iter().map(|chunk| chunk.text.as_str()).collect::<Vec<_>>(),
            "embeddings": embeddings,
            "metadatas": chunks
                .iter()
                .map(|chunk| {
                    json!({
                        "source_name": chunk.source_name,
                        "document_id": chunk.document_id,
                        "chunk_index": chunk.chunk_index,
                        "tags": chunk.tags.join(","),
                        "created_at": chunk.created_at,
                    })
                })
                .collect::<Vec<_>>(),
        });

        let response = self
            .client
            .post(self.collection_action_url(&collection_id, "upsert"))
            .json(&payload)
            .send()
            .await
            .context("failed to call Chroma upsert endpoint")?;

        parse_json_response::<Value>(response, "upsert collection chunks").await?;

        tracing::info!(
            collection = %self.collection_name,
            source_name = %source_name,
            deleted_chunks,
            inserted_chunks = chunks.len(),
            elapsed_ms = started_at.elapsed().as_millis() as u64,
            "upserted chunks into Chroma"
        );

        Ok(deleted_chunks)
    }

    pub async fn count_chunks(&self, source_filter: Option<&str>) -> Result<usize> {
        let collection_id = self.ensure_collection_id().await?;
        let mut body = json!({ "limit": DEFAULT_LIST_LIMIT });
        if let Some(source) = source_filter {
            body["where"] = json!({ "source_name": source });
        }
        let response = self
            .client
            .post(self.collection_action_url(&collection_id, "get"))
            .json(&body)
            .send()
            .await
            .context("failed to call Chroma get endpoint for count")?;

        let payload =
            parse_json_response::<GetResponse>(response, "count collection chunks").await?;
        Ok(payload.ids.len())
    }

    pub async fn list_sources(&self) -> Result<Vec<String>> {
        let collection_id = self.ensure_collection_id().await?;
        let response = self
            .client
            .post(self.collection_action_url(&collection_id, "get"))
            .json(&json!({
                "limit": DEFAULT_LIST_LIMIT,
                "include": ["metadatas"],
            }))
            .send()
            .await
            .context("failed to call Chroma get endpoint for sources")?;

        let payload =
            parse_json_response::<GetResponse>(response, "list collection sources").await?;
        let metadatas = payload.metadatas.unwrap_or_default();
        let mut sources: Vec<String> = metadatas
            .into_iter()
            .filter_map(|m| m.and_then(|m| metadata_string(&m, "source_name")))
            .collect();
        sources.sort();
        sources.dedup();
        Ok(sources)
    }

    pub async fn list_chunks_paginated(
        &self,
        offset: usize,
        limit: usize,
        source_filter: Option<&str>,
    ) -> Result<Vec<ChunkItem>> {
        let collection_id = self.ensure_collection_id().await?;
        let mut body = json!({
            "limit": DEFAULT_LIST_LIMIT,
            "include": ["documents", "metadatas"],
        });
        if let Some(source) = source_filter {
            body["where"] = json!({ "source_name": source });
        }
        let response = self
            .client
            .post(self.collection_action_url(&collection_id, "get"))
            .json(&body)
            .send()
            .await
            .context("failed to call Chroma get endpoint")?;

        let payload =
            parse_json_response::<GetResponse>(response, "list collection chunks").await?;
        let documents = payload.documents.unwrap_or_default();
        let metadatas = payload.metadatas.unwrap_or_default();
        let mut items = Vec::with_capacity(payload.ids.len());

        for (index, id) in payload.ids.into_iter().enumerate() {
            let text = documents
                .get(index)
                .and_then(|value| value.clone())
                .unwrap_or_default();
            let metadata = metadatas
                .get(index)
                .and_then(|value| value.clone())
                .unwrap_or_default();
            let source_name =
                metadata_string(&metadata, "source_name").unwrap_or_else(|| "unknown".to_string());
            let chunk_index = metadata_usize(&metadata, "chunk_index").unwrap_or(usize::MAX);
            let tags = metadata_tags(&metadata);

            items.push((
                source_name.clone(),
                chunk_index,
                ChunkItem {
                    id,
                    source_name,
                    text,
                    tags,
                },
            ));
        }

        items.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

        Ok(items
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|(_, _, item)| item)
            .collect())
    }

    pub async fn delete_chunks_by_ids(&self, ids: &[String]) -> Result<usize> {
        let collection_id = self.ensure_collection_id().await?;
        let response = self
            .client
            .post(self.collection_action_url(&collection_id, "delete"))
            .json(&json!({ "ids": ids }))
            .send()
            .await
            .context("failed to call Chroma delete endpoint")?;

        let deleted =
            parse_json_response::<DeleteResponse>(response, "delete chunks by ids").await?;
        Ok(deleted.deleted)
    }

    pub async fn delete_chunks_by_source(&self, source_name: &str) -> Result<usize> {
        let collection_id = self.ensure_collection_id().await?;
        let response = self
            .client
            .post(self.collection_action_url(&collection_id, "delete"))
            .json(&json!({
                "where": { "source_name": source_name }
            }))
            .send()
            .await
            .context("failed to call Chroma delete endpoint")?;

        let deleted =
            parse_json_response::<DeleteResponse>(response, "delete chunks by source").await?;
        Ok(deleted.deleted)
    }

    pub async fn query_chunks(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let started_at = Instant::now();
        let collection_id = self.ensure_collection_id().await?;
        let response = self
            .client
            .post(self.collection_action_url(&collection_id, "query"))
            .json(&json!({
                "query_embeddings": [query_embedding],
                "n_results": top_k,
                "include": ["documents", "metadatas", "distances"],
            }))
            .send()
            .await
            .context("failed to call Chroma query endpoint")?;

        let payload =
            parse_json_response::<QueryResponse>(response, "query collection chunks").await?;
        let ids = payload.ids.into_iter().next().unwrap_or_default();
        let documents = payload
            .documents
            .unwrap_or_default()
            .into_iter()
            .next()
            .unwrap_or_default();
        let metadatas = payload
            .metadatas
            .unwrap_or_default()
            .into_iter()
            .next()
            .unwrap_or_default();
        let distances = payload
            .distances
            .unwrap_or_default()
            .into_iter()
            .next()
            .unwrap_or_default();
        let mut items = Vec::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            let metadata = metadatas.get(index).cloned().unwrap_or_default();
            let distance = distances.get(index).copied().unwrap_or(f32::INFINITY);
            let score = similarity_score(distance);

            items.push(RetrievedChunk {
                id,
                source_name: metadata_string(&metadata, "source_name")
                    .unwrap_or_else(|| "unknown".to_string()),
                document_id: metadata_string(&metadata, "document_id").unwrap_or_default(),
                chunk_index: metadata_usize(&metadata, "chunk_index").unwrap_or(usize::MAX),
                text: documents.get(index).cloned().unwrap_or_default(),
                tags: metadata_tags(&metadata),
                score,
                distance,
            });
        }

        tracing::info!(
            collection = %self.collection_name,
            requested_top_k = top_k,
            returned_chunks = items.len(),
            elapsed_ms = started_at.elapsed().as_millis() as u64,
            "queried Chroma collection"
        );

        Ok(items)
    }

    async fn ensure_collection_id(&self) -> Result<String> {
        if let Some(cached) = self.collection_id.lock().unwrap().clone() {
            return Ok(cached);
        }

        let response = self
            .client
            .get(self.collections_url())
            .send()
            .await
            .context("failed to list Chroma collections")?;
        let collections =
            parse_json_response::<Vec<CollectionRecord>>(response, "list collections").await?;

        if let Some(collection) = collections
            .into_iter()
            .find(|collection| collection.name == self.collection_name)
        {
            *self.collection_id.lock().unwrap() = Some(collection.id.clone());
            return Ok(collection.id);
        }

        let response = self
            .client
            .post(self.collections_url())
            .json(&json!({ "name": self.collection_name }))
            .send()
            .await
            .context("failed to create Chroma collection")?;
        let created =
            parse_json_response::<CollectionRecord>(response, "create collection").await?;
        *self.collection_id.lock().unwrap() = Some(created.id.clone());

        Ok(created.id)
    }

    async fn delete_source_chunks(&self, collection_id: &str, source_name: &str) -> Result<usize> {
        let response = self
            .client
            .post(self.collection_action_url(collection_id, "delete"))
            .json(&json!({
                "where": {
                    "source_name": source_name,
                }
            }))
            .send()
            .await
            .context("failed to call Chroma delete endpoint")?;

        let deleted =
            parse_json_response::<DeleteResponse>(response, "delete source chunks").await?;
        Ok(deleted.deleted)
    }

    fn collections_url(&self) -> String {
        format!(
            "{}/api/v2/tenants/{DEFAULT_TENANT}/databases/{DEFAULT_DATABASE}/collections",
            self.base_url
        )
    }

    fn collection_action_url(&self, collection_id: &str, action: &str) -> String {
        format!("{}/{collection_id}/{action}", self.collections_url())
    }
}

#[derive(Debug, Deserialize)]
struct CollectionRecord {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct DeleteResponse {
    deleted: usize,
}

#[derive(Debug, Deserialize)]
struct GetResponse {
    ids: Vec<String>,
    documents: Option<Vec<Option<String>>>,
    metadatas: Option<Vec<Option<HashMap<String, Value>>>>,
}

#[derive(Debug, Deserialize)]
struct QueryResponse {
    ids: Vec<Vec<String>>,
    documents: Option<Vec<Vec<String>>>,
    metadatas: Option<Vec<Vec<HashMap<String, Value>>>>,
    distances: Option<Vec<Vec<f32>>>,
}

async fn parse_json_response<T>(response: Response, action: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("failed to read Chroma response body for {action}"))?;

    if !status.is_success() {
        return Err(anyhow!(
            "Chroma {action} failed with status {}: {}",
            status,
            body
        ));
    }

    serde_json::from_str(&body)
        .with_context(|| format!("failed to decode Chroma response for {action}: {body}"))
}

fn metadata_string(metadata: &HashMap<String, Value>, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn metadata_usize(metadata: &HashMap<String, Value>, key: &str) -> Option<usize> {
    metadata
        .get(key)
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
}

fn metadata_tags(metadata: &HashMap<String, Value>) -> Vec<String> {
    match metadata.get("tags") {
        Some(Value::String(tags)) => tags
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_string)
            .collect(),
        Some(Value::Array(tags)) => tags
            .iter()
            .filter_map(|tag| tag.as_str())
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn similarity_score(distance: f32) -> f32 {
    if !distance.is_finite() {
        return 0.0;
    }

    1.0 / (1.0 + distance.max(0.0))
}
