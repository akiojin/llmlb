import { Building2, Plus, Archive, Users } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { useOrganizationsViewModel } from '@/viewmodels/useOrganizationsViewModel'

const selectStyle = 'h-10 min-w-0 rounded-md border border-input bg-background px-3 text-sm'

export function OrganizationsModal({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const vm = useOrganizationsViewModel(open)
  return <Dialog open={open} onOpenChange={(next) => { if (!next) vm.clearKey(); onOpenChange(next) }}>
    <DialogContent className="max-h-[88vh] max-w-5xl overflow-y-auto">
      <DialogHeader><DialogTitle className="flex items-center gap-2"><Building2 className="h-5 w-5" />Organizations</DialogTitle>
        <DialogDescription>One shared inference URL. Separate membership and usage for each organization.</DialogDescription></DialogHeader>
      {vm.error && <p role="alert" className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">{vm.error}</p>}
      {vm.isGlobal && <form className="flex flex-wrap items-end gap-2 border-b pb-4" onSubmit={(event) => { event.preventDefault(); vm.create() }}>
        <div className="min-w-40 flex-1 space-y-2"><Label htmlFor="new-organization-name">Organization name</Label>
          <Input id="new-organization-name" value={vm.createName} onChange={(event) => vm.setCreateName(event.target.value)} maxLength={100} /></div>
        <Button type="submit" disabled={!vm.createName.trim() || vm.isPending}><Plus className="mr-2 h-4 w-4" />Create organization</Button>
      </form>}
      <div className="grid min-h-64 gap-5 md:grid-cols-[220px_1fr]">
        <nav aria-label="Organizations" className="space-y-1 border-b pb-3 md:border-b-0 md:border-r md:pr-4">
          {vm.organizations.map((org) => <button key={org.id} type="button" aria-pressed={vm.selected?.id === org.id}
            className={`flex w-full items-center justify-between gap-2 rounded-lg px-3 py-2 text-left text-sm ${vm.selected?.id === org.id ? 'bg-primary/10 text-primary' : 'hover:bg-muted'}`}
            onClick={() => vm.select(org.id)}><span className="break-words">{org.name}</span>{org.archived_at && <Archive className="h-4 w-4 shrink-0" aria-label="Archived" />}</button>)}
          {!vm.isLoading && !vm.organizations.length && <p className="p-3 text-sm text-muted-foreground">No organizations yet.</p>}
        </nav>
        {vm.selected ? <section className="min-w-0 space-y-4" aria-label="Organization detail">
          <div><h3 className="text-lg font-semibold">{vm.selected.name}</h3><p className="text-xs text-muted-foreground">{vm.selected.archived_at ? 'Archived · historical usage retained' : 'Active organization'} · Quota: not configured</p></div>
          {vm.isGlobal && !vm.selected.archived_at && <div className="flex flex-wrap items-end gap-2">
            <div className="min-w-40 flex-1 space-y-2"><Label htmlFor="rename-organization">Rename organization</Label><Input id="rename-organization" value={vm.renameName} onChange={(event) => vm.setRenameName(event.target.value)} /></div>
            <Button variant="outline" disabled={vm.isPending || !vm.renameName.trim()} onClick={vm.rename}>Save name</Button>
            <Button variant="outline" disabled={vm.isPending || vm.members.length > 0} onClick={vm.archive}>Archive</Button>
          </div>}
          <h4 className="flex items-center gap-2 text-sm font-medium"><Users className="h-4 w-4" />Accounts</h4>
          {vm.isLoading ? <p role="status">Loading accounts…</p> : vm.members.length ? <ul className="divide-y rounded-lg border">
            {vm.members.map((account) => <li key={account.id} className="flex flex-wrap items-center justify-between gap-2 p-3 text-sm">
              <div className="min-w-0"><p className="break-all font-medium">{account.username}</p><p className="text-xs text-muted-foreground">{account.role === 'admin' ? 'System administrator' : 'Account'}{account.is_active === false ? ' · Disabled' : ''}</p></div>
              {vm.isGlobal ? <div className="flex gap-2"><select className={selectStyle} aria-label={`Organization role for ${account.username}`} value={account.organization_role ?? 'member'} disabled={vm.isPending}
                onChange={(event) => vm.changeRole(account.id, event.target.value as 'admin' | 'member')}><option value="member">Member</option><option value="admin">Organization admin</option></select>
                <Button size="sm" variant="outline" aria-label={`Issue inference key for ${account.username}`} disabled={vm.isPending || account.is_active === false} onClick={() => vm.openIssueKey(account)}>Issue key</Button>
                <Button size="sm" variant="ghost" disabled={vm.isPending} onClick={() => vm.remove(account.id)}>Remove</Button></div> : <Badge variant="secondary">{account.organization_role === 'admin' ? 'Organization admin' : 'Member'}</Badge>}
            </li>)}
          </ul> : <p className="text-sm text-muted-foreground">No accounts assigned.</p>}
          {vm.isGlobal && !vm.selected.archived_at && <form className="space-y-2 rounded-lg border bg-muted/20 p-3" onSubmit={(event) => { event.preventDefault(); vm.assign() }}>
            <Label htmlFor="organization-account">Assign an existing account</Label><div className="flex flex-wrap gap-2">
              <select id="organization-account" className={`${selectStyle} flex-1`} value={vm.accountId} onChange={(event) => vm.setAccountId(event.target.value)}>
                <option value="">Select account</option>{vm.accounts.map((account) => <option key={account.id} value={account.id}>{account.username}</option>)}
              </select><select aria-label="New organization role" className={selectStyle} value={vm.role} onChange={(event) => vm.setRole(event.target.value as 'admin' | 'member')}><option value="member">Member</option><option value="admin">Organization admin</option></select>
              <Button type="submit" disabled={!vm.accountId || vm.isPending}>Assign</Button></div><p className="text-xs text-muted-foreground">Moving an account invalidates its old organization keys. Historical usage stays in the original organization.</p>
          </form>}
          {vm.keyAccount && <section aria-label="Issue attributed key" className="space-y-3 rounded-lg border border-primary/30 bg-primary/5 p-4">
            <h4 className="text-sm font-medium">Inference key for {vm.keyAccount.username}</h4><p className="text-xs text-muted-foreground">Usage belongs to this account and {vm.selected.name}. The issuer remains a separate audit identity.</p>
            {vm.issuedKey ? <><p className="text-sm">Copy this key now. It is shown only once.</p><code className="block break-all rounded-md bg-background p-3 text-sm select-all">{vm.issuedKey}</code><Button variant="outline" onClick={vm.clearKey}>Dismiss key</Button></> :
              <form className="flex flex-wrap items-end gap-2" onSubmit={(event) => { event.preventDefault(); vm.issueKey() }}><div className="min-w-32 flex-1 space-y-1"><Label htmlFor="attributed-key-name">Key name</Label><Input id="attributed-key-name" value={vm.keyName} onChange={(event) => vm.setKeyName(event.target.value)} /></div><Button type="submit" disabled={vm.isPending || !vm.keyName.trim()}>Issue key</Button><Button type="button" variant="ghost" onClick={vm.clearKey}>Cancel</Button></form>}
          </section>}
        </section> : <p className="text-sm text-muted-foreground">Create an organization to manage its accounts.</p>}
      </div>
    </DialogContent>
  </Dialog>
}
