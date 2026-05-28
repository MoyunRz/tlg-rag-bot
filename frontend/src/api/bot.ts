import type { AxiosProgressEvent } from 'axios'

import client from './client'

export interface ChunkItem {
  id: string
  source_name: string
  text: string
  tags: string[]
}

export interface ChunkListResponse {
  total: number
  page: number
  page_size: number
  items: ChunkItem[]
}

export interface UploadFileResult {
  source_name: string
  chunk_count: number
  error?: string
}

export interface UploadResponse {
  status: string
  message: string
  collection: string
  documents_processed: number
  chunks_created: number
  files: UploadFileResult[]
}

export interface RetrievedChunk {
  id: string
  source_name: string
  document_id: string
  chunk_index: number
  text: string
  tags: string[]
  score: number
  distance: number
}

export interface RagTimings {
  embed: number
  retrieve: number
  generate?: number
  total: number
}

export interface RagCitation {
  index: number
  source_name: string
  document_id: string
  chunk_index: number
  excerpt: string
  tags: string[]
  score: number
}

export interface DebugResponse {
  question: string
  faq_hit: boolean
  retrieved_chunks: string[]
  final_answer: string
  retrieved_chunk_items?: RetrievedChunk[]
  provider?: string
  model?: string
  top_k?: number
  retrieval_count?: number
  answer_format?: string
  citations?: RagCitation[]
  score_threshold?: number
  timings_ms?: RagTimings
}

export const fetchChunks = async (params?: { page?: number; page_size?: number; source?: string }) => {
  const { data } = await client.get<ChunkListResponse>('/kb/chunks', { params })
  return data
}

export interface DeleteChunksRequest {
  ids?: string[]
  source?: string
}

export const deleteChunks = async (request: DeleteChunksRequest) => {
  const { data } = await client.delete<{ deleted: number }>('/kb/chunks', { data: request })
  return data
}

export const fetchSources = async () => {
  const { data } = await client.get<string[]>('/kb/sources')
  return data
}

export interface TextUploadRequest {
  source_name: string
  text: string
  tags?: string[]
}

export const uploadText = async (request: TextUploadRequest) => {
  const { data } = await client.post<UploadResponse>('/kb/text', request)
  return data
}

export const uploadKnowledgeBase = async (
  formData: FormData,
  onUploadProgress?: (event: AxiosProgressEvent) => void,
) => {
  const { data } = await client.post<UploadResponse>('/kb/upload', formData, {
    onUploadProgress,
  })
  return data
}

export const runDebugQuery = async (question: string) => {
  const { data } = await client.post<DebugResponse>('/debug/query', { question })
  return data
}
