import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { modelsApi, type ModelWithStatus, type ModelStatus } from '@/lib/api'
import { toast } from '@/hooks/use-toast'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Search, Download, Check, Loader2, HardDrive, Cpu, ThumbsUp } from 'lucide-react'
import { cn } from '@/lib/utils'

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

function formatDownloads(downloads?: number): string {
  if (!downloads) return '—'
  if (downloads >= 1_000_000) return `${(downloads / 1_000_000).toFixed(1)}M`
  if (downloads >= 1_000) return `${(downloads / 1_000).toFixed(1)}K`
  return String(downloads)
}

function statusBadge(status: ModelStatus, lifecycle?: string) {
  switch (status) {
    case 'downloading':
      return (
        <Badge variant="secondary" data-model-status="downloading">
          Preparing
        </Badge>
      )
    case 'downloaded':
      return (
        <Badge variant="online" className="gap-1" data-model-status="ready">
          <Check className="h-3 w-3" />
          Ready
        </Badge>
      )
    default:
      if (lifecycle === 'registered') {
        return (
          <Badge variant="secondary" data-model-status="registered">
            Registered
          </Badge>
        )
      }
      return (
        <Badge variant="outline" data-model-status="available">
          Available
        </Badge>
      )
  }
}

interface ModelCardProps {
  model: ModelWithStatus
  onRegister: (model: ModelWithStatus) => void
  isRegistering: boolean
}

function ModelCard({ model, onRegister, isRegistering }: ModelCardProps) {
  const isRegistered = model.lifecycle_status === 'registered'
  const canRegister = model.status === 'available' && !isRegistered

  return (
    <Card
      className="model-card overflow-hidden transition-shadow hover:shadow-md"
      data-model-card="true"
      data-model-id={model.id}
      data-testid="model-card"
    >
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0 space-y-1">
            <h4 className="truncate font-medium" data-model-name={model.name}>
              {model.name}
            </h4>
            <p
              className="text-xs text-muted-foreground line-clamp-2"
              data-model-description={model.description}
            >
              {model.description}
            </p>
          </div>
          {statusBadge(model.status, model.lifecycle_status)}
        </div>

        {/* Model Info */}
        <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
          <span
            className="flex items-center gap-1"
            data-model-size={formatBytes(model.size_bytes)}
          >
            <HardDrive className="h-3 w-3" />
            {formatBytes(model.size_bytes)}
          </span>
          <span className="flex items-center gap-1">
            <Cpu className="h-3 w-3" />
            {formatBytes(model.required_memory_bytes)} VRAM
          </span>
          {model.hf_info?.downloads && (
            <span className="flex items-center gap-1">
              <Download className="h-3 w-3" />
              {formatDownloads(model.hf_info.downloads)}
            </span>
          )}
          {model.hf_info?.likes && (
            <span className="flex items-center gap-1">
              <ThumbsUp className="h-3 w-3" />
              {model.hf_info.likes}
            </span>
          )}
        </div>

        {/* Tags */}
        {model.tags.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-1">
            {model.tags.map((tag) => (
              <Badge key={tag} variant="outline" className="text-xs">
                {tag}
              </Badge>
            ))}
          </div>
        )}

        {/* Actions */}
        <div className="mt-4">
          {canRegister && (
            <Button
              size="sm"
              onClick={() => onRegister(model)}
              disabled={isRegistering}
              className="w-full"
            >
              {isRegistering ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <Download className="mr-2 h-4 w-4" />
              )}
              Register
            </Button>
          )}
          {model.status === 'downloaded' && (
            <div className="flex items-center justify-center gap-2 text-sm text-muted-foreground">
              <Check className="h-4 w-4 text-green-500" />
              Ready to use
            </div>
          )}
          {isRegistered && model.status !== 'downloaded' && (
            <div className="flex items-center justify-center gap-2 text-sm text-muted-foreground">
              <Check className="h-4 w-4 text-green-500" />
              Registered
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  )
}

export function ModelHubTab() {
  const queryClient = useQueryClient()
  const [search, setSearch] = useState('')
  const [registeringId, setRegisteringId] = useState<string | null>(null)

  const { data: models, isLoading } = useQuery({
    queryKey: ['models-hub'],
    queryFn: modelsApi.getHub,
    refetchInterval: 5000,
  })

  const registerMutation = useMutation({
    mutationFn: (model: ModelWithStatus) => {
      setRegisteringId(model.id)
      return modelsApi.register({
        repo: model.repo,
        filename: model.recommended_filename,
      })
    },
    onSuccess: (_, model) => {
      queryClient.invalidateQueries({ queryKey: ['models-hub'] })
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      toast({ title: `Model ${model.name} registered` })
      setRegisteringId(null)
    },
    onError: (error, model) => {
      toast({
        title: `Failed to register ${model.name}`,
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
      setRegisteringId(null)
    },
  })

  const filteredModels = models?.filter((m) =>
    m.name.toLowerCase().includes(search.toLowerCase()) ||
    m.description.toLowerCase().includes(search.toLowerCase()) ||
    m.tags.some((t) => t.toLowerCase().includes(search.toLowerCase()))
  )

  if (isLoading) {
    return (
      <div className="space-y-4">
        {[...Array(6)].map((_, i) => (
          <div key={i} className="h-40 shimmer rounded" />
        ))}
      </div>
    )
  }

  return (
    <div className="space-y-4">
      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          id="hub-search"
          placeholder="Search models..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="pl-9"
        />
      </div>

      {/* Model Grid */}
      <div id="hub-models-list">
        {!filteredModels || filteredModels.length === 0 ? (
          <div className="flex h-32 flex-col items-center justify-center gap-2 text-muted-foreground">
            <Download className="h-8 w-8" />
            <p>No models found</p>
          </div>
        ) : (
          <div className={cn('grid gap-4 sm:grid-cols-2 lg:grid-cols-3')}>
            {filteredModels.map((model) => (
              <ModelCard
                key={model.id}
                model={model}
                onRegister={(m) => registerMutation.mutate(m)}
                isRegistering={registeringId === model.id}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
