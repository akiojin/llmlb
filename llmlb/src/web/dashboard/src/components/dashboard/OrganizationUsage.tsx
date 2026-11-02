import { Building2, ChevronLeft, ChevronRight, RefreshCw } from 'lucide-react'
import { useUsageViewModel } from '@/viewmodels/useUsageViewModel'
import { formatNumber } from '@/lib/utils'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Dialog, DialogContent, DialogHeader, DialogDescription, DialogTitle } from '@/components/ui/dialog'

export function OrganizationUsage() {
  const vm = useUsageViewModel()
  const totals = vm.data?.totals
  return <section aria-label="Organization and account usage">
    <Card><CardHeader className="space-y-4"><div className="flex flex-wrap items-start justify-between gap-3">
      <div><CardTitle className="flex items-center gap-2"><Building2 className="h-5 w-5" />Organization & account usage</CardTitle>
        <p className="mt-2 text-sm text-muted-foreground">{vm.scope === 'global' ? 'All organizations, including unattributed legacy usage.' : vm.scope === 'organization' ? 'Your organization only.' : 'Your account in your current organization only.'} Requests are grouped by their UTC receipt date.</p></div>
      <Button variant="outline" size="icon" aria-label="Refresh usage" onClick={vm.refresh}><RefreshCw className="h-4 w-4" /></Button>
    </div>
    <div className="flex flex-wrap items-end gap-3">
      <div className="space-y-1"><Label htmlFor="usage-from">From (UTC)</Label><Input id="usage-from" type="date" value={vm.period.from} onChange={(event) => vm.setDate('from', event.target.value)} /></div>
      <div className="space-y-1"><Label htmlFor="usage-to">Through (UTC, inclusive)</Label><Input id="usage-to" type="date" value={vm.period.to} onChange={(event) => vm.setDate('to', event.target.value)} /></div>
      {vm.isGlobal && <div className="space-y-1"><Label htmlFor="usage-group">Group by</Label><select id="usage-group" className="h-10 rounded-md border border-input bg-background px-3 text-sm" value={vm.groupBy} onChange={(event) => vm.setGroupBy(event.target.value as 'organization' | 'account')}><option value="organization">Organization</option><option value="account">Account</option></select></div>}
      <Badge variant="secondary" className="mb-2">Quota: not configured</Badge>
    </div>
    {vm.organizationId && <Button variant="link" className="w-fit p-0" onClick={vm.clearOrganization}>← All organizations</Button>}
    </CardHeader><CardContent className="space-y-4">
      {vm.error && <p role="alert" className="text-sm text-destructive">{vm.error}</p>}
      {totals && <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {[["Requests", totals.request_count], ["Input tokens", totals.input_tokens], ["Output tokens", totals.output_tokens], ["Total tokens", totals.total_tokens]].map(([label, value]) => <div key={label} className="rounded-lg border bg-muted/20 p-3"><p className="text-xs text-muted-foreground">{label}</p><p className="mt-1 font-mono text-xl tabular-nums">{formatNumber(Number(value))}</p></div>)}
      </div>}
      {totals && <p className="text-sm text-muted-foreground"><span>{formatNumber(totals.measured_requests)} measured</span> · <span>{formatNumber(totals.estimated_requests)} estimated</span> · <span>{formatNumber(totals.missing_requests)} missing</span>. Token totals include reported values and estimates; missing tokens are unknown.</p>}
      <div className="overflow-x-auto rounded-lg border"><table className="w-full text-left text-sm">
        <thead className="border-b bg-muted/40 text-xs text-muted-foreground"><tr><th className="p-3">{vm.groupBy === 'organization' ? 'Organization' : 'Account'}</th><th className="p-3 text-right">Requests</th><th className="p-3 text-right">Input</th><th className="p-3 text-right">Output</th><th className="p-3 text-right">Total</th><th className="p-3">Coverage</th></tr></thead>
        <tbody className="divide-y">{vm.isLoading ? <tr><td colSpan={6} className="p-6" role="status">Loading usage…</td></tr> : vm.rows.length ? vm.rows.map((row) => <tr key={`${row.organization_id ?? 'legacy'}:${row.account_id ?? 'all'}`} className="hover:bg-muted/20">
          <td className="p-3"><button className="text-left font-medium underline-offset-4 hover:underline" onClick={() => vm.openRow(row)}>{row.label}</button>
            {row.organization_archived && <p className="text-xs text-muted-foreground">Archived</p>}{row.account_deleted && <p className="text-xs text-muted-foreground">Deleted</p>}
            {vm.groupBy === 'account' && <p className="text-xs text-muted-foreground">{row.organization_name ?? 'Unattributed'}</p>}</td>
          {[row.request_count, row.input_tokens, row.output_tokens, row.total_tokens].map((value, index) => <td key={index} className="p-3 text-right font-mono tabular-nums">{formatNumber(value)}</td>)}
          <td className="whitespace-nowrap p-3 text-xs text-muted-foreground">{formatNumber(row.measured_requests)} measured<br />{formatNumber(row.estimated_requests)} estimated · {formatNumber(row.missing_requests)} missing</td>
        </tr>) : <tr><td colSpan={6} className="p-6 text-center text-muted-foreground">No observed usage in this period.</td></tr>}</tbody>
      </table></div>
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground"><p>{vm.data?.total_count ?? 0} {vm.groupBy === 'organization' ? 'organizations' : 'accounts'} · totals cover all pages</p>
        <div className="flex items-center gap-2"><Button size="icon" variant="outline" aria-label="Previous usage page" disabled={vm.page === 1} onClick={vm.previous}><ChevronLeft className="h-4 w-4" /></Button><span>Page {vm.page}</span><Button size="icon" variant="outline" aria-label="Next usage page" disabled={!vm.data || vm.page * vm.data.per_page >= vm.data.total_count} onClick={vm.next}><ChevronRight className="h-4 w-4" /></Button></div>
      </div>
      <p className="text-xs text-muted-foreground">{vm.data?.coverage_start ? `Observation started ${vm.data.coverage_start}.` : 'No receipt observations yet.'} Earlier missing history cannot be reconstructed. Unattributed legacy records are visible only to system administrators.</p>
    </CardContent></Card>
    <Dialog open={!!vm.detail} onOpenChange={(open) => { if (!open) vm.closeDetail() }}><DialogContent>
      <DialogHeader><DialogTitle>{vm.detailTitle}</DialogTitle><DialogDescription>Usage remains in the organization recorded when each request was received.</DialogDescription></DialogHeader>
      <p className="text-sm">{vm.detail?.organization_name ?? 'Unattributed organization'} · {formatNumber(vm.detail?.request_count ?? 0)} requests</p>
      <h3 className="text-sm font-medium">Attributed keys</h3>{vm.detailKeysError && <p role="alert" className="text-sm text-destructive">{vm.detailKeysError}</p>}
      {vm.detailKeys.length ? <ul className="divide-y rounded-md border">{vm.detailKeys.map((key) => <li key={key.id} className="p-3 text-sm"><span className="font-medium">{key.name}</span>{key.attribution_revoked_at && <Badge variant="destructive" className="ml-2">Revoked</Badge>}<p className="mt-1 text-xs text-muted-foreground">{key.key_prefix}… · Issuer: {key.created_by ?? 'Unavailable'}</p></li>)}</ul> : <p className="text-sm text-muted-foreground">No current keys. Historical usage is retained after key deletion.</p>}
    </DialogContent></Dialog>
  </section>
}
