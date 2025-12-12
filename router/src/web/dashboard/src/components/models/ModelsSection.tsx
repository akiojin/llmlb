import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { type DashboardNode, modelsApi, type ModelInfo, type AvailableModel } from '@/lib/api'
import { formatBytes, cn } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  Box,
  Search,
  Plus,
  Download,
  Trash2,
  Send,
  RefreshCw,
  Server,
  CheckCircle2,
  Clock,
  AlertCircle,
  Loader2,
  ExternalLink,
} from 'lucide-react'

interface ModelsSectionProps {
  nodes: DashboardNode[]
}

export function ModelsSection({ nodes }: ModelsSectionProps) {
  const queryClient = useQueryClient()
  const [search, setSearch] = useState('')
  const [registerUrl, setRegisterUrl] = useState('')
  const [registerOpen, setRegisterOpen] = useState(false)
  const [distributeOpen, setDistributeOpen] = useState(false)
  const [convertOpen, setConvertOpen] = useState(false)
  const [selectedModel, setSelectedModel] = useState<ModelInfo | null>(null)
  const [selectedNodes, setSelectedNodes] = useState<string[]>([])
  const [convertFormat, setConvertFormat] = useState('gguf')

  // Fetch registered models
  const { data: registeredModels, isLoading: isLoadingRegistered } = useQuery({
    queryKey: ['registered-models'],
    queryFn: modelsApi.getRegistered,
    refetchInterval: 10000,
  })

  // Fetch available models (from HF catalog)
  const { data: availableModels, isLoading: isLoadingAvailable } = useQuery({
    queryKey: ['available-models'],
    queryFn: modelsApi.getAvailable,
    refetchInterval: 30000,
  })

  // Register model mutation
  const registerMutation = useMutation({
    mutationFn: (url: string) => modelsApi.register(url),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      toast({ title: 'Model registered successfully' })
      setRegisterUrl('')
      setRegisterOpen(false)
    },
    onError: (error) => {
      toast({
        title: 'Failed to register model',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Pull model mutation
  const pullMutation = useMutation({
    mutationFn: (modelName: string) => modelsApi.pull(modelName),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      toast({ title: 'Model pull started' })
    },
    onError: (error) => {
      toast({
        title: 'Failed to pull model',
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

  // Distribute model mutation
  const distributeMutation = useMutation({
    mutationFn: ({ modelName, nodeIds }: { modelName: string; nodeIds: string[] }) =>
      modelsApi.distribute(modelName, nodeIds),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      toast({ title: 'Model distribution started' })
      setDistributeOpen(false)
      setSelectedNodes([])
    },
    onError: (error) => {
      toast({
        title: 'Failed to distribute model',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Convert model mutation
  const convertMutation = useMutation({
    mutationFn: ({ modelName, format }: { modelName: string; format: string }) =>
      modelsApi.convert(modelName, format),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['registered-models'] })
      toast({ title: 'Model conversion started' })
      setConvertOpen(false)
    },
    onError: (error) => {
      toast({
        title: 'Failed to start conversion',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  const filteredRegistered = (registeredModels as ModelInfo[] | undefined)?.filter((m) =>
    m.name.toLowerCase().includes(search.toLowerCase())
  )

  const filteredAvailable = (availableModels as AvailableModel[] | undefined)?.filter((m) =>
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

  const onlineNodes = nodes.filter((n) => n.status === 'online')

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
                  {(registeredModels as ModelInfo[]).length}
                </Badge>
              )}
            </TabsTrigger>
            <TabsTrigger value="available" className="gap-2">
              <Download className="h-4 w-4" />
              Available
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

        {/* Registered Models Tab */}
        <TabsContent value="registered">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Box className="h-5 w-5" />
                Registered Models
              </CardTitle>
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
                  <Button variant="outline" size="sm" onClick={() => setRegisterOpen(true)}>
                    <Plus className="mr-2 h-4 w-4" />
                    Register a model
                  </Button>
                </div>
              ) : (
                <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                  {filteredRegistered.map((model) => (
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
                            variant="outline"
                            size="sm"
                            onClick={() => {
                              setSelectedModel(model)
                              setDistributeOpen(true)
                            }}
                            disabled={model.state !== 'ready'}
                          >
                            <Send className="mr-1 h-3 w-3" />
                            Distribute
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => {
                              setSelectedModel(model)
                              setConvertOpen(true)
                            }}
                            disabled={model.state !== 'ready'}
                          >
                            <RefreshCw className="mr-1 h-3 w-3" />
                            Convert
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => deleteMutation.mutate(model.name)}
                            disabled={deleteMutation.isPending}
                          >
                            <Trash2 className="h-3 w-3 text-destructive" />
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

        {/* Available Models Tab */}
        <TabsContent value="available">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Download className="h-5 w-5" />
                Available Models (Hugging Face)
              </CardTitle>
            </CardHeader>
            <CardContent>
              {isLoadingAvailable ? (
                <div className="space-y-4">
                  {[...Array(5)].map((_, i) => (
                    <div key={i} className="h-16 shimmer rounded" />
                  ))}
                </div>
              ) : !filteredAvailable || filteredAvailable.length === 0 ? (
                <div className="flex h-32 flex-col items-center justify-center gap-2 text-muted-foreground">
                  <Download className="h-8 w-8" />
                  <p>No available models found</p>
                </div>
              ) : (
                <ScrollArea className="h-96">
                  <div className="space-y-2">
                    {filteredAvailable.map((model) => (
                      <div
                        key={model.name}
                        className="flex items-center justify-between rounded-lg border p-3 hover:bg-muted/50"
                      >
                        <div className="space-y-1">
                          <h4 className="font-medium">{model.name}</h4>
                          <div className="flex items-center gap-2 text-xs text-muted-foreground">
                            {model.downloads !== undefined && (
                              <span>{model.downloads.toLocaleString()} downloads</span>
                            )}
                            {model.likes !== undefined && (
                              <span>{model.likes.toLocaleString()} likes</span>
                            )}
                          </div>
                        </div>
                        <div className="flex gap-2">
                          {model.url && (
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => window.open(model.url, '_blank')}
                            >
                              <ExternalLink className="h-4 w-4" />
                            </Button>
                          )}
                          <Button
                            size="sm"
                            onClick={() => pullMutation.mutate(model.name)}
                            disabled={pullMutation.isPending}
                          >
                            <Download className="mr-1 h-3 w-3" />
                            Pull
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
      </Tabs>

      {/* Register Model Dialog */}
      <Dialog open={registerOpen} onOpenChange={setRegisterOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Register Model</DialogTitle>
            <DialogDescription>
              Enter a Hugging Face model URL or model name to register.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="model-url">Model URL or Name</Label>
              <Input
                id="model-url"
                placeholder="https://huggingface.co/... or model-name"
                value={registerUrl}
                onChange={(e) => setRegisterUrl(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setRegisterOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={() => registerMutation.mutate(registerUrl)}
              disabled={!registerUrl || registerMutation.isPending}
            >
              {registerMutation.isPending && (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              )}
              Register
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Distribute Model Dialog */}
      <Dialog open={distributeOpen} onOpenChange={setDistributeOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Distribute Model</DialogTitle>
            <DialogDescription>
              Select nodes to distribute "{selectedModel?.name}" to.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            {onlineNodes.length === 0 ? (
              <div className="flex h-24 items-center justify-center text-muted-foreground">
                No online nodes available
              </div>
            ) : (
              <div className="space-y-2">
                {onlineNodes.map((node) => (
                  <div
                    key={node.node_id}
                    className="flex items-center gap-3 rounded-lg border p-3"
                  >
                    <Checkbox
                      id={node.node_id}
                      checked={selectedNodes.includes(node.node_id)}
                      onCheckedChange={(checked) => {
                        if (checked) {
                          setSelectedNodes([...selectedNodes, node.node_id])
                        } else {
                          setSelectedNodes(selectedNodes.filter((id) => id !== node.node_id))
                        }
                      }}
                    />
                    <Label htmlFor={node.node_id} className="flex-1 cursor-pointer">
                      <div className="flex items-center gap-2">
                        <Server className="h-4 w-4" />
                        {node.custom_name || node.machine_name}
                      </div>
                      <p className="text-xs text-muted-foreground">{node.ip_address}</p>
                    </Label>
                  </div>
                ))}
              </div>
            )}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDistributeOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={() => {
                if (selectedModel) {
                  distributeMutation.mutate({
                    modelName: selectedModel.name,
                    nodeIds: selectedNodes,
                  })
                }
              }}
              disabled={selectedNodes.length === 0 || distributeMutation.isPending}
            >
              {distributeMutation.isPending && (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              )}
              Distribute
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Convert Model Dialog */}
      <Dialog open={convertOpen} onOpenChange={setConvertOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Convert Model</DialogTitle>
            <DialogDescription>
              Convert "{selectedModel?.name}" to a different format.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="convert-format">Target Format</Label>
              <Select value={convertFormat} onValueChange={setConvertFormat}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="gguf">GGUF (llama.cpp)</SelectItem>
                  <SelectItem value="ggml">GGML (legacy)</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConvertOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={() => {
                if (selectedModel) {
                  convertMutation.mutate({
                    modelName: selectedModel.name,
                    format: convertFormat,
                  })
                }
              }}
              disabled={convertMutation.isPending}
            >
              {convertMutation.isPending && (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              )}
              Convert
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
