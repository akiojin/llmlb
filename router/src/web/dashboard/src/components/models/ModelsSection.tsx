import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { modelsApi, type RegisteredModelView, type LifecycleStatus } from '@/lib/api'
import { toast } from '@/hooks/use-toast'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Box, Search, Plus, Trash2, Loader2, Download } from 'lucide-react'
import { cn } from '@/lib/utils'

function formatGb(value?: number | null): string {
  if (value == null || Number.isNaN(value)) return '—'
  return `${value.toFixed(1)} GB`
}

function lifecycleStatusBadge(status: LifecycleStatus) {
  switch (status) {
    case 'registered':
      return <Badge variant="online">Registered</Badge>
    case 'caching':
      return <Badge variant="secondary"><Download className="mr-1 h-3 w-3 animate-pulse" />Caching</Badge>
    case 'pending':
      return <Badge variant="outline">Pending</Badge>
    case 'error':
      return <Badge variant="destructive">Error</Badge>
    default:
      return <Badge variant="outline">{status}</Badge>
  }
}

export function ModelsSection() {
  const queryClient = useQueryClient()
  const [search, setSearch] = useState('')
  const [registerOpen, setRegisterOpen] = useState(false)
  const [registerRepo, setRegisterRepo] = useState('')
  const [registerFilename, setRegisterFilename] = useState('')

  const { data: registeredModels, isLoading: isLoadingRegistered } = useQuery({
    queryKey: ['registered-models'],
    queryFn: modelsApi.getRegistered,
    refetchInterval: 10000,
  })

  const registerMutation = useMutation({
    mutationFn: (params: { repo: string; filename?: string }) =>
      modelsApi.register(params.repo, params.filename),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      toast({ title: 'Model registration queued' })
      setRegisterOpen(false)
      setRegisterRepo('')
      setRegisterFilename('')
    },
    onError: (error) => {
      toast({
        title: 'Failed to register model',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (modelName: string) => modelsApi.delete(modelName),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      toast({ title: 'Model deleted' })
    },
    onError: (error) => {
      toast({
        title: 'Failed to delete model',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  const filteredRegistered = (registeredModels as RegisteredModelView[] | undefined)?.filter((m) =>
    m.name.toLowerCase().includes(search.toLowerCase())
  )

  return (
    <>
      <Card>
        <CardHeader>
          <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <CardTitle className="flex items-center gap-2">
              <Box className="h-5 w-5" />
              Registered Models
              {registeredModels && (
                <Badge variant="secondary" className="ml-1">
                  {(registeredModels as RegisteredModelView[]).length}
                </Badge>
              )}
            </CardTitle>
            <div className="flex gap-2">
              <div className="relative">
                <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  placeholder="Search models..."
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  className="pl-9 w-64"
                />
              </div>
              <Button onClick={() => setRegisterOpen(true)}>
                <Plus className="mr-2 h-4 w-4" />
                Register
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {isLoadingRegistered ? (
            <div className="space-y-4">
              {[...Array(3)].map((_, i) => (
                <div key={i} className="h-24 shimmer rounded" />
              ))}
            </div>
          ) : !filteredRegistered || filteredRegistered.length === 0 ? (
            <div className="flex h-32 flex-col items-center justify-center gap-2 text-muted-foreground">
              <Box className="h-8 w-8" />
              <p>No registered models</p>
              <p className="text-sm">
                Register a model from{' '}
                <a
                  href="https://huggingface.co/models?library=gguf"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-primary underline"
                >
                  Hugging Face
                </a>
              </p>
            </div>
          ) : (
            <div id="local-models-list" className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {filteredRegistered.map((model) => (
                <Card key={model.name} className="overflow-hidden">
                  <CardContent className="p-4">
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0 space-y-1">
                        <h4 className="truncate font-medium">{model.name}</h4>
                        <p className="text-xs text-muted-foreground">
                          {model.source || 'local'}
                          {model.repo ? ` • ${model.repo}` : ''}
                        </p>
                      </div>
                      {lifecycleStatusBadge(model.lifecycle_status)}
                    </div>

                    {/* ダウンロード進行状況 */}
                    {model.download_progress && (model.lifecycle_status === 'downloading' || model.lifecycle_status === 'pending') && (
                      <div className="mt-3">
                        <div className="h-1.5 w-full rounded-full bg-muted">
                          <div
                            className={cn(
                              'h-full rounded-full transition-all',
                              model.lifecycle_status === 'error' ? 'bg-destructive' : 'bg-primary'
                            )}
                            style={{ width: `${Math.round(model.download_progress.percent * 100)}%` }}
                          />
                        </div>
                        <p className="mt-1 text-xs text-muted-foreground">
                          {Math.round(model.download_progress.percent * 100)}%
                          {model.download_progress.error && (
                            <span className="text-destructive"> • {model.download_progress.error}</span>
                          )}
                        </p>
                      </div>
                    )}

                    <div className="mt-3 space-y-1 text-sm">
                      <p className="text-muted-foreground">
                        Size: {formatGb(model.size_gb)}
                      </p>
                      <p className="text-muted-foreground">
                        Required VRAM: {formatGb(model.required_memory_gb)}
                      </p>
                      {model.path && (
                        <p className="truncate text-xs text-muted-foreground">
                          {model.path}
                        </p>
                      )}
                    </div>

                    <div className="mt-4 flex gap-2">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => deleteMutation.mutate(model.name)}
                        disabled={deleteMutation.isPending || model.lifecycle_status === 'downloading'}
                      >
                        <Trash2 className="h-4 w-4 text-destructive" />
                      </Button>
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Dialog open={registerOpen} onOpenChange={setRegisterOpen}>
        <DialogContent id="convert-modal">
          <DialogHeader>
            <DialogTitle>Register Model</DialogTitle>
            <DialogDescription>
              Register a model from Hugging Face. Enter the repository ID and optionally a specific GGUF filename.
              Browse models at{' '}
              <a
                href="https://huggingface.co/models?library=gguf"
                target="_blank"
                rel="noopener noreferrer"
                className="text-primary underline"
              >
                huggingface.co
              </a>
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="convert-repo">Repo</Label>
              <Input
                id="convert-repo"
                placeholder="bartowski/Qwen2.5-7B-Instruct-GGUF"
                value={registerRepo}
                onChange={(e) => setRegisterRepo(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="convert-filename">Filename (optional)</Label>
              <Input
                id="convert-filename"
                placeholder="Qwen2.5-7B-Instruct-Q4_K_M.gguf"
                value={registerFilename}
                onChange={(e) => setRegisterFilename(e.target.value)}
              />
            </div>
          </div>

          <DialogFooter>
            <Button id="convert-modal-close" variant="outline" onClick={() => setRegisterOpen(false)}>
              Cancel
            </Button>
            <Button
              id="convert-submit"
              onClick={() =>
                registerMutation.mutate({
                  repo: registerRepo.trim(),
                  filename: registerFilename.trim() ? registerFilename.trim() : undefined,
                })
              }
              disabled={!registerRepo.trim() || registerMutation.isPending}
            >
              {registerMutation.isPending ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : null}
              Register
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
