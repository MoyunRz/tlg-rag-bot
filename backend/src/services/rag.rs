use std::{sync::Arc, time::Instant};

use anyhow::{ensure, Result};

use crate::{
    config::RagConfigInternal,
    models::kb::{RagAnswer, RagCitation, RagTimings, RetrievedChunk},
    services::{chroma::ChromaService, embed::EmbeddingService, llm::LlmService},
};

const ANSWER_FORMAT_MARKDOWN: &str = "markdown";
const LOW_CONTEXT_FALLBACK: &str =
    "当前知识库里没有足够相关的信息，暂时无法给出确定答复。\n\n建议联系人工客服进一步确认。";

#[derive(Debug, Clone)]
pub struct RagService {
    config: RagConfigInternal,
    chroma: Arc<ChromaService>,
    embedder: Arc<EmbeddingService>,
    llm: Arc<LlmService>,
}

impl RagService {
    pub fn new(
        config: RagConfigInternal,
        chroma: Arc<ChromaService>,
        embedder: Arc<EmbeddingService>,
        llm: Arc<LlmService>,
    ) -> Self {
        Self {
            config,
            chroma,
            embedder,
            llm,
        }
    }

    pub async fn answer_question(&self, question: &str) -> Result<RagAnswer> {
        let question = question.trim();
        ensure!(!question.is_empty(), "question cannot be empty");

        let total_started = Instant::now();

        let embed_started = Instant::now();
        let query_embedding = self.embedder.embed_query(question).await?;
        let embed_ms = elapsed_ms(embed_started);

        let retrieve_started = Instant::now();
        let retrieved = self
            .chroma
            .query_chunks(&query_embedding, self.config.top_k)
            .await?;
        let filtered = self.filter_chunks(retrieved);
        let retrieve_ms = elapsed_ms(retrieve_started);

        tracing::info!(
            question = %preview(question, 80),
            top_k = self.config.top_k,
            retrieved_count = filtered.len(),
            score_threshold = ?self.config.score_threshold,
            "retrieved chunks for RAG query"
        );

        if filtered.is_empty() {
            return Ok(self.low_context_answer(
                question,
                embed_ms,
                retrieve_ms,
                elapsed_ms(total_started),
            ));
        }

        let context = build_context(&filtered, self.config.max_context_chars);
        let generate_started = Instant::now();
        let final_answer = self
            .llm
            .chat(&self.config.bot_prompt.system, &self.build_user_prompt(question, &context))
            .await?;
        let generate_ms = elapsed_ms(generate_started);
        let total_ms = elapsed_ms(total_started);

        tracing::info!(
            question = %preview(question, 80),
            provider = %self.llm.provider(),
            model = %self.llm.model(),
            retrieved_count = filtered.len(),
            context_chars = context.chars().count(),
            elapsed_ms = total_ms,
            "generated RAG answer"
        );

        Ok(RagAnswer {
            question: question.to_string(),
            faq_hit: false,
            retrieved_chunks: filtered.iter().map(|chunk| chunk.text.clone()).collect(),
            retrieved_chunk_items: filtered.clone(),
            final_answer,
            provider: self.llm.provider().to_string(),
            model: self.llm.model().to_string(),
            top_k: self.config.top_k,
            retrieval_count: filtered.len(),
            answer_format: Some(ANSWER_FORMAT_MARKDOWN.to_string()),
            citations: Some(build_citations(&filtered)),
            score_threshold: self.config.score_threshold,
            timings_ms: Some(RagTimings {
                embed: embed_ms,
                retrieve: retrieve_ms,
                generate: Some(generate_ms),
                total: total_ms,
            }),
        })
    }

    fn filter_chunks(&self, chunks: Vec<RetrievedChunk>) -> Vec<RetrievedChunk> {
        chunks
            .into_iter()
            .filter(|chunk| {
                self.config
                    .score_threshold
                    .map(|threshold| chunk.score >= threshold)
                    .unwrap_or(true)
            })
            .collect()
    }

    fn low_context_answer(
        &self,
        question: &str,
        embed_ms: u64,
        retrieve_ms: u64,
        total_ms: u64,
    ) -> RagAnswer {
        RagAnswer {
            question: question.to_string(),
            faq_hit: false,
            retrieved_chunks: Vec::new(),
            retrieved_chunk_items: Vec::new(),
            final_answer: LOW_CONTEXT_FALLBACK.to_string(),
            provider: self.llm.provider().to_string(),
            model: self.llm.model().to_string(),
            top_k: self.config.top_k,
            retrieval_count: 0,
            answer_format: Some(ANSWER_FORMAT_MARKDOWN.to_string()),
            citations: None,
            score_threshold: self.config.score_threshold,
            timings_ms: Some(RagTimings {
                embed: embed_ms,
                retrieve: retrieve_ms,
                generate: None,
                total: total_ms,
            }),
        }
    }
}

fn build_context(chunks: &[RetrievedChunk], max_context_chars: usize) -> String {
    let mut context = String::new();

    for (index, chunk) in chunks.iter().enumerate() {
        let section = format!(
            "[{}] 来源: {}\n内容: {}\n",
            index + 1,
            chunk.source_name,
            chunk.text
        );

        if !context.is_empty()
            && context.chars().count() + section.chars().count() > max_context_chars
        {
            break;
        }

        context.push_str(&section);
        context.push('\n');
    }

    context.trim().to_string()
}

impl RagService {
    fn build_user_prompt(&self, question: &str, context: &str) -> String {
        self.config.bot_prompt.user_template
            .replace("{question}", question)
            .replace("{context}", context)
    }
}

fn build_citations(chunks: &[RetrievedChunk]) -> Vec<RagCitation> {
    chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| RagCitation {
            index: index + 1,
            source_name: chunk.source_name.clone(),
            document_id: chunk.document_id.clone(),
            chunk_index: chunk.chunk_index,
            excerpt: preview(&chunk.text, 220),
            tags: chunk.tags.clone(),
            score: chunk.score,
        })
        .collect()
}

fn elapsed_ms(started_at: Instant) -> u64 {
    started_at.elapsed().as_millis() as u64
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
    use super::{build_citations, build_context};
    use crate::models::kb::RetrievedChunk;

    fn sample_chunk(index: usize, text: &str) -> RetrievedChunk {
        RetrievedChunk {
            id: format!("chunk-{index}"),
            source_name: format!("source-{index}.md"),
            document_id: format!("doc-{index}"),
            chunk_index: index,
            text: text.to_string(),
            tags: vec!["faq".to_string()],
            score: 0.9 - index as f32 * 0.1,
            distance: 0.1 + index as f32 * 0.1,
        }
    }

    #[test]
    fn build_context_numbers_sources_in_order() {
        let chunks = vec![sample_chunk(0, "第一段"), sample_chunk(1, "第二段")];

        let context = build_context(&chunks, 10_000);

        assert!(context.contains("[1] 来源: source-0.md"));
        assert!(context.contains("[2] 来源: source-1.md"));
    }

    #[test]
    fn build_citations_preserves_chunk_order_and_excerpt() {
        let long_text = "A".repeat(240);
        let chunks = vec![sample_chunk(0, "第一段"), sample_chunk(1, &long_text)];

        let citations = build_citations(&chunks);

        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].index, 1);
        assert_eq!(citations[0].source_name, "source-0.md");
        assert_eq!(citations[1].index, 2);
        assert_eq!(citations[1].document_id, "doc-1");
        assert!(citations[1].excerpt.ends_with("..."));
    }
}
