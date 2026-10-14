import { type DashboardEndpoint } from '@/lib/api'
import { useModelDownloadDialogViewModel } from '@/viewmodels/useModelDownloadDialogViewModel'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Progress } from '@/components/ui/progress'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Download, Loader2, CheckCircle, XCircle } from 'lucide-react'

/**
 * SPEC-e8e9326e: xLLM Model Download Dialog
 * Allows downloading models to xLLM endpoints
 */

interface ModelDownloadDialogProps {
  endpoint: DashboardEndpoint | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function ModelDownloadDialog({
  endpoint,
  open,
  onOpenChange,
}: ModelDownloadDialogProps) {
  return (
    <ModelDownloadDialogContent
      key={`${endpoint?.id ?? 'empty'}:${open ? 'open' : 'closed'}`}
      endpoint={endpoint}
      open={open}
      onOpenChange={onOpenChange}
    />
  )
}

function ModelDownloadDialogContent({
  endpoint,
  open,
  onOpenChange,
}: ModelDownloadDialogProps) {
  const {
    supportsDownload, modelName, setModelName, status, progress, progressMessage,
    errorMessage, isDownloadPending, canDownload, handleDownload, handleClose, handleOpenChange,
  } = useModelDownloadDialogViewModel({ endpoint, open, onOpenChange })

  if (!supportsDownload || !endpoint) return null

  return (
    <Dialog
      open={open}
      onOpenChange={handleOpenChange}
    >
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Download className="h-5 w-5" />
            Download Model
          </DialogTitle>
          <DialogDescription>
            {`Download a model to ${endpoint.name}`}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {status === 'idle' && (
            <div className="space-y-2">
              <Label htmlFor="model-name">Model Name</Label>
              <Input
                id="model-name"
                placeholder="e.g., llama-3.1-8b-instruct"
                value={modelName}
                onChange={(e) => setModelName(e.target.value)}
                disabled={isDownloadPending}
              />
              <p className="text-xs text-muted-foreground">
                Enter the model name as recognized by xLLM
              </p>
            </div>
          )}

          {status === 'downloading' && (
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <Loader2 className="h-4 w-4 animate-spin" />
                <span className="text-sm font-medium">{`Downloading ${modelName}`}</span>
              </div>
              <Progress value={progress} className="h-2" />
              <p className="text-xs text-muted-foreground">{progressMessage}</p>
            </div>
          )}

          {status === 'completed' && (
            <div className="flex flex-col items-center gap-2 py-4">
              <CheckCircle className="h-12 w-12 text-green-500" />
              <p className="text-sm font-medium">Download Completed</p>
              <p className="text-xs text-muted-foreground">
                {`${modelName} is now available`}
              </p>
            </div>
          )}

          {status === 'error' && (
            <div className="flex flex-col items-center gap-2 py-4">
              <XCircle className="h-12 w-12 text-destructive" />
              <p className="text-sm font-medium text-destructive">Download Failed</p>
              <p className="text-xs text-muted-foreground">{errorMessage}</p>
            </div>
          )}
        </div>

        <DialogFooter>
          {(status === 'idle' || status === 'error') && (
            <>
              <Button variant="outline" onClick={handleClose}>
                Cancel
              </Button>
              <Button
                onClick={handleDownload}
                disabled={!canDownload}
              >
                {isDownloadPending && (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                )}
                Download
              </Button>
            </>
          )}
          {status === 'completed' && (
            <Button onClick={handleClose}>Close</Button>
          )}
          {status === 'downloading' && (
            <p className="text-xs text-muted-foreground">
              Download in progress...
            </p>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
