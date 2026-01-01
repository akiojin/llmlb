import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  modelsApi,
  type RegisteredModelView,
  type LifecycleStatus,
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
import { Box, Search, Plus, Trash2, Loader2, Store } from 'lucide-react'
import { ModelHubTab } from './ModelHubTab'

function formatGb(value?: number | null): string {
  if (value == null || Number.isNaN(value)) return '—'
  return `${value.toFixed(1)} GB`
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

export function ModelsSection() {
  const queryClient = useQueryClient()
  const [search, setSearch] = useState('')
  const [registerOpen, setRegisterOpen] = useState(false)
  const [registerRepo, setRegisterRepo] = useState('')
  const [registerFilename, setRegisterFilename] = useState('')
  const [registerDisplayName, setRegisterDisplayName] = useState('')

  function resetRegisterForm() {
    setRegisterRepo('')
    setRegisterFilename('')
    setRegisterDisplayName('')
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
                    Register from the Model Hub or add a Hugging Face repo
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

    </>
  )
}
