import { ArrowLeft, Building2, KeyRound, RefreshCw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { TablePagination } from '@/components/ui/table-pagination'
import { useOrganizationsViewModel } from '@/viewmodels/useOrganizationsViewModel'
import type { UsageRow, UsageTotals } from '@/lib/api/usage'

interface OrganizationsPageProps {
  onBack: () => void
}

const selectClass = 'h-10 rounded-md border border-input bg-background px-3 text-sm focus:outline-none focus:ring-2 focus:ring-ring disabled:opacity-50'
const count = (value: number) => value.toLocaleString()

function UsageSummary({ totals }: { totals: UsageTotals }) {
  const values = [
    ['Requests', totals.request_count], ['Input tokens', totals.input_tokens],
    ['Output tokens', totals.output_tokens], ['Total tokens', totals.total_tokens],
  ] as const
  return (
    <div className="space-y-3 border-y border-border py-4">
      <dl className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        {values.map(([label, value]) => (
          <div key={label}>
            <dt className="text-xs uppercase tracking-wide text-muted-foreground">{label}</dt>
            <dd className="mt-1 font-mono text-2xl font-semibold tabular-nums">{count(value)}</dd>
          </div>
        ))}
      </dl>
      <p className="text-sm text-muted-foreground">
        {`${count(totals.measured_requests)} measured · ${count(totals.estimated_requests)} estimated · ${count(totals.missing_requests)} missing`}
        {!!totals.pending_requests && ` · ${count(totals.pending_requests)} pending`}
      </p>
      <p className="text-xs text-muted-foreground">Totals cover all pages. Token totals include reported and estimated values; missing usage is not a measured zero.</p>
    </div>
  )
}

function UsageTable({ rows, loading }: { rows: UsageRow[]; loading: boolean }) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Organization / account</TableHead><TableHead className="text-right">Requests</TableHead>
          <TableHead className="text-right">Input</TableHead><TableHead className="text-right">Output</TableHead>
          <TableHead className="text-right">Total tokens</TableHead><TableHead>Coverage</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((row) => (
          <TableRow key={`${row.organization_id ?? 'unattributed'}:${row.account_id ?? 'organization'}`}>
            <TableCell>
              <div className="min-w-40 font-medium">{row.organization_id ? row.organization_name ?? 'Deleted organization' : 'Unattributed (legacy)'}</div>
              {row.organization_archived && <Badge variant="secondary" className="mt-1">Archived</Badge>}
              {row.account_id && <div className="mt-1 text-xs text-muted-foreground">{row.account_deleted ? 'Deleted account' : row.account_name ?? row.account_id}</div>}
            </TableCell>
            {[row.request_count, row.input_tokens, row.output_tokens, row.total_tokens].map((value, index) => (
              <TableCell key={index} className="text-right font-mono tabular-nums">{count(value)}</TableCell>
            ))}
            <TableCell className="min-w-36 text-xs text-muted-foreground">
              <div>Measured {count(row.measured_requests)}</div><div>Estimated {count(row.estimated_requests)}</div>
              <div>Missing {count(row.missing_requests)}</div>
              {!!row.pending_requests && <div>Pending {count(row.pending_requests)}</div>}
            </TableCell>
          </TableRow>
        ))}
        {rows.length === 0 && <TableRow><TableCell colSpan={6} className="h-24 text-center text-muted-foreground">{loading ? 'Loading usage…' : 'No observed usage in this period.'}</TableCell></TableRow>}
      </TableBody>
    </Table>
  )
}

export default function OrganizationsPage({ onBack }: OrganizationsPageProps) {
  const vm = useOrganizationsViewModel()

  if (vm.isAuthLoading) return <div className="flex min-h-screen items-center justify-center bg-background">Loading…</div>
  if (!vm.canView) {
    return (
      <main className="flex min-h-screen items-center justify-center bg-background p-6">
        <div className="max-w-md space-y-4 text-center">
          <Building2 className="mx-auto h-10 w-10 text-muted-foreground" />
          <h1 className="font-display text-xl font-semibold">Organization membership required</h1>
          <p className="text-sm text-muted-foreground">Ask a system administrator to assign your organization before viewing usage or using inference.</p>
          <Button variant="outline" onClick={onBack}>Back to Dashboard</Button>
        </div>
      </main>
    )
  }

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b border-border bg-background/90">
        <div className="mx-auto flex max-w-[1600px] flex-wrap items-center justify-between gap-4 px-4 py-5 sm:px-6 lg:px-8">
          <div className="flex items-center gap-3">
            <Button variant="ghost" size="icon" onClick={onBack} aria-label="Back to Dashboard"><ArrowLeft className="h-4 w-4" /></Button>
            <div>
              <p className="text-xs uppercase tracking-widest text-muted-foreground">{vm.scopeLabel}</p>
              <h1 className="mt-1 font-display text-xl font-semibold tracking-tight">Organizations &amp; usage</h1>
            </div>
          </div>
          <Button variant="outline" size="sm" onClick={vm.refresh}><RefreshCw className="mr-2 h-4 w-4" />Refresh</Button>
        </div>
      </header>

      <main className="mx-auto max-w-[1600px] space-y-6 px-4 py-6 sm:px-6 lg:px-8">
        {vm.errorMessage && <p role="alert" className="rounded-md border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive">{vm.errorMessage}</p>}
        <div className="grid gap-6 lg:grid-cols-[minmax(230px,300px)_minmax(0,1fr)]">
          <aside className="space-y-4">
            <Card>
              <CardHeader><h2 className="text-base font-semibold leading-none tracking-tight">Organizations</h2></CardHeader>
              <CardContent className="space-y-2">
                {vm.canManage && <Button variant={vm.selectedOrganizationId === null ? 'secondary' : 'ghost'} className="w-full justify-start" onClick={() => vm.selectOrganization(null)}>View all</Button>}
                {vm.organizations.map((org) => (
                  <Button key={org.id} variant={vm.selectedOrganizationId === org.id ? 'secondary' : 'ghost'}
                    className="h-auto w-full justify-between gap-2 whitespace-normal py-2 text-left" onClick={() => vm.selectOrganization(org.id)}>
                    <span className="min-w-0 break-words">{org.name}</span>{org.archived_at && <Badge variant="outline">Archived</Badge>}
                  </Button>
                ))}
                {vm.organizations.length === 0 && <p className="text-sm text-muted-foreground">No organizations yet.</p>}
              </CardContent>
            </Card>
            {vm.canManage && <Card>
              <CardHeader><h2 className="text-base font-semibold leading-none tracking-tight">Create organization</h2></CardHeader>
              <CardContent>
                <form className="space-y-3" onSubmit={(event) => { event.preventDefault(); vm.createOrganization() }}>
                  <Label htmlFor="new-organization-name">New organization name</Label>
                  <Input id="new-organization-name" value={vm.newOrganizationName} onChange={(event) => vm.setNewOrganizationName(event.target.value)} maxLength={200} disabled={vm.isSaving} />
                  <Button type="submit" className="w-full" disabled={!vm.newOrganizationName.trim() || vm.isSaving}>Create organization</Button>
                </form>
              </CardContent>
            </Card>}
            <p className="rounded-md border border-border bg-muted/30 p-4 text-xs leading-relaxed text-muted-foreground">Monthly quota is not available. This page reports usage; it does not enforce a monthly limit.</p>
          </aside>

          <div className="min-w-0 space-y-6">
            {vm.selectedAccount && <Card>
              <CardHeader><h2 className="text-base font-semibold leading-none tracking-tight">{`Account detail · ${vm.selectedAccount.username}`}</h2></CardHeader>
              <CardContent className="space-y-3">
                <dl className="grid gap-3 text-sm sm:grid-cols-3">
                  <div><dt className="text-xs text-muted-foreground">Organization</dt><dd>{vm.organizations.find((org) => org.id === vm.selectedAccount?.organization_id)?.name ?? 'Unassigned'}</dd></div>
                  <div><dt className="text-xs text-muted-foreground">Organization role</dt><dd>{vm.selectedAccount.organization_role === 'admin' ? 'Organization admin' : 'Member'}</dd></div>
                  <div><dt className="text-xs text-muted-foreground">Account status</dt><dd>{vm.selectedAccount.disabled ? 'Disabled' : 'Active'}</dd></div>
                </dl>
                <p className="text-xs text-muted-foreground">The usage below is filtered to this account within your permitted organization scope.</p>
                {vm.scopeLabel !== 'Your account' && <Button size="sm" variant="outline" onClick={() => vm.selectAccount(null)}>Clear account filter</Button>}
              </CardContent>
            </Card>}
            {vm.canManage && vm.selectedOrganization && <Card>
              <CardHeader><h2 className="text-base font-semibold leading-none tracking-tight">Organization details</h2></CardHeader>
              <CardContent>
                <form className="flex flex-wrap items-end gap-3" onSubmit={(event) => { event.preventDefault(); vm.renameOrganization() }}>
                  <div className="min-w-40 flex-1 space-y-2"><Label htmlFor="organization-name">Organization name</Label>
                    <Input id="organization-name" value={vm.renameName} onChange={(event) => vm.setRenameName(event.target.value)} disabled={vm.isSaving || !!vm.selectedOrganization.archived_at} /></div>
                  <Button type="submit" variant="outline" disabled={vm.isSaving || !vm.renameName.trim() || !!vm.selectedOrganization.archived_at}>Save name</Button>
                  <Button type="button" variant="outline" onClick={vm.archiveOrganization} disabled={vm.isSaving || vm.accounts.length > 0 || !!vm.selectedOrganization.archived_at}>Archive organization</Button>
                </form>
                <p className="mt-3 text-xs text-muted-foreground">An organization can be archived only after every account has been moved or unassigned. Historical usage remains.</p>
              </CardContent>
            </Card>}

            <Card>
              <CardHeader><h2 className="text-base font-semibold leading-none tracking-tight">Usage</h2></CardHeader>
              <CardContent className="space-y-4">
                <div className="flex flex-wrap items-end gap-3">
                  <div className="space-y-2"><Label htmlFor="usage-from">From (UTC, inclusive)</Label><Input id="usage-from" type="date" value={vm.from} onChange={(event) => vm.setFrom(event.target.value)} /></div>
                  <div className="space-y-2"><Label htmlFor="usage-to">To (UTC, inclusive)</Label><Input id="usage-to" type="date" value={vm.to} onChange={(event) => vm.setTo(event.target.value)} /></div>
                  {vm.canManage && <div className="space-y-2"><Label htmlFor="usage-grouping">Group by</Label><select id="usage-grouping" className={`${selectClass} block`} value={vm.groupBy} onChange={(event) => vm.setGroupBy(event.target.value as 'organization' | 'account')}><option value="organization">Organization</option><option value="account">Account</option></select></div>}
                </div>
                {vm.hasUsage ? <UsageSummary totals={vm.usageTotals} /> : <p role="status" className="py-4 text-sm text-muted-foreground">{vm.isLoading ? 'Loading usage…' : 'Usage is unavailable.'}</p>}
                <p className="text-xs text-muted-foreground">{vm.coverageStart ? `Coverage begins ${vm.coverageStart}. Earlier missing records are not reconstructed.` : 'No observations yet. Earlier missing records are not reconstructed.'}</p>
                <UsageTable rows={vm.usageRows} loading={vm.isLoading} />
                <TablePagination currentPage={vm.currentPage} totalPages={vm.totalPages} totalCount={vm.totalCount} onPageChange={vm.changePage} />
                <a className="inline-block text-sm text-primary underline underline-offset-4" href="/dashboard/?tab=history">Requests, details &amp; export</a>
              </CardContent>
            </Card>

            <Card>
              <CardHeader><h2 className="text-base font-semibold leading-none tracking-tight">Accounts</h2></CardHeader>
              <CardContent>
                <Table>
                  <TableHeader><TableRow><TableHead>Account</TableHead><TableHead>Organization role</TableHead><TableHead>Status</TableHead>{vm.canManage && <TableHead className="text-right">Actions</TableHead>}</TableRow></TableHeader>
                  <TableBody>
                    {vm.accounts.map((account) => <TableRow key={account.id}>
                      <TableCell><button type="button" className="text-left font-medium text-primary underline-offset-4 hover:underline focus-visible:rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" onClick={() => vm.selectAccount(account.id)} aria-label={`View account ${account.username}`}>{account.username}</button>{account.role === 'admin' && <div className="text-xs text-muted-foreground">System admin</div>}</TableCell>
                      <TableCell>{account.organization_id ? account.organization_role === 'admin' ? 'Organization admin' : 'Member' : 'Unassigned'}</TableCell>
                      <TableCell><Badge variant={account.disabled ? 'secondary' : 'outline'}>{account.disabled ? 'Disabled' : 'Active'}</Badge></TableCell>
                      {vm.canManage && <TableCell><div className="flex flex-wrap justify-end gap-2">
                        <Button size="sm" variant="outline" onClick={() => vm.editMembership(account)} disabled={vm.isSaving} aria-label={`Edit membership for ${account.username}`}>Membership</Button>
                        <Button size="sm" variant="outline" onClick={() => vm.openKeyForAccount(account)} disabled={!vm.canIssueFor(account)} aria-label={`Issue API key for ${account.username}`}><KeyRound className="mr-1 h-3 w-3" />Issue key</Button>
                        <Button size="sm" variant="ghost" onClick={() => vm.toggleDisabled(account)} disabled={vm.isSaving}>{account.disabled ? 'Enable' : 'Disable'}</Button>
                      </div></TableCell>}
                    </TableRow>)}
                    {vm.accounts.length === 0 && <TableRow><TableCell colSpan={vm.canManage ? 4 : 3} className="h-20 text-center text-muted-foreground">{vm.isLoading ? 'Loading accounts…' : 'No accounts in this view.'}</TableCell></TableRow>}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </div>
        </div>
      </main>

      <Dialog open={!!vm.editingAccount} onOpenChange={(open) => { if (!open) vm.cancelMembership() }}>
        <DialogContent>
          <DialogHeader><DialogTitle>Edit membership</DialogTitle><DialogDescription>{vm.editingAccount?.username} belongs to at most one organization. Previous usage stays with its original organization.</DialogDescription></DialogHeader>
          <form className="space-y-4" onSubmit={(event) => { event.preventDefault(); vm.saveMembership() }}>
            <div className="space-y-2"><Label htmlFor="membership-organization">Organization</Label><select id="membership-organization" className={`${selectClass} w-full`} value={vm.membershipOrganizationId} onChange={(event) => vm.setMembershipOrganizationId(event.target.value)} disabled={vm.isSaving}><option value="">Unassigned</option>{vm.organizations.filter((org) => !org.archived_at).map((org) => <option key={org.id} value={org.id}>{org.name}</option>)}</select></div>
            <div className="space-y-2"><Label htmlFor="membership-role">Organization role</Label><select id="membership-role" className={`${selectClass} w-full`} value={vm.membershipRole} onChange={(event) => vm.setMembershipRole(event.target.value as 'admin' | 'member')} disabled={vm.isSaving || !vm.membershipOrganizationId}><option value="member">Member</option><option value="admin">Organization admin</option></select></div>
            <p className="text-xs text-muted-foreground">Existing keys from a previous organization stop accepting new requests. Issue a new attributed key after a transfer.</p>
            <div className="flex justify-end gap-2"><Button type="button" variant="outline" onClick={vm.cancelMembership}>Cancel</Button><Button type="submit" disabled={vm.isSaving}>Save membership</Button></div>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog open={!!vm.keyTarget} onOpenChange={(open) => { if (!open) vm.clearKey() }}>
        <DialogContent>
          <DialogHeader><DialogTitle>Issue API key</DialogTitle><DialogDescription>Usage belongs to {vm.keyTarget?.username}, in their selected organization. The issuer is recorded separately.</DialogDescription></DialogHeader>
          {vm.createdKey ? <div className="space-y-4">
            <p className="text-sm text-muted-foreground">Copy this key now. It is shown only once and is removed when you close this dialog.</p>
            <Label htmlFor="issued-api-key">API key (shown once)</Label><Input id="issued-api-key" readOnly value={vm.createdKey} autoComplete="off" className="font-mono" />
            <div className="flex justify-end gap-2"><Button variant="outline" onClick={() => { void vm.copyCreatedKey() }}>{vm.keyCopied ? 'Copied' : 'Copy key'}</Button><Button onClick={vm.clearKey}>Done</Button></div>
          </div> : <form className="space-y-4" onSubmit={(event) => { event.preventDefault(); vm.issueKey() }}>
            <div className="space-y-2"><Label htmlFor="organization-key-name">Key name</Label><Input id="organization-key-name" value={vm.keyName} onChange={(event) => vm.setKeyName(event.target.value)} disabled={vm.isIssuingKey} autoComplete="off" /></div>
            <p className="text-xs text-muted-foreground">Permissions: inference and model listing. Management access is not included.</p>
            <div className="flex justify-end gap-2"><Button type="button" variant="outline" onClick={vm.clearKey}>Cancel</Button><Button type="submit" disabled={!vm.canIssueKey}>{vm.isIssuingKey ? 'Issuing…' : 'Issue key'}</Button></div>
          </form>}
        </DialogContent>
      </Dialog>
    </div>
  )
}
