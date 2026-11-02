import { useEffect, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useAuth } from '@/hooks/useAuth'
import {
  organizationsApi, type Organization, type OrganizationAccount,
  type OrganizationKeyRequest, type MembershipUpdate,
} from '@/lib/api/organizations'
import { usageApi, type UsageFilters, type UsageGrouping, type UsageRow, type UsageTotals } from '@/lib/api/usage'
import { AuthScopeChangedError, getAuthSessionSnapshot, isAuthSessionCurrent, type AuthSessionSnapshot } from '@/lib/authSession'
import { copyToClipboard, cleanupManualCopyBuffer, selectTextForManualCopy } from '@/lib/utils'
import { queryKeys } from '@/lib/queryKeys'

const emptyTotals: UsageTotals = {
  request_count: 0, input_tokens: 0, output_tokens: 0, total_tokens: 0,
  measured_requests: 0, estimated_requests: 0, missing_requests: 0,
}

interface KeyCommand {
  session: AuthSessionSnapshot
  attempt: number
  payload: OrganizationKeyRequest
}

type ManagementCommand =
  | { type: 'create'; name: string }
  | { type: 'rename'; id: string; name: string }
  | { type: 'archive'; id: string }
  | { type: 'membership'; id: string; membership: MembershipUpdate }
  | { type: 'disabled'; id: string; disabled: boolean }

export interface OrganizationsViewModel {
  isAuthLoading: boolean
  canView: boolean
  canManage: boolean
  scopeLabel: string
  organizations: Organization[]
  accounts: OrganizationAccount[]
  selectedOrganizationId: string | null
  selectedOrganization: Organization | null
  selectOrganization: (id: string | null) => void
  selectedAccount: OrganizationAccount | null
  selectAccount: (id: string | null) => void
  isLoading: boolean
  isSaving: boolean
  errorMessage: string | null
  newOrganizationName: string
  setNewOrganizationName: (name: string) => void
  createOrganization: () => void
  renameName: string
  setRenameName: (name: string) => void
  renameOrganization: () => void
  archiveOrganization: () => void
  editingAccount: OrganizationAccount | null
  membershipOrganizationId: string
  setMembershipOrganizationId: (id: string) => void
  membershipRole: 'admin' | 'member'
  setMembershipRole: (role: 'admin' | 'member') => void
  editMembership: (account: OrganizationAccount) => void
  saveMembership: () => void
  cancelMembership: () => void
  toggleDisabled: (account: OrganizationAccount) => void
  canIssueFor: (account: OrganizationAccount) => boolean
  keyTarget: OrganizationAccount | null
  keyName: string
  setKeyName: (name: string) => void
  createdKey: string | null
  isIssuingKey: boolean
  canIssueKey: boolean
  openKeyForAccount: (account: OrganizationAccount) => void
  issueKey: () => void
  clearKey: () => void
  copyCreatedKey: () => Promise<void>
  keyCopied: boolean
  from: string
  to: string
  setFrom: (value: string) => void
  setTo: (value: string) => void
  groupBy: UsageGrouping
  setGroupBy: (value: UsageGrouping) => void
  usageRows: UsageRow[]
  hasUsage: boolean
  usageTotals: UsageTotals
  coverageStart: string | null
  currentPage: number
  totalPages: number
  totalCount: number
  changePage: (page: number) => void
  refresh: () => void
}

/** The server decides authority; this ViewModel presents only the current scope. */
export function useOrganizationsViewModel(): OrganizationsViewModel {
  const { user, isLoading: isAuthLoading } = useAuth()
  const queryClient = useQueryClient()
  const session = getAuthSessionSnapshot()
  const canManage = !!user && user.role === 'admin' && !user.disabled
  const canView = !!user && !user.disabled && (canManage || !!user.organization_id)
  const ownOrganizationId = user?.organization_id ?? null
  const isMember = !canManage && user?.organization_role !== 'admin'
  // Until the user makes a selection, defaults follow the resolved server identity.
  // The first me may finish after this hook mounts; an unknown actor is no default.
  const [organizationChoice, setSelectedOrganizationId] = useState<string | null | undefined>(undefined)
  const selectedOrganizationId = organizationChoice === undefined ? canManage ? null : ownOrganizationId : organizationChoice
  const [accountChoice, setSelectedAccountId] = useState<string | null | undefined>(undefined)
  const selectedAccountId = accountChoice === undefined ? isMember ? user?.id ?? null : null : accountChoice
  const [newOrganizationName, setNewOrganizationName] = useState('')
  const [renameName, setRenameName] = useState('')
  const [editingAccount, setEditingAccount] = useState<OrganizationAccount | null>(null)
  const [membershipOrganizationId, setMembershipOrganizationId] = useState('')
  const [membershipRole, setMembershipRole] = useState<'admin' | 'member'>('member')
  const [operationError, setOperationError] = useState<string | null>(null)
  const today = new Date().toISOString().slice(0, 10)
  const [from, changeFrom] = useState(`${today.slice(0, 7)}-01`)
  const [to, changeTo] = useState(today)
  const [groupingChoice, changeGrouping] = useState<UsageGrouping>()
  const groupBy = groupingChoice ?? (canManage ? 'organization' : 'account')
  const [page, setPage] = useState(1)
  const [keyTarget, setKeyTarget] = useState<OrganizationAccount | null>(null)
  const keyTargetRef = useRef<OrganizationAccount | null>(null)
  const keyAttempt = useRef(0)
  const pendingKey = useRef<{ attempt: number; key: string } | null>(null)
  const [keyName, setKeyName] = useState('')
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [keyCopied, setKeyCopied] = useState(false)

  useEffect(() => () => {
    pendingKey.current = null
    keyTargetRef.current = null
    ++keyAttempt.current
    cleanupManualCopyBuffer()
  }, [])

  const organizationQuery = useQuery({
    queryKey: ['organizations', session.identity], queryFn: organizationsApi.list, enabled: canView,
  })
  const accountQuery = useQuery({
    queryKey: ['organization-accounts', session.identity], queryFn: organizationsApi.accounts, enabled: canView,
  })
  const organizations = (organizationQuery.data ?? []).filter((org) => canManage || org.id === ownOrganizationId)
  const allAccounts = (accountQuery.data ?? []).filter((account) => canManage || (
    account.organization_id === ownOrganizationId && (!isMember || account.id === user?.id)
  ))
  const accounts = allAccounts.filter((account) => !selectedOrganizationId || account.organization_id === selectedOrganizationId)
  const selectedOrganization = organizations.find((org) => org.id === selectedOrganizationId) ?? null
  const selectedAccount = allAccounts.find((account) => account.id === selectedAccountId) ?? null
  const filters: UsageFilters = {
    group_by: selectedAccountId || !canManage ? 'account' : groupBy, from, to, page, per_page: 25,
    ...((canManage ? selectedOrganizationId : ownOrganizationId) ? {
      organization_id: (canManage ? selectedOrganizationId : ownOrganizationId)!,
    } : {}),
    ...(isMember && user ? { account_id: user.id } : selectedAccountId ? { account_id: selectedAccountId } : {}),
  }
  const validInterval = !!from && !!to && from <= to
  const usageQuery = useQuery({
    queryKey: ['organization-usage', session.identity, filters],
    queryFn: () => usageApi.list(filters), enabled: canView && validInterval,
  })

  const invalidateLists = () => {
    for (const queryKey of [['organizations'], ['organization-accounts'], ['organization-usage'], queryKeys.users(), queryKeys.apiKeys()]) {
      void queryClient.invalidateQueries({ queryKey })
    }
  }

  const managementMutation = useMutation({
    mutationFn: async ({ command, session: commandSession }: { command: ManagementCommand; session: AuthSessionSnapshot }) => {
      if (!canManage || !isAuthSessionCurrent(commandSession)) throw new AuthScopeChangedError()
      switch (command.type) {
        case 'create': await organizationsApi.create(command.name); break
        case 'rename': await organizationsApi.rename(command.id, command.name); break
        case 'archive': await organizationsApi.archive(command.id); break
        case 'membership': await organizationsApi.setMembership(command.id, command.membership); break
        case 'disabled': await organizationsApi.setDisabled(command.id, command.disabled); break
      }
    },
    onSuccess: (_, { command, session: commandSession }) => {
      if (!isAuthSessionCurrent(commandSession)) return
      invalidateLists()
      setOperationError(null)
      if (command.type === 'create') setNewOrganizationName('')
      if (command.type === 'membership') setEditingAccount(null)
    },
    onError: (error, { session: commandSession }) => {
      if (isAuthSessionCurrent(commandSession)) setOperationError(error instanceof Error ? error.message : 'The change could not be saved.')
    },
  })
  const manage = (command: ManagementCommand) => {
    if (!canManage || !isAuthSessionCurrent(session) || managementMutation.isPending) return
    setOperationError(null)
    managementMutation.mutate({ command, session })
  }
  const canIssueFor = (account: OrganizationAccount) => canManage && !account.disabled && !!account.organization_id
    && organizations.some((org) => org.id === account.organization_id && !org.archived_at)
  const currentKeyAttempt = (command: KeyCommand) => isAuthSessionCurrent(command.session)
    && command.attempt === keyAttempt.current
    && keyTargetRef.current?.id === command.payload.usage_account_id
    && keyTargetRef.current?.organization_id === command.payload.organization_id
  const keyMutation = useMutation({
    mutationFn: async (command: KeyCommand) => {
      if (!canManage || !currentKeyAttempt(command)) throw new AuthScopeChangedError()
      const data = await organizationsApi.issueKey(command.payload)
      if (currentKeyAttempt(command)) pendingKey.current = { attempt: command.attempt, key: data.key }
      // React Query retains mutation results, so return public metadata only.
      return { id: data.id }
    },
    onSuccess: (_, command) => {
      const completed = pendingKey.current
      if (completed?.attempt === command.attempt) pendingKey.current = null
      if (!currentKeyAttempt(command) || completed?.attempt !== command.attempt) return
      setCreatedKey(completed.key)
      setKeyCopied(false)
      setOperationError(null)
      void queryClient.invalidateQueries({ queryKey: queryKeys.apiKeys() })
    },
    onError: (error, command) => {
      if (pendingKey.current?.attempt === command.attempt) pendingKey.current = null
      if (currentKeyAttempt(command)) setOperationError(error instanceof Error ? error.message : 'The key could not be issued.')
    },
  })

  const clearKey = () => {
    ++keyAttempt.current
    pendingKey.current = null
    keyTargetRef.current = null
    setKeyTarget(null)
    setKeyName('')
    setCreatedKey(null)
    setKeyCopied(false)
    keyMutation.reset()
    cleanupManualCopyBuffer()
  }
  const openKeyForAccount = (account: OrganizationAccount) => {
    if (!canIssueFor(account)) return
    clearKey()
    keyTargetRef.current = account
    setKeyTarget(account)
  }
  const renderedKeyAttempt = keyAttempt.current
  const issueKey = () => {
    if (renderedKeyAttempt !== keyAttempt.current || !keyTarget || !canIssueFor(keyTarget) || !isAuthSessionCurrent(session) || !keyName.trim() || keyMutation.isPending || createdKey) return
    keyMutation.mutate({
      session, attempt: ++keyAttempt.current,
      payload: { name: keyName.trim(), usage_account_id: keyTarget.id, organization_id: keyTarget.organization_id!,
        permissions: ['openai.inference', 'openai.models.read'] },
    })
  }
  const selectOrganization = (id: string | null) => {
    if (!canManage && id !== ownOrganizationId) return
    clearKey()
    setEditingAccount(null)
    setSelectedOrganizationId(id)
    setSelectedAccountId(isMember ? user?.id ?? null : null)
    setRenameName(organizations.find((org) => org.id === id)?.name ?? '')
    setPage(1)
  }
  const selectAccount = (id: string | null) => {
    if (!canView || (isMember && id !== user?.id) || (id !== null && !accounts.some((account) => account.id === id))) return
    clearKey()
    setSelectedAccountId(id)
    setPage(1)
  }
  const editMembership = (account: OrganizationAccount) => {
    if (!canManage) return
    setEditingAccount(account)
    setMembershipOrganizationId(account.organization_id ?? '')
    setMembershipRole(account.organization_role ?? 'member')
  }
  const error = organizationQuery.error ?? accountQuery.error ?? usageQuery.error
  const totalCount = usageQuery.data?.total ?? 0

  return {
    isAuthLoading, canView, canManage,
    scopeLabel: canManage ? 'All organizations' : isMember ? 'Your account' : 'Your organization',
    organizations, accounts, selectedOrganizationId, selectedOrganization, selectOrganization,
    selectedAccount, selectAccount,
    isLoading: organizationQuery.isLoading || accountQuery.isLoading || usageQuery.isLoading,
    isSaving: managementMutation.isPending,
    errorMessage: operationError ?? (!validInterval ? 'Choose a valid UTC date range.' : error instanceof Error ? error.message : null),
    newOrganizationName, setNewOrganizationName,
    createOrganization: () => { if (newOrganizationName.trim()) manage({ type: 'create', name: newOrganizationName.trim() }) },
    renameName, setRenameName,
    renameOrganization: () => { if (selectedOrganizationId && renameName.trim()) manage({ type: 'rename', id: selectedOrganizationId, name: renameName.trim() }) },
    archiveOrganization: () => { if (selectedOrganizationId) manage({ type: 'archive', id: selectedOrganizationId }) },
    editingAccount, membershipOrganizationId, setMembershipOrganizationId,
    membershipRole, setMembershipRole, editMembership,
    saveMembership: () => { if (editingAccount) manage({ type: 'membership', id: editingAccount.id,
      membership: { organization_id: membershipOrganizationId || null, organization_role: membershipRole } }) },
    cancelMembership: () => setEditingAccount(null),
    toggleDisabled: (account) => manage({ type: 'disabled', id: account.id, disabled: !account.disabled }),
    canIssueFor, keyTarget, keyName, setKeyName, createdKey, isIssuingKey: keyMutation.isPending,
    canIssueKey: !!keyTarget && canIssueFor(keyTarget) && !!keyName.trim() && !keyMutation.isPending && !createdKey,
    openKeyForAccount, issueKey, clearKey, keyCopied,
    copyCreatedKey: async () => {
      if (!createdKey || !isAuthSessionCurrent(session)) return
      const commandSession = session
      const attempt = keyAttempt.current
      try {
        const { method } = await copyToClipboard(createdKey)
        if (isAuthSessionCurrent(commandSession) && attempt === keyAttempt.current) {
          if (method === 'manual') selectTextForManualCopy(createdKey)
          setKeyCopied(method !== 'manual')
        }
      } catch {
        if (isAuthSessionCurrent(commandSession) && attempt === keyAttempt.current) setOperationError('Copy failed. Select and copy the key manually.')
      }
    },
    from, to,
    setFrom: (value) => { changeFrom(value); setPage(1) },
    setTo: (value) => { changeTo(value); setPage(1) },
    groupBy: filters.group_by,
    setGroupBy: (value) => { if (canManage) { changeGrouping(value); if (value === 'organization') setSelectedAccountId(null); setPage(1) } },
    usageRows: usageQuery.data?.rows ?? [], hasUsage: !!usageQuery.data, usageTotals: usageQuery.data?.totals ?? emptyTotals,
    coverageStart: usageQuery.data?.coverage_start ?? null,
    currentPage: page, totalPages: Math.max(1, Math.ceil(totalCount / 25)), totalCount,
    changePage: (value) => { if (value > 0 && value <= Math.max(1, Math.ceil(totalCount / 25))) setPage(value) },
    refresh: () => { if (canView) invalidateLists() },
  }
}
