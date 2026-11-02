import { useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { organizationsApi, type OrganizationAccount } from '@/lib/api/organizations'
import { apiKeysApi } from '@/lib/api/api-keys'
import { usersApi } from '@/lib/api/users'
import { getSessionGeneration } from '@/lib/api/client'
import { useAuth } from '@/hooks/useAuth'

interface IssueKeyCommand {
  generation: number
  attempt: number
  payload: Parameters<typeof apiKeysApi.create>[0]
}

export function useOrganizationsViewModel(open: boolean) {
  const { user, checkAuth } = useAuth()
  const isGlobal = user?.role === 'admin'
  const queryClient = useQueryClient()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [createName, setCreateName] = useState('')
  const [renameName, setRenameName] = useState<string | null>(null)
  const [accountId, setAccountId] = useState('')
  const [role, setRole] = useState<'admin' | 'member'>('member')
  const [keyAccount, setKeyAccount] = useState<OrganizationAccount | null>(null)
  const [keyName, setKeyName] = useState('')
  const [issuedKey, setIssuedKey] = useState<string | null>(null)
  const keyAttempt = useRef(0)
  const organizations = useQuery({ queryKey: ['organizations', user?.id, user?.organization_id],
    queryFn: organizationsApi.list, enabled: open })
  const selected = organizations.data?.find((org) => org.id === selectedId) ?? organizations.data?.[0]
  const members = useQuery({ queryKey: ['organization-members', user?.id, selected?.id],
    queryFn: () => organizationsApi.members(selected!.id), enabled: open && !!selected })
  const accounts = useQuery({ queryKey: ['organization-assignable-accounts'], queryFn: usersApi.list,
    enabled: open && isGlobal })
  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['organizations'] }),
      queryClient.invalidateQueries({ queryKey: ['organization-members'] }),
      queryClient.invalidateQueries({ queryKey: ['organization-assignable-accounts'] }),
      queryClient.invalidateQueries({ queryKey: ['usage'] }),
    ])
  }
  const create = useMutation({ mutationFn: () => organizationsApi.create(createName.trim()),
    onSuccess: async (org) => { setCreateName(''); setSelectedId(org.id); setRenameName(null); await refresh() } })
  const rename = useMutation({ mutationFn: () => organizationsApi.rename(selected!.id, (renameName ?? selected!.name).trim()),
    onSuccess: refresh })
  const archive = useMutation({ mutationFn: () => organizationsApi.archive(selected!.id),
    onSuccess: async () => { setSelectedId(null); setRenameName(null); await refresh() } })
  const membership = useMutation({
    mutationFn: ({ id, org, role }: { id: string; org: string | null; role: 'admin' | 'member' }) =>
      organizationsApi.setMembership(id, org, role),
    onSuccess: async (_, change) => { setAccountId(''); await refresh(); if (change.id === user?.id) await checkAuth() },
  })
  const isCurrentKeyCommand = (command: IssueKeyCommand) =>
    command.generation === getSessionGeneration() && command.attempt === keyAttempt.current
  const issueKey = useMutation({
    mutationFn: (command: IssueKeyCommand) => {
      if (!isCurrentKeyCommand(command)) throw new Error('API key issue command is no longer current')
      return apiKeysApi.create(command.payload)
    },
    onSuccess: (key, command) => {
      if (!isCurrentKeyCommand(command)) return
      setIssuedKey(key.key)
      setKeyName('')
    },
  })
  const clearKey = () => {
    keyAttempt.current++
    setKeyAccount(null)
    setKeyName('')
    setIssuedKey(null)
    issueKey.reset()
  }
  const error = organizations.error ?? members.error ?? accounts.error ?? create.error ?? rename.error ?? archive.error ?? membership.error ?? issueKey.error
  return { isGlobal, organizations: organizations.data ?? [], selected, members: members.data ?? [],
    accounts: accounts.data ?? [], isLoading: organizations.isLoading || members.isLoading,
    error: error instanceof Error ? error.message : null,
    select: (id: string) => { setSelectedId(id); setRenameName(null); setAccountId(''); clearKey() },
    createName, setCreateName, renameName: renameName ?? selected?.name ?? '', setRenameName,
    accountId, setAccountId, role, setRole,
    create: () => create.mutate(), rename: () => rename.mutate(), archive: () => archive.mutate(),
    assign: () => membership.mutate({ id: accountId, org: selected!.id, role }),
    changeRole: (id: string, role: 'admin' | 'member') => membership.mutate({ id, org: selected!.id, role }),
    remove: (id: string) => membership.mutate({ id, org: null, role: 'member' }),
    keyAccount, keyName, setKeyName, issuedKey, clearKey,
    openIssueKey: (account: OrganizationAccount) => { clearKey(); setKeyAccount(account) },
    issueKey: () => {
      if (!isGlobal || !open || !selected || !keyAccount) return
      issueKey.mutate({ generation: getSessionGeneration(), attempt: ++keyAttempt.current,
        payload: { name: keyName.trim(), usage_account_id: keyAccount.id,
          organization_id: selected.id, permissions: ['openai.inference', 'openai.models.read'] } })
    },
    isPending: create.isPending || rename.isPending || archive.isPending || membership.isPending || issueKey.isPending,
  }
}
