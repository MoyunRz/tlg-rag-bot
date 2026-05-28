use std::{panic::AssertUnwindSafe, sync::Arc, time::Instant};

use anyhow::{anyhow, bail, Context, Result};
use model2vec_rs::model::StaticModel;

use crate::models::kb::EmbeddingBatch;

#[derive(Debug, Clone)]
pub struct EmbeddingService {
    model_name: String,
    chunk_batch_size: usize,
    dimension: usize,
    model: Arc<StaticModel>,
}

impl EmbeddingService {
    pub fn try_new(model_name: String, chunk_batch_size: usize) -> Result<Self> {
        let started_at = Instant::now();
        let model_name = model_name.trim().to_string();

        let model = StaticModel::from_pretrained(&model_name, None, None, None)
            .with_context(|| format!("failed to load model2vec model from {model_name}"))?;
        let dimension = model.encode_single("dimension probe").len();

        if dimension == 0 {
            bail!("model2vec model {model_name} returned empty embeddings");
        }

        tracing::info!(
            model = %model_name,
            dimension,
            elapsed_ms = started_at.elapsed().as_millis() as u64,
            "loaded model2vec embedding model"
        );

        Ok(Self {
            model_name,
            chunk_batch_size,
            dimension,
            model: Arc::new(model),
        })
    }

    pub fn model(&self) -> &str {
        &self.model_name
    }

    pub async fn embed_chunks(
        &self,
        source_name: &str,
        chunks: &[String],
    ) -> Result<EmbeddingBatch> {
        if chunks.len() <= self.chunk_batch_size {
            return self.embed_texts(source_name, "chunk", chunks).await;
        }

        let started_at = Instant::now();
        let mut vectors = Vec::with_capacity(chunks.len());

        tracing::info!(
            label = %source_name,
            input_kind = "chunk",
            input_count = chunks.len(),
            batch_size = self.chunk_batch_size,
            batch_count = chunks.len().div_ceil(self.chunk_batch_size),
            model = %self.model_name,
            dimension = self.dimension,
            "starting chunk embedding batches"
        );

        for (batch_index, chunk_batch) in chunks.chunks(self.chunk_batch_size).enumerate() {
            let batch_started_at = Instant::now();
            let batch_start = batch_index * self.chunk_batch_size;
            let batch_end = batch_start + chunk_batch.len();

            tracing::info!(
                label = %source_name,
                input_kind = "chunk",
                input_count = chunks.len(),
                batch_index,
                batch_start,
                batch_end,
                batch_size = chunk_batch.len(),
                model = %self.model_name,
                "starting chunk embedding batch"
            );

            let batch_embeddings = self.embed_texts(source_name, "chunk", chunk_batch).await?;
            vectors.extend(batch_embeddings.vectors);

            tracing::info!(
                label = %source_name,
                input_kind = "chunk",
                input_count = chunks.len(),
                batch_index,
                batch_start,
                batch_end,
                batch_size = chunk_batch.len(),
                elapsed_ms = batch_started_at.elapsed().as_millis() as u64,
                completed_vectors = vectors.len(),
                model = %self.model_name,
                "finished chunk embedding batch"
            );
        }

        let batch = validate_embedding_batch(source_name, chunks.len(), self.dimension, vectors)?;

        tracing::info!(
            label = %source_name,
            input_kind = "chunk",
            input_count = chunks.len(),
            batch_size = self.chunk_batch_size,
            dimension = batch.dimension,
            elapsed_ms = started_at.elapsed().as_millis() as u64,
            model = %self.model_name,
            "finished chunk embedding batches"
        );

        Ok(batch)
    }

    pub async fn embed_query(&self, question: &str) -> Result<Vec<f32>> {
        let inputs = vec![question.to_string()];
        let embeddings = self.embed_texts("query", "query", &inputs).await?;

        embeddings
            .vectors
            .into_iter()
            .next()
            .context("missing query embedding vector")
    }

    async fn embed_texts(
        &self,
        label: &str,
        input_kind: &str,
        inputs: &[String],
    ) -> Result<EmbeddingBatch> {
        if inputs.is_empty() {
            return Ok(EmbeddingBatch {
                vectors: Vec::new(),
                dimension: self.dimension,
            });
        }

        let started_at = Instant::now();
        let label_for_log = label.to_string();
        let input_kind_for_log = input_kind.to_string();
        let model_name = self.model_name.clone();
        let input_batch = inputs.to_vec();
        let input_count = input_batch.len();
        let expected_dimension = self.dimension;
        let encode_batch_size = input_count.max(1);
        let model = Arc::clone(&self.model);

        tracing::info!(
            label = %label,
            input_kind = %input_kind,
            input_count,
            model = %self.model_name,
            "starting embedding generation"
        );

        let embeddings = tokio::task::spawn_blocking(move || -> Result<EmbeddingBatch> {
            let worker_started_at = Instant::now();
            let vectors = std::panic::catch_unwind(AssertUnwindSafe(|| {
                model.encode_with_args(&input_batch, Some(512), encode_batch_size)
            }))
            .map_err(|_| anyhow!("model2vec encoding panicked"))?;
            let batch =
                validate_embedding_batch(&label_for_log, input_count, expected_dimension, vectors)?;

            tracing::info!(
                label = %label_for_log,
                input_kind = %input_kind_for_log,
                input_count,
                model = %model_name,
                dimension = batch.dimension,
                elapsed_ms = worker_started_at.elapsed().as_millis() as u64,
                "embedding generation finished"
            );

            Ok(batch)
        })
        .await
        .context("embedding worker panicked")??;

        tracing::info!(
            label = %label,
            input_kind = %input_kind,
            input_count = inputs.len(),
            model = %self.model_name,
            dimension = embeddings.dimension,
            elapsed_ms = started_at.elapsed().as_millis() as u64,
            "generated embeddings"
        );

        Ok(embeddings)
    }
}

fn validate_embedding_batch(
    label: &str,
    input_count: usize,
    expected_dimension: usize,
    vectors: Vec<Vec<f32>>,
) -> Result<EmbeddingBatch> {
    if vectors.len() != input_count {
        bail!(
            "embedding count mismatch for {}: expected {}, got {}",
            label,
            input_count,
            vectors.len()
        );
    }

    if expected_dimension == 0 {
        bail!("embedding dimension must be greater than 0 for {}", label);
    }

    if let Some((index, actual_dimension)) =
        vectors.iter().enumerate().find_map(|(index, vector)| {
            (vector.len() != expected_dimension).then_some((index, vector.len()))
        })
    {
        bail!(
            "embedding dimension mismatch for {} at index {}: expected {}, got {}",
            label,
            index,
            expected_dimension,
            actual_dimension
        );
    }

    Ok(EmbeddingBatch {
        vectors,
        dimension: expected_dimension,
    })
}

#[cfg(test)]
mod tests {
    use super::validate_embedding_batch;

    #[test]
    fn validate_embedding_batch_rejects_count_mismatch() {
        let error = validate_embedding_batch("test", 2, 3, vec![vec![1.0, 2.0, 3.0]])
            .expect_err("should reject vector count mismatch");
        assert!(error.to_string().contains("embedding count mismatch"));
    }

    #[test]
    fn validate_embedding_batch_rejects_dimension_mismatch() {
        let error =
            validate_embedding_batch("test", 2, 3, vec![vec![1.0, 2.0, 3.0], vec![1.0, 2.0]])
                .expect_err("should reject vector dimension mismatch");
        assert!(error.to_string().contains("embedding dimension mismatch"));
    }

    #[test]
    fn validate_embedding_batch_accepts_matching_vectors() {
        let batch =
            validate_embedding_batch("test", 2, 3, vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]])
                .expect("should accept matching vectors");
        assert_eq!(batch.dimension, 3);
        assert_eq!(batch.vectors.len(), 2);
    }
}
