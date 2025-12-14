import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { modelsApi, type ModelInfo, type ConvertTask } from '@/lib/api'
import { formatBytes } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import {
  Box,
  Search,
  Plus,
  Trash2,
  CheckCircle2,
  Clock,
  AlertCircle,
  Loader2,
} from 'lucide-react'

export function ModelsSection() {
  const queryClient = useQueryClient()
  const [search, setSearch] = useState('')
  const [registerUrl, setRegisterUrl] = useState('')

  // Fetch registered models
  const { data: registeredModels, isLoading: isLoadingRegistered } = useQuery({
    queryKey: ['registered-models'],
    queryFn: modelsApi.getRegistered,
    refetchInterval: 10000,
  })

  // Fetch convert tasks (for showing converting/queued models)
  const { data: convertTasks } = useQuery({
    queryKey: ['convert-tasks'],
    queryFn: modelsApi.getConvertTasks,
    refetchInterval: 3000,
  })

  // Filter to show only non-completed tasks
  const activeConvertTasks = convertTasks?.filter(
    (t: ConvertTask) => t.status !== 'completed'
  )

  // Register model mutation
  const registerMutation = useMutation({
    mutationFn: (url: string) => modelsApi.register(url),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      queryClient.invalidateQueries({ queryKey: ['convert-tasks'] })
      toast({ title: 'Model registration started' })
      setRegisterUrl('')
    },
    onError: (error) => {
      toast({
        title: 'Failed to register model',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Delete model mutation
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

  // Delete convert task mutation
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

  const filteredRegistered = (registeredModels as ModelInfo[] | undefined)?.filter((m) =>
    m.name.toLowerCase().includes(search.toLowerCase())
  )

  const getStateIcon = (state: string) => {
    switch (state) {
      case 'ready':
        return <CheckCircle2 className="h-4 w-4 text-success" />
      case 'downloading':
      case 'converting':
        return <Loader2 className="h-4 w-4 animate-spin text-primary" />
      case 'pending':
        return <Clock className="h-4 w-4 text-warning" />
      default:
        return <AlertCircle className="h-4 w-4 text-muted-foreground" />
    }
  }

  const getStateBadge = (state: string) => {
    switch (state) {
      case 'ready':
        return <Badge variant="online">Ready</Badge>
      case 'downloading':
        return <Badge variant="secondary">Downloading</Badge>
      case 'converting':
        return <Badge variant="secondary">Converting</Badge>
      case 'pending':
        return <Badge variant="outline">Pending</Badge>
      default:
        return <Badge variant="outline">{state}</Badge>
    }
  }

  const getConvertTaskStatusBadge = (status: ConvertTask['status']) => {
    switch (status) {
      case 'in_progress':
        return <Badge variant="secondary">Converting</Badge>
      case 'queued':
        return <Badge variant="outline">Queued</Badge>
      case 'failed':
        return <Badge variant="destructive">Failed</Badge>
      case 'completed':
        return <Badge variant="online">Completed</Badge>
      default:
        return <Badge variant="outline">{status}</Badge>
    }
  }

  const getConvertTaskIcon = (status: ConvertTask['status']) => {
    switch (status) {
      case 'in_progress':
        return <Loader2 className="h-4 w-4 animate-spin text-primary" />
      case 'queued':
        return <Clock className="h-4 w-4 text-muted-foreground" />
      case 'failed':
        return <AlertCircle className="h-4 w-4 text-destructive" />
      case 'completed':
        return <CheckCircle2 className="h-4 w-4 text-success" />
      default:
        return <Clock className="h-4 w-4 text-muted-foreground" />
    }
  }

  return (
    <Card>
      <CardHeader>
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <CardTitle className="flex items-center gap-2">
            <Box className="h-5 w-5" />
            Models
            {registeredModels && (
              <Badge variant="secondary" className="ml-1">
                {(registeredModels as ModelInfo[]).length}
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
                className="pl-9 w-48"
              />
            </div>
            <Input
              id="hf-register-url"
              placeholder="owner/model-name"
              value={registerUrl}
              onChange={(e) => setRegisterUrl(e.target.value)}
              className="w-64"
            />
            <Button
              id="hf-register-url-submit"
              onClick={() => registerMutation.mutate(registerUrl)}
              disabled={!registerUrl || registerMutation.isPending}
            >
              {registerMutation.isPending ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <Plus className="mr-2 h-4 w-4" />
              )}
              Register
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent id="local-models-list">
        <div id="registering-tasks">
          {isLoadingRegistered ? (
            <div className="space-y-4">
              {[...Array(3)].map((_, i) => (
                <div key={i} className="h-24 shimmer rounded" />
              ))}
            </div>
          ) : (!filteredRegistered || filteredRegistered.length === 0) && (!activeConvertTasks || activeConvertTasks.length === 0) ? (
            <div className="flex h-32 flex-col items-center justify-center gap-2 text-muted-foreground">
              <Box className="h-8 w-8" />
              <p>No registered models</p>
              <p className="text-xs">Use the input field above to register a model</p>
            </div>
          ) : (
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {/* Converting/Queued Tasks */}
              {activeConvertTasks?.map((task) => (
                <Card key={task.id} className="overflow-hidden border-dashed border-2">
                  <CardContent className="p-4">
                    <div className="flex items-start justify-between">
                      <div className="space-y-1">
                        <h4 className="font-medium flex items-center gap-2">
                          {getConvertTaskIcon(task.status)}
                          {task.repo}
                        </h4>
                        {task.filename && (
                          <p className="text-xs text-muted-foreground">
                            {task.filename}
                          </p>
                        )}
                      </div>
                      {getConvertTaskStatusBadge(task.status)}
                    </div>

                    {/* Progress bar for in_progress tasks */}
                    {task.status === 'in_progress' && (
                      <div className="mt-3">
                        <div className="h-1.5 w-full rounded-full bg-muted">
                          <div
                            className="h-full rounded-full bg-primary transition-all"
                            style={{ width: `${task.progress * 100}%` }}
                          />
                        </div>
                        <p className="mt-1 text-xs text-muted-foreground">
                          {(task.progress * 100).toFixed(1)}%
                        </p>
                      </div>
                    )}

                    {/* Error message for failed tasks */}
                    {task.status === 'failed' && task.error && (
                      <p className="mt-2 text-xs text-destructive line-clamp-2">
                        {task.error}
                      </p>
                    )}

                    {/* Delete/Cancel button */}
                    <div className="mt-3">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => deleteConvertTaskMutation.mutate(task.id)}
                        disabled={deleteConvertTaskMutation.isPending}
                      >
                        <Trash2 className="mr-1 h-3 w-3 text-destructive" />
                        {task.status === 'failed' ? 'Delete' : 'Cancel'}
                      </Button>
                    </div>
                  </CardContent>
                </Card>
              ))}

              {/* Registered Models */}
              {filteredRegistered?.map((model) => (
                <Card key={model.name} className="overflow-hidden">
                  <CardContent className="p-4">
                    <div className="flex items-start justify-between">
                      <div className="space-y-1">
                        <h4 className="font-medium flex items-center gap-2">
                          {getStateIcon(model.state)}
                          {model.name}
                        </h4>
                        <p className="text-xs text-muted-foreground">
                          {model.source || 'Local'}
                        </p>
                      </div>
                      {getStateBadge(model.state)}
                    </div>

                    <div className="mt-3 space-y-1 text-sm">
                      {model.size_bytes && (
                        <p className="text-muted-foreground">
                          Size: {formatBytes(model.size_bytes)}
                        </p>
                      )}
                      {model.format && (
                        <p className="text-muted-foreground">
                          Format: {model.format}
                        </p>
                      )}
                      {model.progress !== undefined && model.progress < 100 && (
                        <div className="mt-2">
                          <div className="h-1.5 w-full rounded-full bg-muted">
                            <div
                              className="h-full rounded-full bg-primary transition-all"
                              style={{ width: `${model.progress}%` }}
                            />
                          </div>
                          <p className="mt-1 text-xs text-muted-foreground">
                            {model.progress.toFixed(1)}%
                          </p>
                        </div>
                      )}
                    </div>

                    <div className="mt-4 flex gap-2">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => deleteMutation.mutate(model.name)}
                        disabled={deleteMutation.isPending}
                      >
                        <Trash2 className="mr-1 h-3 w-3 text-destructive" />
                        Delete
                      </Button>
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
