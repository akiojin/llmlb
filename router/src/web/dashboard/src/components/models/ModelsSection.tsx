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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Box, Search, Plus, Trash2, Loader2, Download, Store } from 'lucide-react'
import { cn } from '@/lib/utils'
import { ModelHubTab } from './ModelHubTab'

type RegisterFormat = 'safetensors' | 'gguf'
type RegisterGgufPolicy = 'quality' | 'memory' | 'speed'

function formatGb(value?: number | null): string {
  if (value == null || Number.isNaN(value)) return '—'
  return `${value.toFixed(1)} GB`
}

function formatSizeGb(valueBytes?: number | null): string {
  if (valueBytes == null || Number.isNaN(valueBytes)) return '—'
  return `${(valueBytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
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
  const [registerFormat, setRegisterFormat] = useState<RegisterFormat | ''>('')
  const [registerFilename, setRegisterFilename] = useState('')
  const [registerGgufPolicy, setRegisterGgufPolicy] = useState<RegisterGgufPolicy | ''>('')
  const [discoverOpen, setDiscoverOpen] = useState(false)
  const [discoverBaseModel, setDiscoverBaseModel] = useState('')
  const [discoverResults, setDiscoverResults] = useState<GgufDiscoveryResult[]>([])

  function resetRegisterForm() {
    setRegisterRepo('')
    setRegisterFormat('')
    setRegisterFilename('')
    setRegisterGgufPolicy('')
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
      format: RegisterFormat
      filename?: string
      gguf_policy?: RegisterGgufPolicy
    }) => modelsApi.register(params),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      toast({ title: 'Model registration queued' })
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

  const filteredRegistered = (registeredModels as RegisteredModelView[] | undefined)?.filter((m) =>
    m.name.toLowerCase().includes(search.toLowerCase())
  )

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
            <TabsList>
              <TabsTrigger value="local" className="gap-2">
                <Box className="h-4 w-4" />
                Local
                {registeredModels && (
                  <Badge variant="secondary" className="ml-1">
                    {(registeredModels as RegisteredModelView[]).length}
                  </Badge>
                )}
              </TabsTrigger>
              <TabsTrigger value="hub" className="gap-2">
                <Store className="h-4 w-4" />
                Model Hub
              </TabsTrigger>
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
              ) : !filteredRegistered || filteredRegistered.length === 0 ? (
                <div className="flex h-32 flex-col items-center justify-center gap-2 text-muted-foreground">
                  <Box className="h-8 w-8" />
                  <p>No local models</p>
                  <p className="text-sm">
                    Pull from the Model Hub or register a Hugging Face repo
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

                        {/* Download Progress */}
                        {model.download_progress && (model.lifecycle_status === 'caching' || model.lifecycle_status === 'pending') && (
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
                            disabled={deleteMutation.isPending || model.lifecycle_status === 'caching'}
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
        <DialogContent id="convert-modal">
          <DialogHeader>
            <DialogTitle>Register Model</DialogTitle>
            <DialogDescription>
              Register a model from Hugging Face. Choose the artifact format to cache and run. If a repository
              contains both <code>safetensors</code> and <code>.gguf</code>, you must explicitly pick one.
              Browse models at{' '}
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
              <Label htmlFor="convert-format">Format</Label>
              <Select
                value={registerFormat || 'none'}
                onValueChange={(value) =>
                  setRegisterFormat(value === 'none' ? '' : (value as RegisterFormat))
                }
              >
                <SelectTrigger id="convert-format">
                  <SelectValue placeholder="Select format" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">Select...</SelectItem>
                  <SelectItem value="safetensors">safetensors (native engine: TBD)</SelectItem>
                  <SelectItem value="gguf">GGUF (llama.cpp fallback)</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="convert-repo">Repo</Label>
              <Input
                id="convert-repo"
                placeholder="nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16"
                value={registerRepo}
                onChange={(e) => setRegisterRepo(e.target.value)}
              />
            </div>
            {registerFormat === 'gguf' ? (
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
            ) : null}
            {registerFormat === 'gguf' ? (
              <>
                <div className="space-y-2">
                  <Label htmlFor="convert-filename">GGUF filename (optional)</Label>
                  <Input
                    id="convert-filename"
                    placeholder="model.Q4_K_M.gguf"
                    value={registerFilename}
                    onChange={(e) => setRegisterFilename(e.target.value)}
                  />
                  <p className="text-xs text-muted-foreground">
                    Specify an exact <code>.gguf</code> filename if you already know which variant you want.
                  </p>
                </div>
                {!registerFilename.trim() ? (
                  <div className="space-y-2">
                    <Label htmlFor="convert-gguf-policy">GGUF selection policy</Label>
                    <Select
                      value={registerGgufPolicy || 'none'}
                      onValueChange={(value) =>
                        setRegisterGgufPolicy(value === 'none' ? '' : (value as RegisterGgufPolicy))
                      }
                    >
                      <SelectTrigger id="convert-gguf-policy">
                        <SelectValue placeholder="Select policy" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="none">Select...</SelectItem>
                        <SelectItem value="quality">Quality (higher quality)</SelectItem>
                        <SelectItem value="memory">Memory (lower VRAM/RAM)</SelectItem>
                        <SelectItem value="speed">Speed (smaller, practical)</SelectItem>
                      </SelectContent>
                    </Select>
                    <p className="text-xs text-muted-foreground">
                      When no filename is specified, the router selects from GGUF siblings:
                      <br />
                      <b>Quality</b>: prefers F32/BF16/F16/Q8/Q6... (best quality)
                      <br />
                      <b>Memory</b>: chooses the smallest GGUF file (lowest memory)
                      <br />
                      <b>Speed</b>: chooses a smaller, practical GGUF among common quantizations
                    </p>
                  </div>
                ) : null}
              </>
            ) : registerFormat === 'safetensors' ? (
              <p className="text-xs text-muted-foreground">
                <b>Note</b>: text generation via safetensors is <b>TBD</b> (engine implementation will be decided later).
                For now, use <b>GGUF (llama.cpp)</b> if you need to run the model.
                <br />
                <br />
                <b>safetensors</b> requires <code>config.json</code> and <code>tokenizer.json</code> in the HF snapshot.
                If weights are sharded, an <code>.index.json</code> must be present.
              </p>
            ) : null}
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
                  format: registerFormat as RegisterFormat,
                  filename:
                    registerFormat === 'gguf' && registerFilename.trim()
                      ? registerFilename.trim()
                      : undefined,
                  gguf_policy:
                    registerFormat === 'gguf' && !registerFilename.trim() && registerGgufPolicy
                      ? (registerGgufPolicy as RegisterGgufPolicy)
                      : undefined,
                })
              }
              disabled={
                !registerRepo.trim() ||
                !registerFormat ||
                (registerFormat === 'gguf' && !registerFilename.trim() && !registerGgufPolicy) ||
                registerMutation.isPending
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
        <DialogContent
          id="discover-gguf-modal"
          className="max-w-3xl max-h-[80vh] overflow-hidden"
        >
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
                            {file.quantization ? <span>{file.quantization} • </span> : null}
                            {formatSizeGb(file.size_bytes)}
                          </div>
                        </div>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => {
                            setRegisterRepo(alt.repo)
                            setRegisterFilename(file.filename)
                            setRegisterGgufPolicy('')
                            setRegisterFormat('gguf')
                            setDiscoverOpen(false)
                            toast({
                              title: 'Selected GGUF file',
                              description: `${alt.repo} • ${file.filename}`,
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
