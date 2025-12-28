import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { modelsApi, type RegisteredModelView, type LifecycleStatus } from '@/lib/api'
import { toast } from '@/hooks/use-toast'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Box, Search, Trash2, Download, Store } from 'lucide-react'
import { cn } from '@/lib/utils'
import { ModelHubTab } from './ModelHubTab'

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

  const { data: registeredModels, isLoading: isLoadingRegistered } = useQuery({
    queryKey: ['registered-models'],
    queryFn: modelsApi.getRegistered,
    refetchInterval: 10000,
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
            {/* Search */}
            <div className="relative">
              <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                placeholder="Search local models..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="pl-9"
              />
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
                  Pull a model from the Model Hub tab
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
  )
}
