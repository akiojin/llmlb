# Phase 1 データモデル: 包括的E2Eテストスイート強化

**機能ID**: `SPEC-62241000` | **日付**: 2026-02-13

## データモデル概要

本機能はE2Eテストコードの追加であり、永続化データモデルの新規追加はない。
以下はテストで操作する既存データエンティティと、モックサーバーのインターフェース定義。

## テスト対象エンティティ

### Endpoint

```typescript
interface Endpoint {
  id: string
  name: string
  base_url: string
  status: 'pending' | 'online' | 'offline' | 'error'
  endpoint_type: 'xllm' | 'ollama' | 'vllm' | 'openai' | 'unknown'
  health_check_interval: number  // 秒 (デフォルト: 60)
  inference_timeout: number      // 秒 (デフォルト: 120, 範囲: 10-600)
  notes: string | null
}
```

### User

```typescript
interface User {
  id: string
  username: string
  role: 'admin' | 'user'
}
```

### ApiKey

```typescript
interface ApiKey {
  id: string
  name: string
  key_prefix: string
  permissions: ApiKeyPermission[]
  expires_at: string | null
  created_at: string
}

type ApiKeyPermission =
  | 'openai.inference'
  | 'openai.models.read'
  | 'endpoints.read'
  | 'endpoints.manage'
  | 'api_keys.manage'
  | 'users.manage'
  | 'invitations.manage'
  | 'models.manage'
  | 'registry.read'
  | 'logs.read'
  | 'metrics.read'
```

## モックサーバーインターフェース拡張

### MockOpenAIEndpointServer 拡張

```typescript
interface MockOpenAIEndpointServerOptions {
  models?: string[]
  responseDelayMs?: number
  // 新規追加
  endpointType?: 'xllm' | 'ollama' | 'vllm' | 'openai'
  supportAudio?: boolean
  supportImages?: boolean
  supportResponses?: boolean
}
```

### Audio API モックレスポンス

```typescript
// POST /v1/audio/transcriptions
interface TranscriptionResponse {
  text: string
}

// POST /v1/audio/speech
// → バイナリレスポンス (audio/mpeg)
```

### Image API モックレスポンス

```typescript
// POST /v1/images/generations
interface ImageGenerationResponse {
  created: number
  data: Array<{ url: string } | { b64_json: string }>
}
```

### Responses API モックレスポンス

```typescript
// POST /v1/responses
interface ResponsesAPIResponse {
  id: string
  object: 'response'
  created_at: number
  output: Array<{
    type: 'message'
    role: 'assistant'
    content: Array<{ type: 'output_text'; text: string }>
  }>
}
```

## 権限マトリクスデータ構造

```typescript
interface PermissionTestCase {
  permission: ApiKeyPermission
  endpoint: string
  method: 'GET' | 'POST' | 'PUT' | 'DELETE'
  expectedStatus: 200 | 201 | 403
  description: string
}
```

## テストヘルパー拡張

### api-helpers.ts 追加関数

```typescript
// ユーザー管理
function createUser(request, username, password, role): Promise<User>
function updateUserRole(request, userId, role): Promise<void>
function deleteUser(request, userId): Promise<void>
function listUsers(request): Promise<User[]>

// APIキー管理
function createApiKeyWithPermissions(request, name, permissions, expiresAt?): Promise<{id, key}>
function deleteApiKey(request, keyId): Promise<void>

// ログ
function getLbLogs(request): Promise<LogEntry[]>
function getEndpointLogs(request, endpointId): Promise<LogEntry[]>

// メトリクス
function getPrometheusMetrics(request): Promise<string>

// システム
function getSystemInfo(request): Promise<SystemInfo>
```
