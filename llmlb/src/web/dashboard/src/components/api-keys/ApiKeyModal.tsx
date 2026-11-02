import { useApiKeyModalViewModel } from '@/viewmodels/useApiKeyModalViewModel'
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
  const {
    isAdmin, createOpen, deleteKey, setDeleteKey, newKeyName, setNewKeyName,
    newKeyExpires, setNewKeyExpires,
    createdKey, showKey, copiedId, createdKeyCodeRef, apiKeys, isLoading, errorMessage,
    isCreating, isDeleting, canCreate, canIssue, handleMainOpenChange, handleCreateOpenChange,
    handleOpenCreateDialog, handleCopy, handleCreate,
    toggleCreatedKey, handleRefresh, handleDelete,
  } = useApiKeyModalViewModel({ open, onOpenChange })

  return (
    <>
      <Dialog open={open} onOpenChange={handleMainOpenChange}>
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
              <Button id="create-api-key" onClick={handleOpenCreateDialog} disabled={!canIssue}>
                <Plus className="mr-2 h-4 w-4" />
                Create Key
              </Button>
              <Button
                variant="outline"
                size="icon"
                aria-label="Refresh API keys"
                title="Refresh API keys"
                onClick={handleRefresh}
              >
                <RefreshCw className="h-4 w-4" />
              </Button>
            </div>
            {!canIssue && <p className="text-sm text-muted-foreground">Assign this account to an active organization before creating an inference key.</p>}
            {errorMessage && <p role="alert" className="text-sm text-destructive">{errorMessage}</p>}

            {/* Created Key Alert */}
            {createdKey && (
              <div className="rounded-lg border border-success/50 bg-success/10 p-4">
                <p className="text-sm font-medium text-success mb-2">
                  API Key Created Successfully
                </p>
                <p className="text-xs text-muted-foreground mb-2">
                  {"Copy this key now. You won't be able to see it again."}
                </p>
                <div className="flex items-center gap-2">
                  <code
                    ref={createdKeyCodeRef}
                    className="flex-1 rounded bg-muted px-2 py-1 text-xs font-mono break-all"
                  >
                    {showKey === 'created' ? createdKey : '•'.repeat(32)}
                  </code>
                  <Button
                    variant="outline"
                    size="icon"
                    aria-label={
                      showKey === 'created'
                        ? 'Hide API key'
                        : 'Show API key'
                    }
                    onClick={toggleCreatedKey}
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
              ) : !apiKeys || apiKeys.length === 0 ? (
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
                      <TableHead>Attribution</TableHead>
                      <TableHead>Key prefix</TableHead>
                      <TableHead>Created</TableHead>
                      <TableHead>Expires</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {apiKeys.map((key) => (
                      <TableRow key={key.id}>
                        <TableCell className="font-medium">{key.name}</TableCell>
                        <TableCell>
                          <div className="flex flex-wrap gap-1">
                            {key.permissions.map((permission) => (
                              <Badge key={`${key.id}-${permission}`} variant="secondary">
                                {permission}
                              </Badge>
                            ))}
                          </div>
                        </TableCell>
                        <TableCell>
                          {key.usage_account_id && key.organization_id ? <div className="max-w-64 break-all text-xs text-muted-foreground">{`Account ${key.usage_account_id} · Organization ${key.organization_id}`}</div>
                            : isAdmin && <Badge variant="outline">Unattributed (legacy)</Badge>}
                          <div className="mt-1 max-w-64 break-all text-xs text-muted-foreground">Issuer: {key.created_by ?? 'Unavailable'}</div>
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
                          {key.createdAtLabel}
                        </TableCell>
                        <TableCell>
                          {key.expires_at ? (
                            <Badge
                              variant={key.expired ? 'destructive' : 'outline'}
                            >
                              {key.expiresLabel}
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
                            disabled={!key.canDelete}
                            aria-label={`Delete ${key.name}`}
                            title={key.canDelete ? 'Delete API key' : 'Only the issuer can delete this key'}
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
      <Dialog open={createOpen} onOpenChange={handleCreateOpenChange}>
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
                  <>
                    New keys belong to your current account and organization and include:
                    <div className="mt-2 flex flex-wrap gap-1">
                      <Badge variant="secondary">openai.inference</Badge>
                      <Badge variant="secondary">openai.models.read</Badge>
                    </div>
                  </>
              </div>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => handleCreateOpenChange(false)}>
              Cancel
            </Button>
            <Button
              onClick={handleCreate}
              disabled={!canCreate}
            >
              {isCreating && (
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
              {`Are you sure you want to delete "${deleteKey?.name}"? This action cannot be undone.`}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={handleDelete}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {isDeleting && (
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
