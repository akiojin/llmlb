import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  modelsApi,
  type RegisteredModelView,
  type LifecycleStatus,
  type GgufDiscoveryResult,
} from '@/lib/api'
import { toast } from '@/hooks/use-toast'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Label } from '@/components/ui/label'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Box, Search, Plus, Trash2, Loader2, Store, Cloud } from 'lucide-react'
import { ModelHubTab } from './ModelHubTab'

function formatGb(value?: number | null): string {
  if (value == null || Number.isNaN(value)) return '—'
  return `${value.toFixed(1)} GB`
}

function formatSizeGb(valueBytes?: number | null): string {
  if (valueBytes == null || Number.isNaN(valueBytes)) return '—'
  return `${(valueBytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}


function lifecycleStatusBadge(status: LifecycleStatus, ready: boolean) {
  if (ready) {
    return <Badge variant="online">Ready</Badge>
  }
  switch (status) {
    case 'registered':
      return <Badge variant="online">Registered</Badge>
    case 'pending':
      return <Badge variant="outline">Pending</Badge>
    case 'error':
      return <Badge variant="destructive">Error</Badge>
    default:
      return <Badge variant="outline">{status}</Badge>
  }
}

// Cloud provider display names and colors
const CLOUD_PROVIDERS = {
  openai: { name: 'OpenAI', color: 'text-green-600' },
  anthropic: { name: 'Anthropic', color: 'text-orange-600' },
  google: { name: 'Google', color: 'text-blue-600' },
} as const

type CloudProvider = keyof typeof CLOUD_PROVIDERS

export function ModelsSection() {
  const queryClient = useQueryClient()
  const [search, setSearch] = useState('')
  const [registerOpen, setRegisterOpen] = useState(false)
  const [registerRepo, setRegisterRepo] = useState('')
  const [registerFilename, setRegisterFilename] = useState('')
  const [registerDisplayName, setRegisterDisplayName] = useState('')
  const [discoverOpen, setDiscoverOpen] = useState(false)
  const [discoverBaseModel, setDiscoverBaseModel] = useState('')
  const [discoverResults, setDiscoverResults] = useState<GgufDiscoveryResult[]>([])

  function resetRegisterForm() {
    setRegisterRepo('')
    setRegisterFilename('')
    setRegisterDisplayName('')
    setDiscoverOpen(false)
    setDiscoverBaseModel('')
    setDiscoverResults([])
  }

  const { data: registeredModels, isLoading: isLoadingRegistered } = useQuery({
    queryKey: ['registered-models'],
    queryFn: modelsApi.getRegistered,
    refetchInterval: 10000,
  })

  const registerMutation = useMutation({
    mutationFn: (params: {
      repo: string
      filename?: string
      display_name?: string
    }) => modelsApi.register(params),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      toast({ title: 'Model registered' })
      setRegisterOpen(false)
      resetRegisterForm()
    },
    onError: (error) => {
      toast({
        title: 'Failed to register model',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  const discoverMutation = useMutation({
    mutationFn: (model: string) => modelsApi.discoverGguf(model),
    onSuccess: (data) => {
      setDiscoverBaseModel(data.base_model)
      setDiscoverResults(data.gguf_alternatives)
      setDiscoverOpen(true)
    },
    onError: (error) => {
      toast({
        title: 'Failed to discover GGUF alternatives',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (modelName: string) => modelsApi.delete(modelName),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      queryClient.invalidateQueries({ queryKey: ['models-hub'] })
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

  // Filter models by owned_by
  const allModels = registeredModels as RegisteredModelView[] | undefined
  const localModels = allModels?.filter((m) =>
    m.owned_by === 'router' || !m.owned_by
  )
  const openaiModels = allModels?.filter((m) => m.owned_by === 'openai')
  const anthropicModels = allModels?.filter((m) => m.owned_by === 'anthropic')
  const googleModels = allModels?.filter((m) => m.owned_by === 'google')

  // Apply search filter to local models
  const filteredLocalModels = localModels?.filter((m) =>
    m.name.toLowerCase().includes(search.toLowerCase())
  )

  // Helper to render cloud model cards (no delete button)
  const renderCloudModelCard = (model: RegisteredModelView) => (
    <Card key={model.name} className="overflow-hidden">
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0 space-y-1">
            <h4 className="truncate font-medium">{model.name}</h4>
            <p className="text-xs text-muted-foreground">
              {model.owned_by || 'cloud'}
            </p>
          </div>
          <Badge variant="online">Ready</Badge>
        </div>
        {model.description && (
          <p className="mt-2 text-sm text-muted-foreground line-clamp-2">
            {model.description}
          </p>
        )}
      </CardContent>
    </Card>
  )

  // Helper to render cloud tab content
  const renderCloudTabContent = (
    models: RegisteredModelView[] | undefined,
    provider: CloudProvider
  ) => {
    if (isLoadingRegistered) {
      return (
        <div className="space-y-4">
          {[...Array(3)].map((_, i) => (
            <div key={i} className="h-24 shimmer rounded" />
          ))}
        </div>
      )
    }
    if (!models || models.length === 0) {
      return (
        <div className="flex h-32 flex-col items-center justify-center gap-2 text-muted-foreground">
          <Cloud className="h-8 w-8" />
          <p>No {CLOUD_PROVIDERS[provider].name} models available</p>
          <p className="text-sm">
            Configure {CLOUD_PROVIDERS[provider].name} API key to access models
          </p>
        </div>
      )
    }
    return (
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {models.map(renderCloudModelCard)}
      </div>
    )
  }

  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Box className="h-5 w-5" />
            Models
          </CardTitle>
        </CardHeader>
        <CardContent>
          <Tabs defaultValue="local" className="space-y-4">
            <TabsList className="flex-wrap">
              <TabsTrigger value="local" className="gap-2">
                <Box className="h-4 w-4" />
                Local
                {localModels && (
                  <Badge variant="secondary" className="ml-1">
                    {localModels.length}
                  </Badge>
                )}
              </TabsTrigger>
              <TabsTrigger value="hub" className="gap-2">
                <Store className="h-4 w-4" />
                Model Hub
              </TabsTrigger>
              {openaiModels && openaiModels.length > 0 && (
                <TabsTrigger value="openai" className="gap-2">
                  <Cloud className="h-4 w-4 text-green-600" />
                  OpenAI
                  <Badge variant="secondary" className="ml-1">
                    {openaiModels.length}
                  </Badge>
                </TabsTrigger>
              )}
              {anthropicModels && anthropicModels.length > 0 && (
                <TabsTrigger value="anthropic" className="gap-2">
                  <Cloud className="h-4 w-4 text-orange-600" />
                  Anthropic
                  <Badge variant="secondary" className="ml-1">
                    {anthropicModels.length}
                  </Badge>
                </TabsTrigger>
              )}
              {googleModels && googleModels.length > 0 && (
                <TabsTrigger value="google" className="gap-2">
                  <Cloud className="h-4 w-4 text-blue-600" />
                  Google
                  <Badge variant="secondary" className="ml-1">
                    {googleModels.length}
                  </Badge>
                </TabsTrigger>
              )}
            </TabsList>

            {/* Local Models Tab */}
            <TabsContent value="local" className="space-y-4">
              <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div className="relative flex-1">
                  <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    placeholder="Search local models..."
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    className="pl-9"
                  />
                </div>
                <Button
                  id="register-model"
                  variant="outline"
                  onClick={() => setRegisterOpen(true)}
                >
                  <Plus className="mr-2 h-4 w-4" />
                  Register
                </Button>
              </div>

              {isLoadingRegistered ? (
                <div className="space-y-4">
                  {[...Array(3)].map((_, i) => (
                    <div key={i} className="h-24 shimmer rounded" />
                  ))}
                </div>
              ) : !filteredLocalModels || filteredLocalModels.length === 0 ? (
                <div className="flex h-32 flex-col items-center justify-center gap-2 text-muted-foreground">
                  <Box className="h-8 w-8" />
                  <p>No local models</p>
                  <p className="text-sm">
                    Register from the Model Hub or add a Hugging Face repo
                  </p>
                </div>
              ) : (
                <div id="local-models-list" className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                  {filteredLocalModels.map((model) => (
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
                          {lifecycleStatusBadge(model.lifecycle_status, model.ready)}
                        </div>

                        <div className="mt-3 space-y-1 text-sm">
                          <p className="text-muted-foreground">
                            Size: {formatGb(model.size_gb)}
                          </p>
                          <p className="text-muted-foreground">
                            Required VRAM: {formatGb(model.required_memory_gb)}
                          </p>
                        </div>

                        <div className="mt-4 flex gap-2">
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => deleteMutation.mutate(model.name)}
                            disabled={deleteMutation.isPending}
                          >
                            <Trash2 className="h-4 w-4 text-destructive" />
                          </Button>
                        </div>
                      </CardContent>
                    </Card>
                  ))}
                </div>
              )}
            </TabsContent>

            {/* Model Hub Tab */}
            <TabsContent value="hub">
              <ModelHubTab />
            </TabsContent>

            {/* OpenAI Tab */}
            <TabsContent value="openai" className="space-y-4">
              {renderCloudTabContent(openaiModels, 'openai')}
            </TabsContent>

            {/* Anthropic Tab */}
            <TabsContent value="anthropic" className="space-y-4">
              {renderCloudTabContent(anthropicModels, 'anthropic')}
            </TabsContent>

            {/* Google Tab */}
            <TabsContent value="google" className="space-y-4">
              {renderCloudTabContent(googleModels, 'google')}
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>

      <Dialog
        open={registerOpen}
        onOpenChange={(open) => {
          setRegisterOpen(open)
          if (!open) resetRegisterForm()
        }}
      >
        <DialogContent id="register-modal">
          <DialogHeader>
            <DialogTitle>Register Model</DialogTitle>
            <DialogDescription>
              Register a model from Hugging Face. If a repository contains multiple artifacts, specify the
              exact filename to use. Nodes download artifacts directly. Browse models at{' '}
              <a
                href="https://huggingface.co/models"
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
              <Label htmlFor="register-repo">Repo</Label>
              <Input
                id="register-repo"
                placeholder="nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16"
                value={registerRepo}
                onChange={(e) => setRegisterRepo(e.target.value)}
              />
            </div>
            <div className="flex items-center justify-between gap-3 rounded-md border bg-muted/30 px-3 py-2">
              <p className="text-xs text-muted-foreground">
                If the repo you entered is a base model repo (safetensors), you can discover GGUF alternatives.
              </p>
              <Button
                id="discover-gguf"
                variant="outline"
                size="sm"
                disabled={!registerRepo.trim() || discoverMutation.isPending}
                onClick={() => discoverMutation.mutate(registerRepo.trim())}
              >
                {discoverMutation.isPending ? (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                ) : null}
                Discover
              </Button>
            </div>
            <div className="space-y-2">
              <Label htmlFor="register-filename">Filename (optional)</Label>
              <Input
                id="register-filename"
                placeholder="model.safetensors or model.Q4_K_M.gguf"
                value={registerFilename}
                onChange={(e) => setRegisterFilename(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                If the repo has multiple artifacts, specify the exact filename you want the node to download.
              </p>
            </div>
            <div className="space-y-2">
              <Label htmlFor="register-display-name">Display name (optional)</Label>
              <Input
                id="register-display-name"
                placeholder="Optional display name"
                value={registerDisplayName}
                onChange={(e) => setRegisterDisplayName(e.target.value)}
              />
            </div>
            <p className="text-xs text-muted-foreground">
              The router stores metadata only. Nodes download model artifacts directly from Hugging Face.
            </p>
          </div>

          <DialogFooter>
            <Button id="register-modal-close" variant="outline" onClick={() => setRegisterOpen(false)}>
              Cancel
            </Button>
            <Button
              id="register-submit"
              onClick={() =>
                registerMutation.mutate({
                  repo: registerRepo.trim(),
                  filename: registerFilename.trim() ? registerFilename.trim() : undefined,
                  display_name: registerDisplayName.trim() ? registerDisplayName.trim() : undefined,
                })
              }
              disabled={
                !registerRepo.trim() || registerMutation.isPending
              }
            >
              {registerMutation.isPending ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : null}
              Register
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Dialog open={discoverOpen} onOpenChange={setDiscoverOpen}>
        <DialogContent id="discover-gguf-modal" className="max-w-3xl max-h-[80vh] overflow-hidden">
          <DialogHeader>
            <DialogTitle>GGUF alternatives</DialogTitle>
            <DialogDescription>
              Discover GGUF repos for <code>{discoverBaseModel || registerRepo || 'model'}</code> and pick an exact file.
            </DialogDescription>
          </DialogHeader>

          <div className="max-h-[60vh] space-y-3 overflow-y-auto pr-1">
            {discoverResults.length === 0 ? (
              <div className="rounded-md border bg-muted/30 p-4 text-sm text-muted-foreground">
                No GGUF alternatives found. Try a different search term or check Hugging Face manually.
              </div>
            ) : (
              discoverResults.map((alt) => (
                <Card key={alt.repo} className="border-border/50">
                  <CardHeader className="pb-3">
                    <CardTitle className="text-base">
                      {alt.repo}{' '}
                      {alt.trusted ? (
                        <Badge variant="secondary" className="ml-2">
                          trusted
                        </Badge>
                      ) : null}
                    </CardTitle>
                    <p className="text-xs text-muted-foreground">Provider: {alt.provider}</p>
                  </CardHeader>
                  <CardContent className="space-y-2">
                    {alt.files.map((file) => (
                      <div
                        key={`${alt.repo}/${file.filename}`}
                        className="flex flex-col gap-2 rounded-md border bg-background px-3 py-2 sm:flex-row sm:items-center sm:justify-between"
                      >
                        <div className="min-w-0">
                          <div className="truncate text-sm font-medium">{file.filename}</div>
                          <div className="mt-0.5 text-xs text-muted-foreground">
                            {file.quantization ? <span>{file.quantization} - </span> : null}
                            {formatSizeGb(file.size_bytes)}
                          </div>
                        </div>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => {
                            setRegisterRepo(alt.repo)
                            setRegisterFilename(file.filename)
                            setDiscoverOpen(false)
                            toast({
                              title: 'Selected GGUF file',
                              description: `${alt.repo} - ${file.filename}`,
                            })
                          }}
                        >
                          Use
                        </Button>
                      </div>
                    ))}
                  </CardContent>
                </Card>
              ))
            )}
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setDiscoverOpen(false)}>
              Close
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

    </>
  )
}
