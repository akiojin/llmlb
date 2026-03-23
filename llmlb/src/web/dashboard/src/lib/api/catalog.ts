// Catalog API - HuggingFace model search

import { fetchWithAuth } from './client'

export interface CatalogSearchResult {
  repo_id: string
  description?: string
  downloads?: number
  tags?: string[]
  engine_names: Record<string, string | null>
  supports_download: string[]
}

export interface CatalogSearchResponse {
  models: CatalogSearchResult[]
}

export interface CatalogModelDetail {
  repo_id: string
  description?: string
  downloads?: number
  tags?: string[]
  siblings?: Array<{ rfilename: string }>
  pipeline_tag?: string
  engine_names: Record<string, string | null>
  supports_download: string[]
}

export interface RecommendedEndpoint {
  id: string
  name: string
  endpoint_type: string
  can_download: boolean
  has_model: boolean
}

export const catalogApi = {
  search: (query: string, limit?: number) =>
    fetchWithAuth<CatalogSearchResponse>('/api/catalog/search', {
      params: { q: query, limit: limit ?? 20 },
    }),

  getModel: (repoId: string) =>
    fetchWithAuth<CatalogModelDetail>(`/api/catalog/${repoId}`),

  recommendEndpoints: (repoId: string) =>
    fetchWithAuth<{ endpoints: RecommendedEndpoint[] }>(
      `/api/catalog/recommend-endpoints/${repoId}`
    ),
}
