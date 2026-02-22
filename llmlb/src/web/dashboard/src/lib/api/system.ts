// System API (self-update)

import { fetchWithAuth } from './client'

export type UpdatePayloadState =
  | { payload: 'not_ready' }
  | { payload: 'downloading'; started_at: string }
  | { payload: 'ready'; kind: unknown }
  | { payload: 'error'; message: string }

export type UpdateState =
  | { state: 'up_to_date'; checked_at?: string | null }
  | {
      state: 'available'
      current: string
      latest: string
      release_url: string
      portable_asset_url?: string | null
      installer_asset_url?: string | null
      payload: UpdatePayloadState
      checked_at: string
    }
  | { state: 'draining'; latest: string; in_flight: number; requested_at: string }
  | { state: 'applying'; latest: string; method: string }
  | {
      state: 'failed'
      latest?: string | null
      release_url?: string | null
      message: string
      failed_at: string
    }

export interface SystemInfo {
  version: string
  pid: number
  in_flight: number
  update: UpdateState
}

export interface ApplyUpdateResponse {
  queued: boolean
  mode: 'normal'
}

export interface ForceApplyUpdateResponse {
  queued: false
  mode: 'force'
  dropped_in_flight: number
}

export const systemApi = {
  getSystem: () => fetchWithAuth<SystemInfo>('/api/system'),
  checkUpdate: () =>
    fetchWithAuth<{ update: UpdateState }>('/api/system/update/check', {
      method: 'POST',
      body: JSON.stringify({}),
    }),
  applyUpdate: () =>
    fetchWithAuth<ApplyUpdateResponse>('/api/system/update/apply', {
      method: 'POST',
      body: JSON.stringify({}),
    }),
  applyForceUpdate: () =>
    fetchWithAuth<ForceApplyUpdateResponse>('/api/system/update/apply/force', {
      method: 'POST',
      body: JSON.stringify({}),
    }),
}
