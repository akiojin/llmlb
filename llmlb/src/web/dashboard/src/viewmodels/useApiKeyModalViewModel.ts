import { useEffect, useState, useRef, type RefObject } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { apiKeysApi, type ApiKey, type ApiKeyPermission } from '@/lib/api/api-keys'
import { queryKeys } from '@/lib/queryKeys'
import { useAuth } from '@/hooks/useAuth'
import { AuthScopeChangedError, getAuthSessionSnapshot, isAuthSessionCurrent, type AuthSessionSnapshot } from '@/lib/authSession'
import { copyToClipboard, formatRelativeTime, selectTextForManualCopy, cleanupManualCopyBuffer } from '@/lib/utils'
import { toast } from '@/hooks/use-toast'

interface ApiKeyModalOptions { open: boolean; onOpenChange: (open: boolean) => void }
export interface ApiKeyRow extends ApiKey {
  createdAtLabel: string
  expired: boolean
  expiresLabel: string
  canDelete: boolean
}
export interface ApiKeyModalViewModel {
  isAdmin: boolean
  createOpen: boolean
  deleteKey: ApiKey | null
  setDeleteKey: (key: ApiKey | null) => void
  newKeyName: string
  setNewKeyName: (value: string) => void
  newKeyExpires: string
  setNewKeyExpires: (value: string) => void
  selectedPermissions: ApiKeyPermission[]
  permissionOptions: ApiKeyPermission[]
  createdKey: string | null
  showKey: string | null
  copiedId: string | null
  createdKeyCodeRef: RefObject<HTMLElement | null>
  apiKeys: ApiKeyRow[] | undefined
  errorMessage: string | null
  isLoading: boolean
  isCreating: boolean
  isDeleting: boolean
  canIssue: boolean
  canCreate: boolean
  handleMainOpenChange: (open: boolean) => void
  handleCreateOpenChange: (open: boolean) => void
  handleOpenCreateDialog: () => void
  handleCopy: (text: string, id: string) => Promise<void>
  handleCreate: () => void
  toggleCreatedKey: () => void
  handleRefresh: () => void
  handleDelete: () => void
}

const INFERENCE_PERMISSIONS = ['openai.inference', 'openai.models.read'] as const
interface CreateCommand {
  session: AuthSessionSnapshot
  attempt: number
  issuerId: string
  payload: { name: string; expires_at?: string; permissions?: ApiKeyPermission[]; usage_account_id: string; organization_id: string }
}

/** Scoped public metadata is cached; one-time plaintext remains local to this dialog. */
export function useApiKeyModalViewModel({ open, onOpenChange }: ApiKeyModalOptions): ApiKeyModalViewModel {
  const { user } = useAuth()
  const isAdmin = user?.role === 'admin'
  const canRead = !!user && !user.disabled && !user.must_change_password && (isAdmin || !!user.organization_id)
  const canIssue = !!user?.id && !!user.organization_id && !user.disabled && !user.must_change_password
  const session = getAuthSessionSnapshot()
  const queryClient = useQueryClient()
  const [createOpen, setCreateOpen] = useState(false)
  const [deleteKey, setDeleteKey] = useState<ApiKey | null>(null)
  const [newKeyName, setNewKeyName] = useState('')
  const [newKeyExpires, setNewKeyExpires] = useState('')
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [showKey, setShowKey] = useState<string | null>(null)
  const [copiedId, setCopiedId] = useState<string | null>(null)
  const createdKeyCodeRef = useRef<HTMLElement | null>(null)
  const attempt = useRef(0)
  const pendingSecret = useRef<{ attempt: number; key: string } | null>(null)
  const copyTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const manualTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => () => {
    ++attempt.current
    pendingSecret.current = null
    if (copyTimer.current) clearTimeout(copyTimer.current)
    if (manualTimer.current) clearTimeout(manualTimer.current)
    cleanupManualCopyBuffer()
  }, [])

  const { data: apiKeys, isLoading, refetch, error } = useQuery({
    queryKey: queryKeys.apiKeys(), queryFn: apiKeysApi.list, enabled: open && canRead,
    refetchInterval: false, refetchOnWindowFocus: false,
  })
  const clearCreatedKeyState = () => {
    ++attempt.current
    pendingSecret.current = null
    setCreatedKey(null)
    setShowKey(null)
    setCopiedId(null)
    if (copyTimer.current) clearTimeout(copyTimer.current)
    if (manualTimer.current) clearTimeout(manualTimer.current)
    cleanupManualCopyBuffer()
  }
  const resetCreateForm = () => { setNewKeyName(''); setNewKeyExpires('') }
  const isCurrentCommand = (command: CreateCommand) => isAuthSessionCurrent(command.session) && command.attempt === attempt.current
  const createMutation = useMutation({
    mutationFn: async (command: CreateCommand) => {
      if (!canIssue || !isCurrentCommand(command)) throw new AuthScopeChangedError()
      const { key, ...metadata } = await apiKeysApi.create(command.payload)
      if (isCurrentCommand(command)) pendingSecret.current = { attempt: command.attempt, key }
      // MutationCache also retains results; it must never retain plaintext.
      return { ...metadata, created_by: metadata.created_by ?? command.issuerId,
        usage_account_id: metadata.usage_account_id ?? command.payload.usage_account_id,
        organization_id: metadata.organization_id ?? command.payload.organization_id }
    },
    onSuccess: (data, command) => {
      const secret = pendingSecret.current
      if (secret?.attempt === command.attempt) pendingSecret.current = null
      if (!isCurrentCommand(command) || secret?.attempt !== command.attempt) return
      queryClient.setQueryData(queryKeys.apiKeys(), (old?: ApiKey[]) => [data, ...(Array.isArray(old) ? old : []).filter((key) => key.id !== data.id)])
      setCreatedKey(secret.key)
      setShowKey(null)
      setCopiedId(null)
      resetCreateForm()
      setCreateOpen(false)
      toast({ title: 'API key created' })
    },
    onError: (failure, command) => {
      if (pendingSecret.current?.attempt === command.attempt) pendingSecret.current = null
      if (!isCurrentCommand(command)) return
      toast({ title: 'Failed to create API key', description: failure instanceof Error ? failure.message : 'Unknown error', variant: 'destructive' })
    },
  })
  const canDelete = (key: ApiKey) => canRead && !!user?.id && key.created_by === user.id
  const deleteMutation = useMutation({
    mutationFn: ({ key, session: commandSession }: { key: ApiKey; session: AuthSessionSnapshot }) => {
      if (!isAuthSessionCurrent(commandSession) || !canDelete(key)) throw new AuthScopeChangedError()
      return apiKeysApi.delete(key.id)
    },
    onSuccess: (_, command) => {
      if (!isAuthSessionCurrent(command.session)) return
      void queryClient.invalidateQueries({ queryKey: queryKeys.apiKeys() })
      setDeleteKey(null)
      toast({ title: 'API key deleted' })
    },
    onError: (failure, command) => {
      if (!isAuthSessionCurrent(command.session)) return
      toast({ title: 'Failed to delete API key', description: failure instanceof Error ? failure.message : 'Unknown error', variant: 'destructive' })
    },
  })

  const handleMainOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) { clearCreatedKeyState(); resetCreateForm(); setCreateOpen(false); setDeleteKey(null); createMutation.reset() }
    onOpenChange(nextOpen)
  }
  const handleCreateOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) { clearCreatedKeyState(); resetCreateForm(); createMutation.reset() }
    setCreateOpen(nextOpen && canIssue)
  }
  const handleOpenCreateDialog = () => {
    if (!canIssue || !isAuthSessionCurrent(session)) return
    clearCreatedKeyState()
    resetCreateForm()
    createMutation.reset()
    setCreateOpen(true)
  }
  const selectCreatedKeyForManualCopy = (text: string) => {
    const selection = window.getSelection()
    if (selection && createdKeyCodeRef.current) {
      const range = document.createRange()
      range.selectNodeContents(createdKeyCodeRef.current)
      selection.removeAllRanges()
      selection.addRange(range)
    } else selectTextForManualCopy(text)
  }
  const handleCopy = async (text: string, id: string) => {
    if (!isAuthSessionCurrent(session)) return
    const copyAttempt = attempt.current
    try {
      const { method } = await copyToClipboard(text)
      if (!isAuthSessionCurrent(session) || copyAttempt !== attempt.current) return
      if (method !== 'manual') {
        setCopiedId(id)
        copyTimer.current = setTimeout(() => { if (isAuthSessionCurrent(session) && copyAttempt === attempt.current) setCopiedId(null) }, 2000)
        toast({ title: 'Copied full API key' })
        return
      }
      setCopiedId(null)
      if (id === 'created') {
        setShowKey('created')
        manualTimer.current = setTimeout(() => {
          if (isAuthSessionCurrent(session) && copyAttempt === attempt.current) selectCreatedKeyForManualCopy(text)
        }, 0)
      } else selectTextForManualCopy(text)
      toast({ title: 'Auto copy unavailable', description: 'Press Ctrl+C to copy the selected value.' })
    } catch {
      if (isAuthSessionCurrent(session) && copyAttempt === attempt.current) toast({ title: 'Failed to copy', variant: 'destructive' })
    }
  }
  const handleCreate = () => {
    if (!canIssue || !user?.organization_id || !isAuthSessionCurrent(session) || !newKeyName.trim() || createMutation.isPending) return
    createMutation.mutate({ session, attempt: attempt.current, issuerId: user.id,
      payload: { name: newKeyName.trim(), expires_at: newKeyExpires || undefined,
        ...(isAdmin ? { permissions: [...INFERENCE_PERMISSIONS] } : {}),
        usage_account_id: user.id, organization_id: user.organization_id } })
  }
  const isExpired = (expiresAt: string | null | undefined) => !!expiresAt && new Date(expiresAt) < new Date()

  return {
    isAdmin, createOpen, deleteKey, setDeleteKey, newKeyName, setNewKeyName, newKeyExpires, setNewKeyExpires,
    selectedPermissions: [...INFERENCE_PERMISSIONS], permissionOptions: [...INFERENCE_PERMISSIONS],
    createdKey, showKey, copiedId, createdKeyCodeRef,
    errorMessage: error instanceof Error ? error.message : null,
    isLoading, isCreating: createMutation.isPending, isDeleting: deleteMutation.isPending,
    canIssue, canCreate: canIssue && !!newKeyName.trim() && !createMutation.isPending,
    apiKeys: apiKeys?.map((key) => ({ ...key, canDelete: canDelete(key), createdAtLabel: formatRelativeTime(key.created_at),
      expired: isExpired(key.expires_at), expiresLabel: key.expires_at ? isExpired(key.expires_at) ? 'Expired' : formatRelativeTime(key.expires_at) : 'Never' })),
    handleMainOpenChange, handleCreateOpenChange, handleOpenCreateDialog, handleCopy, handleCreate,
    toggleCreatedKey: () => { if (isAuthSessionCurrent(session)) setShowKey(showKey === 'created' ? null : 'created') },
    handleRefresh: () => { clearCreatedKeyState(); if (canRead && isAuthSessionCurrent(session)) void refetch() },
    handleDelete: () => { if (deleteKey && canDelete(deleteKey) && isAuthSessionCurrent(session)) deleteMutation.mutate({ key: deleteKey, session }) },
  }
}
