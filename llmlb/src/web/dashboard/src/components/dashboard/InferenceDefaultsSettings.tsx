import { useId } from 'react'
import { Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { useInferenceDefaultsViewModel } from '@/viewmodels/useInferenceDefaultsViewModel'

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : 'Unknown error'
}

export function InferenceDefaultsSettings() {
  const modelId = useId()
  const descriptionId = useId()
  const vm = useInferenceDefaultsViewModel()
  if (!vm.isAdmin) return null

  if (!vm.hasSettings) {
    return vm.settingsError ? (
      <div className="space-y-3">
        <p role="alert" className="break-words text-sm text-destructive">
          Failed to load inference defaults: {errorMessage(vm.settingsError)}
        </p>
        <Button variant="outline" onClick={vm.retrySettings}>Retry settings</Button>
      </div>
    ) : (
      <p role="status" className="text-sm text-muted-foreground">Loading inference defaults...</p>
    )
  }

  return (
    <section className="min-w-0 space-y-4" aria-label="Inference defaults" aria-busy={vm.isSaving}>
      {vm.settingsError ? (
        <div className="space-y-2">
          <p role="alert" className="break-words text-sm text-destructive">
            Failed to load inference defaults: {errorMessage(vm.settingsError)}
          </p>
          <Button variant="outline" onClick={vm.retrySettings} disabled={vm.isSaving}>Retry settings</Button>
        </div>
      ) : null}
      <div className="space-y-2 rounded-lg border border-border p-4">
        <Label htmlFor={modelId}>Default text model</Label>
        <p id={descriptionId} className="text-sm leading-relaxed text-muted-foreground">
          Used when model is omitted. The model must support the requested API.
          {' '}Does not change embedding, image generation, or audio defaults.
        </p>
        <select
          id={modelId}
          aria-describedby={descriptionId}
          value={vm.selectedModel}
          onChange={(event) => vm.edit(event.target.value)}
          disabled={vm.isSaving || vm.isLoadingCatalog}
          className="h-10 w-full min-w-0 rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
        >
          <option value="">Not set (model required)</option>
          {vm.options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
        </select>
        {vm.isLoadingCatalog ? <p role="status" className="text-sm text-muted-foreground">Loading model catalog...</p> : null}
        {vm.savedModelMissing ? (
          <p className="break-words text-sm text-amber-700 dark:text-amber-400">
            Saved model is unavailable. Choose another model or select Not set to clear it.
          </p>
        ) : null}
        {vm.catalogError ? (
          <div className="space-y-2">
            <p role="alert" className="break-words text-sm text-destructive">
              Failed to load model catalog: {errorMessage(vm.catalogError)}
            </p>
            <Button size="sm" variant="outline" onClick={vm.retryCatalog} disabled={vm.isSaving}>Retry catalog</Button>
          </div>
        ) : null}
      </div>
      {vm.saveError ? (
        <p role="alert" className="break-words text-sm text-destructive">
          Failed to save inference defaults: {errorMessage(vm.saveError)}
        </p>
      ) : null}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p role="status" className="min-w-0 flex-1 text-sm text-muted-foreground">
          {vm.isSaved ? 'Saved. Applies to new requests that omit model.' : vm.isDirty ? 'Unsaved changes' : 'Applies across this llmlb instance.'}
        </p>
        <Button onClick={vm.save} disabled={!vm.canSave}>
          {vm.isSaving ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
          {vm.isSaving ? 'Saving...' : 'Save'}
        </Button>
      </div>
    </section>
  )
}
