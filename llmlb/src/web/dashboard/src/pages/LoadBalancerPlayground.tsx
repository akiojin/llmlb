import { cn } from '@/lib/utils'
import { PlaygroundBase } from '@/components/playground'
import { useLoadBalancerPlaygroundViewModel } from '@/viewmodels/useLoadBalancerPlaygroundViewModel'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Network,
  MessageSquare,
  Send,
  AlertCircle,
  RefreshCw,
  Gauge,
  Play,
  Square,
  Loader2,
  Code,
  ShieldCheck,
} from 'lucide-react'

interface LoadBalancerPlaygroundProps {
  onBack: () => void
  initialModel?: string
}

export default function LoadBalancerPlayground({ onBack, initialModel }: LoadBalancerPlaygroundProps) {
  const {
    pg, isAdmin, mode, setMode,
    loadTestTotalRequests, setLoadTestTotalRequests,
    loadTestConcurrency, setLoadTestConcurrency,
    loadTestIntervalMs, setLoadTestIntervalMs,
    isLoadTesting, isStoppingLoadTest, loadTestProgress,
    loadTestProgressLabel, loadTestOutcomeLabel,
    distributionSummary, distributionError, isRefreshingDistribution, refreshDistribution,
    modelOptions, isLoadingModels, refetchModels,
    selectedModelMaxTokens, canSendChat, progressRate,
    sendMessage, startLoadTest, stopLoadTest, curlCommand,
  } = useLoadBalancerPlaygroundViewModel(initialModel)

  const loadTestSettingsPanel = mode === 'load_test' ? (
    <div className="rounded-lg border bg-muted/20 p-3 space-y-3" id="lb-load-test-settings">
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium">Load test settings</p>
        <Badge variant="secondary">Default: High</Badge>
      </div>
      <div className="grid grid-cols-3 gap-3">
        <div className="space-y-1">
          <Label htmlFor="lb-total-requests" className="text-xs">Requests</Label>
          <Input
            id="lb-total-requests"
            type="number"
            min={1}
            value={loadTestTotalRequests}
            onChange={(e) => setLoadTestTotalRequests(e.target.value)}
            disabled={isLoadTesting}
          />
        </div>
        <div className="space-y-1">
          <Label htmlFor="lb-concurrency" className="text-xs">Concurrency</Label>
          <Input
            id="lb-concurrency"
            type="number"
            min={1}
            value={loadTestConcurrency}
            onChange={(e) => setLoadTestConcurrency(e.target.value)}
            disabled={isLoadTesting}
          />
        </div>
        <div className="space-y-1">
          <Label htmlFor="lb-interval-ms" className="text-xs">Interval (ms)</Label>
          <Input
            id="lb-interval-ms"
            type="number"
            min={0}
            value={loadTestIntervalMs}
            onChange={(e) => setLoadTestIntervalMs(e.target.value)}
            disabled={isLoadTesting}
          />
        </div>
      </div>

      {loadTestProgress && (
        <div className="space-y-2" id="lb-load-test-progress">
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span>{loadTestProgressLabel}</span>
            <span>{loadTestOutcomeLabel}</span>
          </div>
          <div className="h-2 rounded-full bg-muted">
            <div
              className="h-2 rounded-full bg-primary transition-all"
              style={{ width: `${progressRate}%` }}
            />
          </div>
        </div>
      )}
    </div>
  ) : null

  const sendButtonElement = mode === 'chat' ? (
    pg.isStreaming ? (
      <Button variant="destructive" onClick={pg.stopGeneration} className="shrink-0" id="lb-stop-chat">
        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
        Stop
      </Button>
    ) : (
      <Button
        onClick={() => void sendMessage()}
        disabled={!canSendChat}
        className="shrink-0"
        id="lb-send-chat"
      >
        <Send className="mr-2 h-4 w-4" />
        Send
      </Button>
    )
  ) : isLoadTesting ? (
    <Button
      variant="destructive"
      onClick={stopLoadTest}
      className="shrink-0"
      id="lb-stop-load-test"
    >
      <Square className="mr-2 h-4 w-4" />
      {isStoppingLoadTest ? 'Stopping...' : 'Stop'}
    </Button>
  ) : (
    <Button
      onClick={() => void startLoadTest()}
      disabled={!pg.selectedModel}
      className="shrink-0"
      id="lb-start-load-test"
    >
      <Play className="mr-2 h-4 w-4" />
      Start Load Test
    </Button>
  )

  const distributionPanel = (distributionSummary || distributionError || isRefreshingDistribution) ? (
    <div className="border-b bg-muted/20 px-4 py-3" id="lb-distribution-panel">
      <div className="mb-2 flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm font-medium">
          <Network className="h-4 w-4 text-primary" />
          Request Distribution
        </div>
        {distributionSummary && (
          <Button
            variant="outline"
            size="sm"
            onClick={refreshDistribution}
            disabled={isRefreshingDistribution}
          >
            <RefreshCw className={cn('mr-2 h-3.5 w-3.5', isRefreshingDistribution && 'animate-spin')} />
            Refresh
          </Button>
        )}
      </div>

      {distributionError && (
        <div className="flex items-center gap-2 text-sm text-destructive">
          <AlertCircle className="h-4 w-4" />
          {distributionError}
        </div>
      )}

      {distributionSummary && (
        <div className="space-y-2">
          <p className="text-xs text-muted-foreground" id="lb-distribution-summary">
            {distributionSummary.label}
          </p>
          <div className="rounded-md border bg-background">
            <div className="grid grid-cols-[1fr_80px_80px_80px_120px] border-b px-3 py-2 text-xs font-medium text-muted-foreground">
              <span>Endpoint</span>
              <span className="text-right">Count</span>
              <span className="text-right">Success</span>
              <span className="text-right">Error</span>
              <span className="text-right">Avg ms</span>
            </div>
            {distributionSummary.rows.length === 0 ? (
              <p className="px-3 py-3 text-xs text-muted-foreground">
                No records found for this run yet.
              </p>
            ) : (
              distributionSummary.rows.map((row) => (
                <div
                  key={row.endpoint}
                  className="grid grid-cols-[1fr_80px_80px_80px_120px] px-3 py-2 text-xs"
                  data-testid="lb-distribution-row"
                >
                  <span className="truncate" title={row.endpoint}>{row.endpoint}</span>
                  <span className="text-right">{row.count}</span>
                  <span className="text-right text-green-600">{row.success}</span>
                  <span className="text-right text-destructive">{row.error}</span>
                  <span className="text-right">{row.averageDurationMs}</span>
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  ) : null

  return (
    <PlaygroundBase
      onBack={onBack}
      sidebarWidth="w-72"
      sidebarId="lb-playground-sidebar"
      sidebarHeader={
        <div className="flex items-center gap-2">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary/10">
            <Network className="h-4 w-4 text-primary" />
          </div>
          <div>
            <h1 className="font-semibold text-sm">Load Balancer</h1>
            <p className="text-xs text-muted-foreground">Playground</p>
          </div>
        </div>
      }
      sidebarInfo={
        <div className="p-3 space-y-2">
          <div
            id="lb-session-auth"
            className="text-xs text-muted-foreground flex items-center gap-2"
          >
            <ShieldCheck className="h-3 w-3" />
            <span className="font-medium">Dashboard session:</span>
            <Badge variant="default">Active</Badge>
          </div>
          <div className="text-xs text-muted-foreground">
            <span className="font-medium">Models:</span> {modelOptions.length}
          </div>
          <div className="text-xs text-muted-foreground">
            <span className="font-medium">Mode:</span>{' '}
            {mode === 'chat' ? 'Interactive chat' : 'Load test'}
          </div>
        </div>
      }
      sidebarExtra={
        <>
          <Button
            variant="outline"
            className="w-full justify-start"
            onClick={() => refetchModels()}
            disabled={isLoadingModels}
          >
            <RefreshCw className={cn('mr-2 h-4 w-4', isLoadingModels && 'animate-spin')} />
            Refresh Models
          </Button>
        </>
      }
      headerContent={
        <div className="flex items-center gap-3">
          <div className="inline-flex rounded-md border border-border p-1">
            <Button
              size="sm"
              variant={mode === 'chat' ? 'default' : 'outline'}
              onClick={() => setMode('chat')}
              className="h-7"
              id="lb-mode-chat"
            >
              <MessageSquare className="mr-1.5 h-3.5 w-3.5" />
              Chat
            </Button>
            {isAdmin && (
              <Button
                size="sm"
                variant={mode === 'load_test' ? 'default' : 'outline'}
                onClick={() => setMode('load_test')}
                className="h-7"
                id="lb-mode-load-test"
              >
                <Gauge className="mr-1.5 h-3.5 w-3.5" />
                Load Test
              </Button>
            )}
          </div>

          <Select value={pg.selectedModel} onValueChange={pg.setSelectedModel}>
            <SelectTrigger className="w-80" id="lb-model-select">
              <SelectValue placeholder="Select a model" />
            </SelectTrigger>
            <SelectContent>
              {isLoadingModels ? (
                <SelectItem value="__loading__" disabled>Loading models...</SelectItem>
              ) : modelOptions.length === 0 ? (
                <SelectItem value="__empty__" disabled>No models available</SelectItem>
              ) : (
                modelOptions.map((model) => (
                  <SelectItem key={model.id} value={model.id}>{model.id}</SelectItem>
                ))
              )}
            </SelectContent>
          </Select>

          {mode === 'chat' && pg.streamEnabled && (
            <Badge variant="secondary" className="text-xs">Streaming</Badge>
          )}
        </div>
      }
      headerRight={
        <Button variant="outline" size="sm" onClick={() => pg.setCurlOpen(true)}>
          <Code className="mr-2 h-4 w-4" />
          cURL
        </Button>
      }
      aboveMessages={distributionPanel}
      messages={pg.messages}
      messagesEndRef={pg.messagesEndRef}
      emptyTitle="Start a load balancer conversation"
      emptyDescription="Choose a model and send requests through the load balancer."
      input={pg.input}
      onInputChange={pg.setInput}
      onSend={() => {
        if (mode === 'chat') void sendMessage()
      }}
      onStop={pg.stopGeneration}
      isStreaming={pg.isStreaming}
      inputDisabled={pg.isStreaming || isLoadTesting}
      attachments={pg.attachments}
      onRemoveAttachment={pg.removeAttachment}
      onPaste={(e) => {
        if (mode !== 'chat') return
        pg.handlePaste(e)
      }}
      inputRef={pg.inputRef}
      imageInputRef={pg.imageInputRef}
      audioInputRef={pg.audioInputRef}
      onImageAttach={(file) => void pg.handleFileAttachment(file, 'image')}
      onAudioAttach={(file) => void pg.handleFileAttachment(file, 'audio')}
      sendDisabled={!canSendChat}
      inputPlaceholder={
        mode === 'chat'
          ? 'Type a message or attach files...'
          : 'Prompt used for each load test request...'
      }
      showAttachButtons={mode === 'chat'}
      formExtraContent={loadTestSettingsPanel}
      sendButton={sendButtonElement}
      inputId="lb-chat-input"
      settingsOpen={pg.settingsOpen}
      onSettingsOpenChange={pg.setSettingsOpen}
      systemPrompt={pg.systemPrompt}
      onSystemPromptChange={pg.setSystemPrompt}
      streamEnabled={pg.streamEnabled}
      onStreamEnabledChange={pg.setStreamEnabled}
      streamDisabled={mode === 'load_test'}
      temperature={pg.temperature}
      onTemperatureChange={pg.setTemperature}
      maxTokens={pg.maxTokens}
      onMaxTokensChange={pg.setMaxTokens}
      useMaxContext={pg.useMaxContext}
      onUseMaxContextChange={pg.setUseMaxContext}
      selectedModelMaxTokens={selectedModelMaxTokens}
      maxContextCheckboxId="lb-use-max-context"
      curlOpen={pg.curlOpen}
      onCurlOpenChange={pg.setCurlOpen}
      curlCommand={curlCommand}
      copied={pg.copied}
      onCopyCurl={pg.handleCopyCurl}
      curlDescription="Copy this command to replay the current request through the load balancer."
      resetChat={pg.resetChat}
    />
  )
}
