import { type DashboardEndpoint, type EndpointType } from '@/lib/api'
import { useEndpointDetailViewModel } from '@/viewmodels/useEndpointDetailViewModel'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Server,
  Clock,
  AlertCircle,
  Save,
  Play,
  RefreshCw,
  MessageSquare,
  Download,
  Activity,
} from 'lucide-react'
import { ModelDownloadDialog } from './ModelDownloadDialog'
import { EndpointModelsTable } from './EndpointModelsTable'
import { EndpointRequestChart } from './EndpointRequestChart'

/**
 * SPEC-e8e9326e: Router-Driven Endpoint Registration System
 * Endpoint Detail Modal
 */

interface EndpointDetailModalProps {
  endpoint: DashboardEndpoint | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

function getStatusBadgeVariant(
  status: DashboardEndpoint['status']
): 'online' | 'pending' | 'offline' | 'destructive' | 'outline' {
  switch (status) {
    case 'online':
      return 'online'
    case 'pending':
      return 'pending'
    case 'offline':
      return 'offline'
    case 'error':
      return 'destructive'
    default:
      return 'outline'
  }
}

function getTypeBadgeVariant(
  type: EndpointType | undefined
): 'default' | 'secondary' | 'outline' {
  switch (type) {
    case 'xllm':
      return 'default'
    case 'ollama':
    case 'vllm':
    case 'lm_studio':
    case 'llamacpp':
      return 'secondary'
    default:
      return 'outline'
  }
}

export function EndpointDetailModal({ endpoint, open, onOpenChange }: EndpointDetailModalProps) {
  if (!endpoint) return null

  return (
    <EndpointDetailModalContent
      key={endpoint.id}
      endpoint={endpoint}
      open={open}
      onOpenChange={onOpenChange}
    />
  )
}

interface EndpointDetailModalContentProps {
  endpoint: DashboardEndpoint
  open: boolean
  onOpenChange: (open: boolean) => void
}

function EndpointDetailModalContent({
  endpoint,
  open,
  onOpenChange,
}: EndpointDetailModalContentProps) {
  const {
    name, setName, notes, setNotes, healthCheckInterval, setHealthCheckInterval,
    inferenceTimeout, setInferenceTimeout, downloadDialogOpen, setDownloadDialogOpen,
    recommendedInferenceTimeoutLabel, differsFromRecommendedTimeout,
    isLoadingTodayStats, todayRequestsLabel, totalRequestsLabel, modelCountLabel, successRateLabel, requestHealth,
    statusLabel, typeLabel, latencyLabel, registeredLabel, lastSeenLabel, errorLabel,
    isSaving, isTesting, isSyncing, handleSave, testConnection, syncModels, openPlayground,
  } = useEndpointDetailViewModel(endpoint, open, onOpenChange)

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

        <ScrollArea className="max-h-[calc(100vh-12rem)]">
        <div className="space-y-6 py-4">
          {/* Status Section */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <Badge variant={getStatusBadgeVariant(endpoint.status)}>
                {statusLabel}
              </Badge>
              <Badge variant={getTypeBadgeVariant(endpoint.endpoint_type)}>
                {typeLabel}
              </Badge>
              <span className="text-xs text-muted-foreground">
                Type is auto-detected
              </span>
              <span className="text-sm text-muted-foreground">
                {modelCountLabel}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={testConnection}
                disabled={isTesting}
              >
                <Play className="h-4 w-4 mr-1" />
                Test Connection
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={syncModels}
                disabled={isSyncing || endpoint.status !== 'online'}
              >
                <RefreshCw className={`h-4 w-4 mr-1 ${isSyncing ? 'animate-spin' : ''}`} />
                Sync Models
              </Button>
            </div>
          </div>

          <Separator />

          {/* SPEC-8c32349f: Request Statistics Cards */}
          <div className="grid grid-cols-2 gap-4">
            {/* Total Requests */}
            <div className="rounded-lg border p-3">
              <div className="flex items-center gap-1.5 mb-1">
                <Activity className="h-3.5 w-3.5 text-muted-foreground" />
                <span className="text-xs text-muted-foreground">Total Requests</span>
              </div>
              <span className="text-xl font-bold">
                {totalRequestsLabel}
              </span>
            </div>

            {/* Today's Requests */}
            <div className="rounded-lg border p-3">
              <div className="flex items-center gap-1.5 mb-1">
                <Activity className="h-3.5 w-3.5 text-muted-foreground" />
                <span className="text-xs text-muted-foreground">Today</span>
              </div>
              {isLoadingTodayStats ? (
                <div className="h-7 w-16 rounded bg-muted animate-pulse" />
              ) : (
                <span className="text-xl font-bold">
                  {todayRequestsLabel}
                </span>
              )}
            </div>

            {/* Success Rate */}
            <div className="rounded-lg border p-3">
              <div className="flex items-center gap-1.5 mb-1">
                <Activity className="h-3.5 w-3.5 text-muted-foreground" />
                <span className="text-xs text-muted-foreground">Success Rate</span>
              </div>
              <span className={`text-xl font-bold ${requestHealth === 'error' ? 'text-red-600' : requestHealth === 'warning' ? 'text-yellow-600' : ''}`}>
                {successRateLabel}
              </span>
            </div>

            {/* Average Response Time */}
            <div className="rounded-lg border p-3">
              <div className="flex items-center gap-1.5 mb-1">
                <Clock className="h-3.5 w-3.5 text-muted-foreground" />
                <span className="text-xs text-muted-foreground">Avg Response</span>
              </div>
              <span className="text-xl font-bold">
                {latencyLabel}
              </span>
            </div>
          </div>

          <Separator />

          {/* SPEC-8c32349f: Daily Request Trend Chart (Phase 6) */}
          <EndpointRequestChart endpointId={endpoint.id} />

          <Separator />

          {/* Info Section */}
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-muted-foreground">Latency:</span>
              <span className="ml-2">{latencyLabel}</span>
            </div>
            <div>
              <span className="text-muted-foreground">Registered:</span>
              <span className="ml-2">{registeredLabel}</span>
            </div>
            <div>
              <span className="text-muted-foreground">Last Seen:</span>
              <span className="ml-2">
                {lastSeenLabel}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">Error Count:</span>
              <span className="ml-2">{endpoint.error_count}</span>
            </div>
          </div>

          {/* Error Message */}
          {endpoint.last_error && (
            <div className="bg-destructive/10 border border-destructive/20 rounded-md p-3">
              <div className="flex items-center gap-2 text-destructive">
                <AlertCircle className="h-4 w-4" />
                <span className="font-medium">Last Error</span>
                {errorLabel && (
                  <Badge variant="outline" className="border-destructive/40 text-destructive">
                    {errorLabel}
                  </Badge>
                )}
              </div>
              <p className="text-sm text-destructive/80 mt-1">{endpoint.last_error}</p>
            </div>
          )}

          <Separator />

          {/* SPEC-8c32349f + SPEC-4bb5b55f: Unified Models Table with TPS and Stats */}
          <EndpointModelsTable
            endpointId={endpoint.id}
            enabled={open}
            headerActions={
              <div className="flex items-center gap-2">
                {endpoint.endpoint_type === 'xllm' && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setDownloadDialogOpen(true)}
                    disabled={endpoint.status !== 'online'}
                  >
                    <Download className="h-4 w-4 mr-1" />
                    Download Model
                  </Button>
                )}
                <Button
                  variant="default"
                  size="sm"
                  onClick={openPlayground}
                  disabled={endpoint.status !== 'online'}
                >
                  <MessageSquare className="h-4 w-4 mr-1" />
                  Open Playground
                </Button>
              </div>
            }
          />

          <Separator />

          {/* Edit Section */}
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="name">Display Name</Label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Endpoint name"
              />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label htmlFor="healthCheckInterval">
                  <Clock className="h-4 w-4 inline mr-1" />
                  Health Check Interval (sec)
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
                  Inference Timeout (sec)
                </Label>
                <Input
                  id="inferenceTimeout"
                  type="number"
                  min="10"
                  max="600"
                  value={inferenceTimeout}
                  onChange={(e) => setInferenceTimeout(e.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                  {recommendedInferenceTimeoutLabel}
                </p>
                {differsFromRecommendedTimeout && (
                  <p className="text-xs text-muted-foreground">
                    Current value differs from the recommended default for this endpoint type.
                  </p>
                )}
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="notes">Notes</Label>
              <Textarea
                id="notes"
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                placeholder="Notes about this endpoint..."
                rows={3}
              />
            </div>
          </div>
        </div>
        </ScrollArea>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
          <Button onClick={handleSave} disabled={isSaving}>
            <Save className="h-4 w-4 mr-1" />
            {isSaving ? 'Saving...' : 'Save'}
          </Button>
        </DialogFooter>
      </DialogContent>

      {/* xLLM Model Download Dialog */}
      <ModelDownloadDialog
        endpoint={endpoint}
        open={downloadDialogOpen}
        onOpenChange={setDownloadDialogOpen}
      />
    </Dialog>
  )
}
