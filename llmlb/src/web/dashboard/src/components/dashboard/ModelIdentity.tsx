import { Badge } from '@/components/ui/badge'

/**
 * Issue #722: モデル ID を canonical 名とエンドポイントが報告した生のモデル名に
 * 区別して表示する。
 *
 * - `isCanonical`: この ID 自体が既知の canonical 名なら ID の横に canonical バッジ
 * - `canonicalName` が ID と異なる: 生のモデル名として ID を表示し、共有 canonical 名を下段に表示
 * - `aliases`: canonical 行に集約された生のモデル名
 */
interface ModelIdentityProps {
  id: string
  canonicalName?: string | null
  aliases?: string[]
  isCanonical?: boolean
}

function CanonicalBadge() {
  return (
    <Badge
      variant="outline"
      data-model-canonical-badge
      className="h-5 shrink-0 border-primary/30 bg-primary/10 px-1.5 py-0 text-[10px] font-semibold text-primary"
    >
      canonical
    </Badge>
  )
}

export function ModelIdentity({
  id,
  canonicalName,
  aliases = [],
  isCanonical = false,
}: ModelIdentityProps) {
  const sharedCanonical = canonicalName && canonicalName !== id ? canonicalName : null
  const visibleAliases = aliases.filter(
    (alias) => alias.length > 0 && alias !== id && alias !== canonicalName
  )

  return (
    <div className="min-w-0 space-y-1" data-model-id={id}>
      <div className="flex min-w-0 items-center gap-2">
        <span className="truncate font-mono text-sm" title={id}>
          {id}
        </span>
        {isCanonical && !sharedCanonical && <CanonicalBadge />}
      </div>

      {sharedCanonical && (
        <div
          className="flex min-w-0 items-center gap-2"
          data-model-canonical-name={sharedCanonical}
        >
          <span
            className="truncate font-mono text-xs text-muted-foreground"
            title={sharedCanonical}
          >
            {sharedCanonical}
          </span>
          <CanonicalBadge />
        </div>
      )}

      {visibleAliases.length > 0 && (
        <div className="flex min-w-0 flex-wrap gap-1">
          {visibleAliases.map((alias) => (
            <span
              key={alias}
              data-model-alias={alias}
              className="max-w-full truncate rounded border bg-muted/40 px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground"
              title={alias}
            >
              {alias}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}
