import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  apiKeysApi,
  type ApiKey,
  type CreateApiKeyResponse,
} from '@/lib/api'
import { copyToClipboard, formatRelativeTime } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import {
  Key,
  Plus,
  Trash2,
  Copy,
  Check,
  Eye,
  EyeOff,
  Loader2,
  RefreshCw,
} from 'lucide-react'

interface ApiKeyModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function ApiKeyModal({ open, onOpenChange }: ApiKeyModalProps) {
  const queryClient = useQueryClient()
  const [createOpen, setCreateOpen] = useState(false)
  const [deleteKey, setDeleteKey] = useState<ApiKey | null>(null)
  const [newKeyName, setNewKeyName] = useState('')
  const [newKeyExpires, setNewKeyExpires] = useState('')
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [showKey, setShowKey] = useState<string | null>(null)
  const [copiedId, setCopiedId] = useState<string | null>(null)

  // Fetch API keys
  const {
    data: apiKeys,
    isLoading,
    isFetching,
    refetch,
  } = useQuery({
    queryKey: ['api-keys'],
    queryFn: apiKeysApi.list,
    enabled: open,
    // Plaintext keys are only shown once at creation time. We must not auto-refresh
    // this query while the modal is open, otherwise it becomes unclear whether the
    // key is still "copyable".
    refetchInterval: false,
    refetchOnWindowFocus: false,
  })

  // Create API key mutation
  const createMutation = useMutation({
    mutationFn: (data: { name: string; expires_at?: string }) =>
      apiKeysApi.create(data),
    onSuccess: (data: CreateApiKeyResponse) => {
      // Update list without refetching, so the "created key" stays visible/copyable
      // until the user explicitly refreshes or closes the modal.
      queryClient.setQueryData(['api-keys'], (old?: ApiKey[]) => {
        const next = Array.isArray(old) ? old : []
        const withoutDup = next.filter((k) => k.id !== data.id)
        const created: ApiKey = {
          id: data.id,
          name: data.name,
          key_prefix: data.key_prefix,
          created_at: data.created_at,
          expires_at: data.expires_at,
          permissions: data.permissions,
        }
        return [created, ...withoutDup]
      })

      setCreatedKey(data.key)
      setShowKey(null)
      setCopiedId(null)
      setNewKeyName('')
      setNewKeyExpires('')
      setCreateOpen(false)
      toast({ title: 'API key created' })
    },
    onError: (error) => {
      toast({
        title: 'Failed to create API key',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Delete API key mutation
  const deleteMutation = useMutation({
    mutationFn: (id: string) => apiKeysApi.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['api-keys'] })
      setDeleteKey(null)
      toast({ title: 'API key deleted' })
    },
    onError: (error) => {
      toast({
        title: 'Failed to delete API key',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Reset created key when modal closes
  useEffect(() => {
    if (!open) {
      setCreatedKey(null)
      setShowKey(null)
      setCopiedId(null)
    }
  }, [open])

  // Enforce: plaintext keys are copyable only immediately after creation.
  // Any background refetch/refresh should make copying impossible, requiring re-creation.
  useEffect(() => {
    if (!open) return
    if (!createdKey) return
    if (!isFetching) return
    setCreatedKey(null)
    setShowKey(null)
    setCopiedId(null)
  }, [open, createdKey, isFetching])

  useEffect(() => {
    if (!createOpen) {
      setNewKeyName('')
      setNewKeyExpires('')
    }
  }, [createOpen])

  const handleCopy = async (text: string, id: string) => {
    try {
      await copyToClipboard(text)
      setCopiedId(id)
      setTimeout(() => setCopiedId(null), 2000)
      toast({ title: 'Copied full API key' })
    } catch {
      toast({ title: 'Failed to copy', variant: 'destructive' })
    }
  }

  const handleCreate = () => {
    createMutation.mutate({
      name: newKeyName,
      expires_at: newKeyExpires || undefined,
    })
  }

  const isExpired = (expiresAt: string | null | undefined) => {
    if (!expiresAt) return false
    return new Date(expiresAt) < new Date()
  }

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent id="api-keys-modal" className="max-w-3xl max-h-[80vh] overflow-hidden">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Key className="h-5 w-5" />
              API Keys
            </DialogTitle>
            <DialogDescription>
              Manage your API keys for programmatic access.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            {/* Actions */}
            <div className="flex justify-between">
              <Button id="create-api-key" onClick={() => setCreateOpen(true)}>
                <Plus className="mr-2 h-4 w-4" />
                Create Key
              </Button>
              <Button
                variant="outline"
                size="icon"
                aria-label="Refresh API keys"
                title="Refresh API keys"
                onClick={() => {
                  // Enforce: plaintext keys are copyable only immediately after creation.
                  // Any "refresh" action should make copying impossible, requiring re-creation.
                  setCreatedKey(null)
                  setShowKey(null)
                  setCopiedId(null)
                  refetch()
                }}
              >
                <RefreshCw className="h-4 w-4" />
              </Button>
            </div>

            {/* Created Key Alert */}
            {createdKey && (
              <div className="rounded-lg border border-success/50 bg-success/10 p-4">
                <p className="text-sm font-medium text-success mb-2">
                  API Key Created Successfully
                </p>
                <p className="text-xs text-muted-foreground mb-2">
                  Copy this key now. You won't be able to see it again.
                </p>
                <div className="flex items-center gap-2">
                  <code className="flex-1 rounded bg-muted px-2 py-1 text-xs font-mono break-all">
                    {showKey === 'created' ? createdKey : '•'.repeat(32)}
                  </code>
                  <Button
                    variant="outline"
                    size="icon"
                    aria-label={showKey === 'created' ? 'Hide API key' : 'Show API key'}
                    onClick={() => setShowKey(showKey === 'created' ? null : 'created')}
                  >
                    {showKey === 'created' ? (
                      <EyeOff className="h-4 w-4" />
                    ) : (
                      <Eye className="h-4 w-4" />
                    )}
                  </Button>
                  <Button
                    id="copy-api-key"
                    variant="outline"
                    size="icon"
                    aria-label="Copy full API key"
                    title="Copy full API key"
                    data-copied={copiedId === 'created' ? 'true' : 'false'}
                    onClick={() => handleCopy(createdKey, 'created')}
                  >
                    {copiedId === 'created' ? (
                      <Check className="h-4 w-4" />
                    ) : (
                      <Copy className="h-4 w-4" />
                    )}
                  </Button>
                </div>
              </div>
            )}

            {/* API Keys Table */}
            <ScrollArea className="h-64 rounded-md border">
              {isLoading ? (
                <div className="flex h-full items-center justify-center">
                  <Loader2 className="h-6 w-6 animate-spin" />
                </div>
              ) : !apiKeys || (apiKeys as ApiKey[]).length === 0 ? (
                <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
                  <Key className="h-8 w-8" />
                  <p>No API keys</p>
                </div>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Name</TableHead>
                      <TableHead>Access</TableHead>
                      <TableHead>Key prefix</TableHead>
                      <TableHead>Created</TableHead>
                      <TableHead>Expires</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {(apiKeys as ApiKey[]).map((key) => (
                      <TableRow key={key.id}>
                        <TableCell className="font-medium">{key.name}</TableCell>
                        <TableCell>
                          <div className="flex flex-wrap gap-1">
                            <Badge variant="secondary">openai.inference</Badge>
                            <Badge variant="secondary">openai.models.read</Badge>
                          </div>
                        </TableCell>
                        <TableCell>
                          <span
                            className="text-xs text-muted-foreground select-none"
                            title="API keys are shown once at creation time; if lost, create a new key."
                          >
                            ••••••••••
                          </span>
                        </TableCell>
                        <TableCell className="text-sm text-muted-foreground">
                          {formatRelativeTime(key.created_at)}
                        </TableCell>
                        <TableCell>
                          {key.expires_at ? (
                            <Badge
                              variant={isExpired(key.expires_at) ? 'destructive' : 'outline'}
                            >
                              {isExpired(key.expires_at)
                                ? 'Expired'
                                : formatRelativeTime(key.expires_at)}
                            </Badge>
                          ) : (
                            <Badge variant="secondary">Never</Badge>
                          )}
                        </TableCell>
                        <TableCell className="text-right">
                          <Button
                            variant="outline"
                            size="icon"
                            className="h-8 w-8"
                            onClick={() => setDeleteKey(key)}
                          >
                            <Trash2 className="h-4 w-4 text-destructive" />
                          </Button>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </ScrollArea>
          </div>
        </DialogContent>
      </Dialog>

      {/* Create Key Dialog */}
      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent className="max-w-xl max-h-[80vh] overflow-y-auto border-border bg-card text-card-foreground">
          <DialogHeader>
            <DialogTitle>Create API Key</DialogTitle>
            <DialogDescription>
              Create a new API key for programmatic access.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="api-key-name">Name</Label>
              <Input
                id="api-key-name"
                placeholder="My API Key"
                value={newKeyName}
                onChange={(e) => setNewKeyName(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="key-expires">Expires (optional)</Label>
              <Input
                id="key-expires"
                type="datetime-local"
                value={newKeyExpires}
                onChange={(e) => setNewKeyExpires(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label className="text-foreground">Access</Label>
              <div className="rounded-md border border-border/70 bg-muted/20 p-3 text-sm text-muted-foreground">
                Keys created here always include:
                <div className="mt-2 flex flex-wrap gap-1">
                  <Badge variant="secondary">openai.inference</Badge>
                  <Badge variant="secondary">openai.models.read</Badge>
                </div>
              </div>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={handleCreate}
              disabled={!newKeyName || createMutation.isPending}
            >
              {createMutation.isPending && (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              )}
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirmation Dialog */}
      <AlertDialog open={!!deleteKey} onOpenChange={() => setDeleteKey(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete API Key</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to delete "{deleteKey?.name}"? This action cannot
              be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => deleteKey && deleteMutation.mutate(deleteKey.id)}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {deleteMutation.isPending && (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              )}
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}
