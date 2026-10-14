import type { DashboardEndpoint } from '@/lib/api'
import { cn } from '@/lib/utils'
import { useEndpointPlaygroundViewModel } from '@/viewmodels/useEndpointPlaygroundViewModel'
import { PlaygroundBase } from '@/components/playground'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Cpu, CircleDot, Loader2, Code } from 'lucide-react'

interface EndpointPlaygroundProps {
  endpointId: string
  onBack: () => void
}

function getStatusBadgeVariant(
  status: DashboardEndpoint['status'] | undefined
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

function getStatusIndicatorColor(status: DashboardEndpoint['status'] | undefined): string {
  switch (status) {
    case 'online':
      return 'text-success'
    case 'pending':
      return 'text-warning'
    case 'offline':
      return 'text-destructive/70'
    case 'error':
      return 'text-destructive'
    default:
      return 'text-muted-foreground'
  }
}

export default function EndpointPlayground({ endpointId, onBack }: EndpointPlaygroundProps) {
  const {
    pg, isLoadingEndpoint, endpointName, endpointBaseUrl, endpointStatus, statusLabel,
    models, isLoadingModels, selectedModelMaxTokens, hasBaseUrl, canSend, sendMessage, curlCommand,
  } = useEndpointPlaygroundViewModel(endpointId)

  if (isLoadingEndpoint) {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-4">
          <Loader2 className="h-8 w-8 animate-spin text-primary" />
          <p className="text-sm text-muted-foreground">Loading endpoint...</p>
        </div>
      </div>
    )
  }

  return (
    <PlaygroundBase
      onBack={onBack}
      sidebarWidth="w-64"
      sidebarHeader={
        <div className="flex items-center gap-2">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary/10">
            <Cpu className="h-4 w-4 text-primary" />
          </div>
          <div>
            <h1 className="font-semibold text-sm truncate" title={endpointName}>
              {endpointName || 'Endpoint'}
            </h1>
            <p className="text-xs text-muted-foreground">Playground</p>
          </div>
        </div>
      }
      sidebarInfo={
        <div className="p-3 space-y-2">
          <div className="text-xs text-muted-foreground">
            <span className="font-medium">URL:</span>{' '}
            <span className="truncate block" title={endpointBaseUrl}>
              {hasBaseUrl ? endpointBaseUrl : 'Not set'}
            </span>
          </div>
          {!hasBaseUrl && (
            <div className="text-xs text-destructive">
              Base URL is not configured. Please check the endpoint settings.
            </div>
          )}
          <div className="text-xs text-muted-foreground">
            <span className="font-medium">Status:</span>{' '}
            <Badge variant={getStatusBadgeVariant(endpointStatus)} className="text-xs">
              {statusLabel}
            </Badge>
          </div>
          <div className="text-xs text-muted-foreground">
            <span className="font-medium">Models:</span> {models.length}
          </div>
        </div>
      }
      headerContent={
        <div className="flex items-center gap-3">
          <Select value={pg.selectedModel} onValueChange={pg.setSelectedModel}>
            <SelectTrigger className="w-64">
              <SelectValue placeholder="Select a model" />
            </SelectTrigger>
            <SelectContent>
              {isLoadingModels ? (
                <SelectItem value="__loading__" disabled>Loading models...</SelectItem>
              ) : models.length === 0 ? (
                <SelectItem value="__no_models__" disabled>No models available</SelectItem>
              ) : (
                models.map((model) => (
                  <SelectItem key={model.model_id} value={model.model_id}>{model.model_id}</SelectItem>
                ))
              )}
            </SelectContent>
          </Select>

          <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <CircleDot className={cn("h-3 w-3", getStatusIndicatorColor(endpointStatus))} />
            {statusLabel}
          </span>

          {pg.streamEnabled && (
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
      messages={pg.messages}
      messagesEndRef={pg.messagesEndRef}
      emptyTitle="Start a conversation"
      emptyDescription="Select a model and send a message to get started."
      messageMaxWidth="max-w-3xl"
      input={pg.input}
      onInputChange={pg.setInput}
      onSend={() => void sendMessage()}
      onStop={pg.stopGeneration}
      isStreaming={pg.isStreaming}
      attachments={pg.attachments}
      onRemoveAttachment={pg.removeAttachment}
      onPaste={pg.handlePaste}
      inputRef={pg.inputRef}
      imageInputRef={pg.imageInputRef}
      audioInputRef={pg.audioInputRef}
      onImageAttach={(file) => void pg.handleFileAttachment(file, 'image')}
      onAudioAttach={(file) => void pg.handleFileAttachment(file, 'audio')}
      sendDisabled={!canSend}
      formMaxWidth="max-w-3xl"
      settingsOpen={pg.settingsOpen}
      onSettingsOpenChange={pg.setSettingsOpen}
      systemPrompt={pg.systemPrompt}
      onSystemPromptChange={pg.setSystemPrompt}
      streamEnabled={pg.streamEnabled}
      onStreamEnabledChange={pg.setStreamEnabled}
      temperature={pg.temperature}
      onTemperatureChange={pg.setTemperature}
      maxTokens={pg.maxTokens}
      onMaxTokensChange={pg.setMaxTokens}
      useMaxContext={pg.useMaxContext}
      onUseMaxContextChange={pg.setUseMaxContext}
      selectedModelMaxTokens={selectedModelMaxTokens}
      settingsDescription="Configure your chat preferences."
      curlOpen={pg.curlOpen}
      onCurlOpenChange={pg.setCurlOpen}
      curlCommand={curlCommand}
      copied={pg.copied}
      onCopyCurl={pg.handleCopyCurl}
      curlCopyDisabled={!hasBaseUrl}
      curlDescription="Copy this command to replicate the API call."
      resetChat={pg.resetChat}
    />
  )
}
