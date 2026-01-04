import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { type DashboardNode, type LogEntry, nodesApi, dashboardApi } from '@/lib/api'
import { formatUptime, formatPercentage, formatRelativeTime, formatBytes, cn } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts'
import {
  Server,
  Cpu,
  HardDrive,
  Zap,
  Download,
  Clock,
  Activity,
  Save,
  Trash2,
  Unplug,
  RefreshCw,
} from 'lucide-react'

interface NodeDetailModalProps {
  node: DashboardNode | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function NodeDetailModal({ node, open, onOpenChange }: NodeDetailModalProps) {
  const queryClient = useQueryClient()
  const [customName, setCustomName] = useState(node?.custom_name || '')
  const [tags, setTags] = useState(node?.tags?.join(', ') || '')
  const [notes, setNotes] = useState(node?.notes || '')

  // Fetch node metrics
  const { data: metrics } = useQuery({
    queryKey: ['node-metrics', node?.node_id],
    queryFn: () => dashboardApi.getNodeMetrics(node!.node_id),
    enabled: !!node?.node_id && open,
    refetchInterval: 5000,
  })

  // Fetch node logs
  const { data: logs, refetch: refetchLogs } = useQuery({
    queryKey: ['node-logs', node?.node_id],
    queryFn: () => nodesApi.getLogs(node!.node_id, { limit: 100 }),
    enabled: !!node?.node_id && open,
  })

  // Update settings mutation
  const updateMutation = useMutation({
    mutationFn: () =>
      nodesApi.updateSettings(node!.node_id, {
        custom_name: customName || undefined,
        tags: tags ? tags.split(',').map((t) => t.trim()) : undefined,
        notes: notes || undefined,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dashboard-overview'] })
      toast({ title: 'Settings saved', variant: 'default' })
    },
    onError: () => {
      toast({ title: 'Failed to save settings', variant: 'destructive' })
    },
  })

  // Disconnect mutation
  const disconnectMutation = useMutation({
    mutationFn: () => nodesApi.disconnect(node!.node_id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dashboard-overview'] })
      toast({ title: 'Node disconnected' })
      onOpenChange(false)
    },
    onError: () => {
      toast({ title: 'Failed to disconnect', variant: 'destructive' })
    },
  })

  // Approve mutation (pending only)
  const approveMutation = useMutation({
    mutationFn: () => nodesApi.approve(node!.node_id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dashboard-overview'] })
      toast({ title: 'Node approved' })
    },
    onError: () => {
      toast({ title: 'Failed to approve', variant: 'destructive' })
    },
  })

  // Delete mutation
  const deleteMutation = useMutation({
    mutationFn: () => nodesApi.delete(node!.node_id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dashboard-overview'] })
      toast({ title: 'Node deleted' })
      onOpenChange(false)
    },
    onError: () => {
      toast({ title: 'Failed to delete', variant: 'destructive' })
    },
  })

  // Reset form when node changes
  if (node && (customName !== (node.custom_name || '') && !open)) {
    setCustomName(node.custom_name || '')
    setTags(node.tags?.join(', ') || '')
    setNotes(node.notes || '')
  }

  if (!node) return null

  // Sanitize metrics data to handle null values (Recharts Tooltip calls toFixed on values)
  const metricsData = ((metrics as Array<{
    timestamp: string
    cpu_usage: number | null
    memory_usage: number | null
    gpu_usage?: number | null
  }>) || []).map(m => ({
    ...m,
    cpu_usage: m.cpu_usage ?? undefined,
    memory_usage: m.memory_usage ?? undefined,
    gpu_usage: m.gpu_usage ?? undefined,
  }))

  const syncBadgeVariant = (state?: DashboardNode['sync_state']) => {
    switch (state) {
      case 'running':
        return 'warning'
      case 'success':
        return 'success'
      case 'failed':
        return 'destructive'
      case 'idle':
        return 'secondary'
      default:
        return 'outline'
    }
  }

  const syncProgress = node.sync_progress
  const syncPercent =
    syncProgress && syncProgress.total_bytes > 0
      ? Math.round((syncProgress.downloaded_bytes / syncProgress.total_bytes) * 100)
      : null
  const syncFallbackText =
    node.sync_state === 'running'
      ? 'Waiting for download'
      : node.sync_state === 'success'
      ? 'Sync complete'
      : node.sync_state === 'failed'
      ? 'Sync failed'
      : 'No sync activity'

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl max-h-[90vh] overflow-hidden">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Server className="h-5 w-5" />
            {node.custom_name || node.machine_name}
          </DialogTitle>
          <DialogDescription>
            Node ID: <code className="text-xs">{node.node_id}</code>
          </DialogDescription>
        </DialogHeader>

        <Tabs defaultValue="overview" className="mt-4">
          <TabsList className="grid w-full grid-cols-4">
            <TabsTrigger value="overview">Overview</TabsTrigger>
            <TabsTrigger value="metrics">Metrics</TabsTrigger>
            <TabsTrigger value="logs">Logs</TabsTrigger>
            <TabsTrigger value="settings">Settings</TabsTrigger>
          </TabsList>

          {/* Overview Tab */}
          <TabsContent value="overview" className="space-y-4 mt-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-1">
                <p className="text-sm text-muted-foreground">Status</p>
                <Badge
                  variant={
                    node.status === 'online'
                      ? 'online'
                      : node.status === 'offline'
                      ? 'offline'
                      : 'pending'
                  }
                >
                  {node.status}
                </Badge>
              </div>
              <div className="space-y-1">
                <p className="text-sm text-muted-foreground">IP Address</p>
                <p className="font-mono text-sm">
                  {node.ip_address}:{node.port}
                </p>
              </div>
              <div className="space-y-1">
                <p className="text-sm text-muted-foreground">Runtime Version</p>
                <p className="text-sm">{node.runtime_version}</p>
              </div>
              <div className="space-y-1">
                <p className="text-sm text-muted-foreground">Uptime</p>
                <p className="text-sm">{formatUptime(node.uptime_seconds)}</p>
              </div>
              <div className="space-y-1">
                <p className="text-sm text-muted-foreground">Total Requests</p>
                <p className="text-sm">{node.total_requests.toLocaleString()}</p>
              </div>
              <div className="space-y-1">
                <p className="text-sm text-muted-foreground">Last Seen</p>
                <p className="text-sm">{formatRelativeTime(node.last_seen)}</p>
              </div>
            </div>

            {node.gpu_model ? <Separator /> : null}

            {/* GPU Info */}
            {node.gpu_model && (
              <div className="space-y-2">
                <h4 className="text-sm font-medium flex items-center gap-2">
                  <Zap className="h-4 w-4" />
                  GPU Information
                </h4>
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-1">
                    <p className="text-sm text-muted-foreground">Model</p>
                    <p className="text-sm">{node.gpu_model}</p>
                  </div>
                  <div className="space-y-1">
                    <p className="text-sm text-muted-foreground">Count</p>
                    <p className="text-sm">{node.gpu_count || 1}</p>
                  </div>
                  <div className="space-y-1">
                    <p className="text-sm text-muted-foreground">Usage</p>
                    <p className="text-sm">
                      {node.gpu_usage !== undefined
                        ? formatPercentage(node.gpu_usage)
                        : '—'}
                    </p>
                  </div>
                  <div className="space-y-1">
                    <p className="text-sm text-muted-foreground">Memory</p>
                    <p className="text-sm">
                      {node.gpu_memory_used_mb !== undefined && node.gpu_memory_total_mb
                        ? `${node.gpu_memory_used_mb.toLocaleString()} / ${node.gpu_memory_total_mb.toLocaleString()} MB`
                        : '—'}
                    </p>
                  </div>
                </div>
              </div>
            )}

            <Separator />

            {/* Model Sync */}
            <div className="space-y-2">
              <h4 className="text-sm font-medium flex items-center gap-2">
                <Download className="h-4 w-4" />
                Model Sync
              </h4>
              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-1">
                  <p className="text-sm text-muted-foreground">State</p>
                  {node.sync_state ? (
                    <Badge
                      variant={syncBadgeVariant(node.sync_state)}
                      className="w-fit capitalize"
                    >
                      {node.sync_state}
                    </Badge>
                  ) : (
                    <p className="text-sm">—</p>
                  )}
                </div>
                <div className="space-y-1">
                  <p className="text-sm text-muted-foreground">Last Updated</p>
                  <p className="text-sm">
                    {formatRelativeTime(node.sync_updated_at)}
                  </p>
                </div>
              </div>
              {syncProgress ? (
                <div className="rounded-lg border p-3 space-y-2">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div className="text-xs font-mono">
                      {syncProgress.model_id}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {syncProgress.file}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <div className="h-2 w-full rounded-full bg-muted">
                      <div
                        className={cn(
                          'h-full rounded-full transition-all',
                          syncPercent !== null && syncPercent > 80
                            ? 'bg-destructive'
                            : syncPercent !== null && syncPercent > 50
                            ? 'bg-warning'
                            : 'bg-success'
                        )}
                        style={{ width: `${syncPercent ?? 0}%` }}
                      />
                    </div>
                    <span className="text-xs">
                      {syncPercent !== null ? `${syncPercent}%` : '—'}
                    </span>
                  </div>
                  <div className="text-xs text-muted-foreground">
                    {formatBytes(syncProgress.downloaded_bytes)} /{' '}
                    {formatBytes(syncProgress.total_bytes)}
                  </div>
                </div>
              ) : (
                <p className="text-sm text-muted-foreground">
                  {syncFallbackText}
                </p>
              )}
            </div>

            {/* Ready Models */}
            {node.ready_models && node.ready_models.length > 0 && (
              <div className="space-y-2">
                <h4 className="text-sm font-medium">Ready Models</h4>
                <div className="flex flex-wrap gap-2">
                  {node.ready_models.map((model) => (
                    <Badge key={model} variant="secondary">
                      {model}
                    </Badge>
                  ))}
                </div>
              </div>
            )}
          </TabsContent>

          {/* Metrics Tab */}
          <TabsContent value="metrics" className="space-y-4 mt-4">
            {metricsData.length > 0 ? (
              <div className="h-64">
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={metricsData}>
                    <CartesianGrid strokeDasharray="3 3" className="stroke-border" />
                    <XAxis
                      dataKey="timestamp"
                      tick={{ fontSize: 10 }}
                      tickFormatter={(value) =>
                        new Date(value).toLocaleTimeString()
                      }
                    />
                    <YAxis domain={[0, 100]} tick={{ fontSize: 10 }} />
                    <Tooltip
                      contentStyle={{
                        backgroundColor: 'hsl(var(--popover))',
                        border: '1px solid hsl(var(--border))',
                        borderRadius: '8px',
                      }}
                    />
                    <Line
                      type="monotone"
                      dataKey="cpu_usage"
                      stroke="hsl(var(--chart-1))"
                      name="CPU"
                      strokeWidth={2}
                      dot={false}
                    />
                    <Line
                      type="monotone"
                      dataKey="memory_usage"
                      stroke="hsl(var(--chart-2))"
                      name="Memory"
                      strokeWidth={2}
                      dot={false}
                    />
                    <Line
                      type="monotone"
                      dataKey="gpu_usage"
                      stroke="hsl(var(--chart-3))"
                      name="GPU"
                      strokeWidth={2}
                      dot={false}
                    />
                  </LineChart>
                </ResponsiveContainer>
              </div>
            ) : (
              <div className="flex h-32 items-center justify-center text-muted-foreground">
                No metrics data available
              </div>
            )}
          </TabsContent>

          {/* Logs Tab */}
          <TabsContent value="logs" className="space-y-4 mt-4">
            <div className="flex justify-end">
              <Button variant="outline" size="sm" onClick={() => refetchLogs()}>
                <RefreshCw className="mr-2 h-4 w-4" />
                Refresh
              </Button>
            </div>
            <ScrollArea className="h-64 rounded-md border">
              <div className="p-4 font-mono text-xs space-y-1">
                {logs?.entries?.length ? (
                  (logs.entries as LogEntry[]).map((log, i) => (
                    <div key={i} className="flex gap-2">
                      <span className="text-muted-foreground">
                        {new Date(log.timestamp).toLocaleTimeString()}
                      </span>
                      <span
                        className={cn(
                          'uppercase w-12',
                          log.level === 'error'
                            ? 'text-destructive'
                            : log.level === 'warn'
                            ? 'text-warning'
                            : 'text-muted-foreground'
                        )}
                      >
                        {log.level}
                      </span>
                      <span>{log.message || ''}</span>
                    </div>
                  ))
                ) : (
                  <div className="text-muted-foreground">No logs available</div>
                )}
              </div>
            </ScrollArea>
          </TabsContent>

          {/* Settings Tab */}
          <TabsContent value="settings" className="space-y-4 mt-4">
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="custom-name">Custom Name</Label>
                <Input
                  id="custom-name"
                  value={customName}
                  onChange={(e) => setCustomName(e.target.value)}
                  placeholder="Enter a custom name..."
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="tags">Tags (comma separated)</Label>
                <Input
                  id="tags"
                  value={tags}
                  onChange={(e) => setTags(e.target.value)}
                  placeholder="gpu, production, etc..."
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="notes">Notes</Label>
                <Textarea
                  id="notes"
                  value={notes}
                  onChange={(e) => setNotes(e.target.value)}
                  placeholder="Add notes about this node..."
                  rows={3}
                />
              </div>
            </div>
          </TabsContent>
        </Tabs>

        <DialogFooter className="flex-col sm:flex-row gap-2 mt-4">
          <div className="flex gap-2">
            {node.status === 'pending' && (
              <Button
                variant="outline"
                onClick={() => approveMutation.mutate()}
                disabled={approveMutation.isPending}
              >
                Approve
              </Button>
            )}
            <Button
              variant="outline"
              onClick={() => disconnectMutation.mutate()}
              disabled={disconnectMutation.isPending}
            >
              <Unplug className="mr-2 h-4 w-4" />
              Disconnect
            </Button>
            <Button
              variant="destructive"
              onClick={() => deleteMutation.mutate()}
              disabled={deleteMutation.isPending}
            >
              <Trash2 className="mr-2 h-4 w-4" />
              Delete
            </Button>
          </div>
          <Button
            onClick={() => updateMutation.mutate()}
            disabled={updateMutation.isPending}
          >
            <Save className="mr-2 h-4 w-4" />
            Save Changes
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
