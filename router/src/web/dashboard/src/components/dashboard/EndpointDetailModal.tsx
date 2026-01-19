import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { type DashboardEndpoint, endpointsApi } from '@/lib/api'
import { formatRelativeTime } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Server, Clock, AlertCircle, Save, Play, RefreshCw } from 'lucide-react'

/**
 * SPEC-66555000: ルーター主導エンドポイント登録システム
 * エンドポイント詳細モーダル
 */

interface EndpointDetailModalProps {
  endpoint: DashboardEndpoint | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

function getStatusBadgeVariant(
  status: DashboardEndpoint['status']
): 'default' | 'destructive' | 'outline' | 'secondary' {
  switch (status) {
    case 'online':
      return 'default'
    case 'pending':
      return 'secondary'
    case 'offline':
      return 'outline'
    case 'error':
      return 'destructive'
    default:
      return 'outline'
  }
}

function getStatusLabel(status: DashboardEndpoint['status']): string {
  switch (status) {
    case 'online':
      return 'オンライン'
    case 'pending':
      return '確認中'
    case 'offline':
      return 'オフライン'
    case 'error':
      return 'エラー'
    default:
      return status
  }
}

export function EndpointDetailModal({ endpoint, open, onOpenChange }: EndpointDetailModalProps) {
  const queryClient = useQueryClient()
  const [name, setName] = useState(endpoint?.name || '')
  const [notes, setNotes] = useState(endpoint?.notes || '')
  const [healthCheckInterval, setHealthCheckInterval] = useState(
    endpoint?.health_check_interval_secs?.toString() || '30'
  )
  const [inferenceTimeout, setInferenceTimeout] = useState(
    endpoint?.inference_timeout_secs?.toString() || '120'
  )

  // Update mutation
  const updateMutation = useMutation({
    mutationFn: (data: Parameters<typeof endpointsApi.update>[1]) =>
      endpointsApi.update(endpoint!.id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dashboard-endpoints'] })
      toast({
        title: '更新完了',
        description: 'エンドポイント設定を更新しました',
      })
    },
    onError: (error) => {
      toast({
        title: '更新失敗',
        description: String(error),
        variant: 'destructive',
      })
    },
  })

  // Test connection mutation
  const testMutation = useMutation({
    mutationFn: () => endpointsApi.test(endpoint!.id),
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: ['dashboard-endpoints'] })
      toast({
        title: result.success ? '接続成功' : '接続失敗',
        description: result.message || (result.latency_ms ? `レイテンシ: ${result.latency_ms}ms` : ''),
        variant: result.success ? 'default' : 'destructive',
      })
    },
    onError: (error) => {
      toast({
        title: '接続テスト失敗',
        description: String(error),
        variant: 'destructive',
      })
    },
  })

  // Sync models mutation
  const syncMutation = useMutation({
    mutationFn: () => endpointsApi.sync(endpoint!.id),
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: ['dashboard-endpoints'] })
      toast({
        title: '同期完了',
        description: `${result.synced_models}件のモデルを同期しました`,
      })
    },
    onError: (error) => {
      toast({
        title: '同期失敗',
        description: String(error),
        variant: 'destructive',
      })
    },
  })

  const handleSave = () => {
    updateMutation.mutate({
      name: name !== endpoint?.name ? name : undefined,
      notes: notes !== endpoint?.notes ? notes : undefined,
      health_check_interval_secs:
        parseInt(healthCheckInterval) !== endpoint?.health_check_interval_secs
          ? parseInt(healthCheckInterval)
          : undefined,
      inference_timeout_secs:
        parseInt(inferenceTimeout) !== endpoint?.inference_timeout_secs
          ? parseInt(inferenceTimeout)
          : undefined,
    })
  }

  if (!endpoint) return null

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Server className="h-5 w-5" />
            {endpoint.name}
          </DialogTitle>
          <DialogDescription>{endpoint.base_url}</DialogDescription>
        </DialogHeader>

        <div className="space-y-6 py-4">
          {/* Status Section */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <Badge variant={getStatusBadgeVariant(endpoint.status)}>
                {getStatusLabel(endpoint.status)}
              </Badge>
              {endpoint.supports_responses_api && (
                <Badge variant="outline">Responses API対応</Badge>
              )}
              <span className="text-sm text-muted-foreground">
                モデル数: {endpoint.model_count}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => testMutation.mutate()}
                disabled={testMutation.isPending}
              >
                <Play className="h-4 w-4 mr-1" />
                接続テスト
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => syncMutation.mutate()}
                disabled={syncMutation.isPending || endpoint.status !== 'online'}
              >
                <RefreshCw className={`h-4 w-4 mr-1 ${syncMutation.isPending ? 'animate-spin' : ''}`} />
                モデル同期
              </Button>
            </div>
          </div>

          <Separator />

          {/* Info Section */}
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-muted-foreground">レイテンシ:</span>
              <span className="ml-2">{endpoint.latency_ms != null ? `${endpoint.latency_ms}ms` : '-'}</span>
            </div>
            <div>
              <span className="text-muted-foreground">登録日時:</span>
              <span className="ml-2">{formatRelativeTime(endpoint.registered_at)}</span>
            </div>
            <div>
              <span className="text-muted-foreground">最終確認:</span>
              <span className="ml-2">
                {endpoint.last_seen ? formatRelativeTime(endpoint.last_seen) : '-'}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">エラー回数:</span>
              <span className="ml-2">{endpoint.error_count}</span>
            </div>
          </div>

          {/* Error Message */}
          {endpoint.last_error && (
            <div className="bg-destructive/10 border border-destructive/20 rounded-md p-3">
              <div className="flex items-center gap-2 text-destructive">
                <AlertCircle className="h-4 w-4" />
                <span className="font-medium">最後のエラー</span>
              </div>
              <p className="text-sm text-destructive/80 mt-1">{endpoint.last_error}</p>
            </div>
          )}

          <Separator />

          {/* Edit Section */}
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="name">表示名</Label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="エンドポイント名"
              />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label htmlFor="healthCheckInterval">
                  <Clock className="h-4 w-4 inline mr-1" />
                  ヘルスチェック間隔（秒）
                </Label>
                <Input
                  id="healthCheckInterval"
                  type="number"
                  min="5"
                  max="3600"
                  value={healthCheckInterval}
                  onChange={(e) => setHealthCheckInterval(e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="inferenceTimeout">
                  <Clock className="h-4 w-4 inline mr-1" />
                  推論タイムアウト（秒）
                </Label>
                <Input
                  id="inferenceTimeout"
                  type="number"
                  min="10"
                  max="600"
                  value={inferenceTimeout}
                  onChange={(e) => setInferenceTimeout(e.target.value)}
                />
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="notes">メモ</Label>
              <Textarea
                id="notes"
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                placeholder="このエンドポイントに関するメモ..."
                rows={3}
              />
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            閉じる
          </Button>
          <Button onClick={handleSave} disabled={updateMutation.isPending}>
            <Save className="h-4 w-4 mr-1" />
            {updateMutation.isPending ? '保存中...' : '保存'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
