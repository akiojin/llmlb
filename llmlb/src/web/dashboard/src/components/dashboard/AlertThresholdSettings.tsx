import { useAlertThresholdSettingsViewModel } from '@/viewmodels/useAlertThresholdSettingsViewModel'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Settings2 } from 'lucide-react'

export function AlertThresholdSettings() {
  const {
    editing, inputValue, setInputValue, thresholdLabel, isPending,
    startEditing, save, cancelEditing, handleKeyDown,
  } = useAlertThresholdSettingsViewModel()

  if (editing) {
    return (
      <div className="flex items-center gap-2 text-sm">
        <Settings2 className="h-4 w-4 text-muted-foreground" />
        <span className="text-muted-foreground">Alert threshold (1h):</span>
        <Input
          type="number"
          min={1}
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          className="h-7 w-24"
          onKeyDown={(e) => handleKeyDown(e.key)}
          autoFocus
        />
        <Button size="sm" variant="outline" className="h-7" onClick={save} disabled={isPending}>
          Save
        </Button>
        <Button size="sm" variant="ghost" className="h-7" onClick={cancelEditing}>
          Cancel
        </Button>
      </div>
    )
  }

  return (
    <div className="flex items-center gap-2 text-sm">
      <Settings2 className="h-4 w-4 text-muted-foreground" />
      <span className="text-muted-foreground">Alert threshold (1h):</span>
      <span className="font-medium">{thresholdLabel}</span>
      <Button size="sm" variant="ghost" className="h-7" onClick={startEditing}>
        Edit
      </Button>
    </div>
  )
}
