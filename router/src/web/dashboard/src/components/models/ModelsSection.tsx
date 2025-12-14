import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  modelsApi,
  type AvailableModelView,
  type ConvertTask,
  type RegisteredModelView,
} from '@/lib/api'
import { cn } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Box, Search, Plus, Download, Trash2, RefreshCw, ExternalLink, Loader2 } from 'lucide-react'

function formatGb(value?: number): string {
  if (value === undefined || Number.isNaN(value)) return '—'
  return `${value.toFixed(1)} GB`
}

function taskStatusBadge(status: ConvertTask['status']) {
  switch (status) {
    case 'completed':
      return <Badge variant="online">Completed</Badge>
    case 'failed':
      return <Badge variant="destructive">Failed</Badge>
    case 'in_progress':
      return <Badge variant="secondary">In progress</Badge>
    case 'queued':
      return <Badge variant="outline">Queued</Badge>
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

  const { data: availableResponse, isLoading: isLoadingAvailable } = useQuery({
    queryKey: ['available-models'],
    queryFn: modelsApi.getAvailable,
    refetchInterval: 60000,
  })

  const { data: convertTasks, isLoading: isLoadingConvertTasks } = useQuery({
    queryKey: ['convert-tasks'],
    queryFn: modelsApi.getConvertTasks,
    refetchInterval: 5000,
  })

  const registerMutation = useMutation({
    mutationFn: (params: { repo: string; filename?: string }) =>
      modelsApi.register(params.repo, params.filename),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['convert-tasks'] })
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

  const pullMutation = useMutation({
    mutationFn: (params: { repo: string; filename: string }) =>
      modelsApi.pull(params.repo, params.filename),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      toast({ title: 'Model pulled and cached' })
    },
    onError: (error) => {
      toast({
        title: 'Failed to pull model',
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

  const deleteConvertTaskMutation = useMutation({
    mutationFn: (taskId: string) => modelsApi.deleteConvertTask(taskId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['convert-tasks'] })
      toast({ title: 'Task deleted' })
    },
    onError: (error) => {
      toast({
        title: 'Failed to delete task',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  const availableModels = availableResponse?.models || []

  const filteredRegistered = (registeredModels as RegisteredModelView[] | undefined)?.filter((m) =>
    m.name.toLowerCase().includes(search.toLowerCase())
  )

  const filteredAvailable = (availableModels as AvailableModelView[]).filter((m) =>
    `${m.name} ${m.repo ?? ''} ${m.filename ?? ''}`.toLowerCase().includes(search.toLowerCase())
  )

  return (
    <>
      <Tabs defaultValue="registered" className="space-y-4">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <TabsList>
            <TabsTrigger value="registered" className="gap-2">
              <Box className="h-4 w-4" />
              Registered
              {registeredModels && (
                <Badge variant="secondary" className="ml-1">
                  {(registeredModels as RegisteredModelView[]).length}
                </Badge>
              )}
            </TabsTrigger>
            <TabsTrigger value="available" className="gap-2">
              <Download className="h-4 w-4" />
              Available
            </TabsTrigger>
            <TabsTrigger value="tasks" className="gap-2">
              <RefreshCw className="h-4 w-4" />
              Convert Tasks
              {convertTasks && (
                <Badge variant="secondary" className="ml-1">
                  {(convertTasks as ConvertTask[]).length}
                </Badge>
              )}
            </TabsTrigger>
          </TabsList>

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

        <TabsContent value="registered">
          <Card>
            <CardHeader>
              <CardTitle>Registered Models</CardTitle>
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
                </div>
              ) : (
                <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                  {filteredRegistered.map((model) => (
                    <Card key={model.name} className="overflow-hidden">
                      <CardContent className="p-4">
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0 space-y-1">
                            <h4 className="truncate font-medium">{model.name}</h4>
                            <p className="text-xs text-muted-foreground">
                              {model.source || 'local'}
                              {model.status ? ` • ${model.status}` : ''}
                            </p>
                          </div>
                          {model.ready ? (
                            <Badge variant="online">Ready</Badge>
                          ) : (
                            <Badge variant="outline">Not cached</Badge>
                          )}
                        </div>

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
                            variant="outline"
                            size="sm"
                            disabled={!model.repo || !model.filename || pullMutation.isPending}
                            onClick={() => {
                              if (!model.repo || !model.filename) return
                              pullMutation.mutate({ repo: model.repo, filename: model.filename })
                            }}
                          >
                            <Download className="mr-1 h-3 w-3" />
                            Pull
                          </Button>

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
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="available">
          <Card>
            <CardHeader>
              <CardTitle>Available Models (Hugging Face)</CardTitle>
            </CardHeader>
            <CardContent>
              {isLoadingAvailable ? (
                <div className="space-y-4">
                  {[...Array(5)].map((_, i) => (
                    <div key={i} className="h-16 shimmer rounded" />
                  ))}
                </div>
              ) : filteredAvailable.length === 0 ? (
                <div className="flex h-32 flex-col items-center justify-center gap-2 text-muted-foreground">
                  <Download className="h-8 w-8" />
                  <p>No available models found</p>
                </div>
              ) : (
                <ScrollArea className="h-96">
                  <div className="space-y-2">
                    {filteredAvailable.map((model) => (
                      <div
                        key={`${model.repo ?? ''}/${model.filename ?? ''}/${model.name}`}
                        className="flex items-center justify-between gap-3 rounded-lg border p-3 hover:bg-muted/50"
                      >
                        <div className="min-w-0 space-y-1">
                          <div className="flex items-center gap-2">
                            <h4 className="truncate font-medium">{model.name}</h4>
                            {model.quantization && (
                              <Badge variant="secondary" className="shrink-0">
                                {model.quantization}
                              </Badge>
                            )}
                          </div>
                          <p className="truncate text-xs text-muted-foreground">
                            {model.repo}
                            {model.filename ? ` • ${model.filename}` : ''}
                          </p>
                          <p className="text-xs text-muted-foreground">
                            Size: {formatGb(model.size_gb)} • Required VRAM: {formatGb(model.required_memory_gb)}
                          </p>
                        </div>

                        <div className="flex shrink-0 gap-2">
                          {model.download_url && (
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => window.open(model.download_url, '_blank')}
                            >
                              <ExternalLink className="h-4 w-4" />
                            </Button>
                          )}

                          <Button
                            size="sm"
                            disabled={!model.repo || registerMutation.isPending}
                            onClick={() => {
                              if (!model.repo) return
                              registerMutation.mutate({
                                repo: model.repo,
                                filename: model.filename,
                              })
                            }}
                          >
                            {registerMutation.isPending ? (
                              <Loader2 className="mr-1 h-4 w-4 animate-spin" />
                            ) : null}
                            Register
                          </Button>
                        </div>
                      </div>
                    ))}
                  </div>
                </ScrollArea>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="tasks">
          <Card>
            <CardHeader>
              <CardTitle>Convert Tasks</CardTitle>
            </CardHeader>
            <CardContent>
              {isLoadingConvertTasks ? (
                <div className="space-y-4">
                  {[...Array(3)].map((_, i) => (
                    <div key={i} className="h-20 shimmer rounded" />
                  ))}
                </div>
              ) : !convertTasks || (convertTasks as ConvertTask[]).length === 0 ? (
                <div className="flex h-32 flex-col items-center justify-center gap-2 text-muted-foreground">
                  <RefreshCw className="h-8 w-8" />
                  <p>No tasks</p>
                </div>
              ) : (
                <div className="space-y-3">
                  {(convertTasks as ConvertTask[]).map((task) => {
                    const percent = Math.round((task.progress || 0) * 100)
                    return (
                      <div key={task.id} className="rounded-lg border p-3">
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0">
                            <p className="truncate font-medium">{task.repo}</p>
                            <p className="truncate text-xs text-muted-foreground">
                              {task.filename}
                            </p>
                          </div>
                          <div className="flex items-center gap-2">
                            {taskStatusBadge(task.status)}
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={deleteConvertTaskMutation.isPending}
                              onClick={() => deleteConvertTaskMutation.mutate(task.id)}
                            >
                              <Trash2 className="h-4 w-4 text-destructive" />
                            </Button>
                          </div>
                        </div>

                        <div className="mt-3">
                          <div className="h-1.5 w-full rounded-full bg-muted">
                            <div
                              className={cn(
                                'h-full rounded-full transition-all',
                                task.status === 'failed' ? 'bg-destructive' : 'bg-primary'
                              )}
                              style={{ width: `${percent}%` }}
                            />
                          </div>
                          <div className="mt-1 flex items-center justify-between text-xs text-muted-foreground">
                            <span>{percent}%</span>
                            {task.error ? <span className="truncate">{task.error}</span> : null}
                          </div>
                        </div>
                      </div>
                    )
                  })}
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      <Dialog open={registerOpen} onOpenChange={setRegisterOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Register Model</DialogTitle>
            <DialogDescription>
              Queue a Hugging Face model for download/convert. Model IDs are normalized to a filename-based format (for example, <code>gpt-oss-20b</code>).
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="hf-repo">Repo</Label>
              <Input
                id="hf-repo"
                placeholder="TheBloke/Llama-2-7B-GGUF"
                value={registerRepo}
                onChange={(e) => setRegisterRepo(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="hf-filename">Filename (optional)</Label>
              <Input
                id="hf-filename"
                placeholder="llama-2-7b.Q4_K_M.gguf"
                value={registerFilename}
                onChange={(e) => setRegisterFilename(e.target.value)}
              />
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setRegisterOpen(false)}>
              Cancel
            </Button>
            <Button
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
