import { queryKeys } from '@/lib/queryKeys'
import { useState, useRef, type RefObject } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  apiKeysApi,
  type ApiKey,
  type ApiKeyPermission,
  type CreateApiKeyResponse,
} from '@/lib/api'
import { getSessionGeneration } from '@/lib/api/client'
import { useAuth } from '@/hooks/useAuth'
import {
  copyToClipboard,
  formatRelativeTime,
  selectTextForManualCopy,
  cleanupManualCopyBuffer,
} from '@/lib/utils'
import { toast } from '@/hooks/use-toast'

interface ApiKeyModalOptions {
  open: boolean
  onOpenChange: (open: boolean) => void
}

interface SessionCommand<T> {
  generation: number
  payload: T
}

type CreateApiKeyInput = Parameters<typeof apiKeysApi.create>[0]

interface CreateKeyCommand extends SessionCommand<CreateApiKeyInput> {
  attempt: number
}

export interface ApiKeyRow extends ApiKey {
  canDelete: boolean
  createdAtLabel: string
  expired: boolean
  expiresLabel: string
}

export interface ApiKeyModalViewModel {
  isAdmin: boolean
  canIssueSelfKey: boolean
  issuanceHint: string | null
  canDeleteSelectedKey: boolean
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
  isLoading: boolean
  isCreating: boolean
  isDeleting: boolean
  canCreate: boolean
  handleMainOpenChange: (open: boolean) => void
  handleCreateOpenChange: (open: boolean) => void
  handleOpenCreateDialog: () => void
  handleCopy: (text: string, id: string) => Promise<void>
  handleCreate: () => void
  handleTogglePermission: (permission: ApiKeyPermission, checked: boolean) => void
  toggleCreatedKey: () => void
  handleRefresh: () => void
  handleDelete: () => void
}

const VIEWER_FIXED_PERMISSIONS: ApiKeyPermission[] = [
  'openai.inference',
  'openai.models.read',
]

const ADMIN_PERMISSION_OPTIONS: ApiKeyPermission[] = [
  'openai.inference',
  'openai.models.read',
]

/** API-key state and commands; plaintext stays outside the query cache. */
export function useApiKeyModalViewModel({ open, onOpenChange }: ApiKeyModalOptions): ApiKeyModalViewModel {
  const { user } = useAuth()
  const isAdmin = user?.role === 'admin'
  const canReadKeys = !!user && (isAdmin || user.scope === 'organization' || user.scope === 'account')
  const canIssueSelfKey = canReadKeys && !!user?.organization_id
  const issuanceHint = canIssueSelfKey ? null : isAdmin
    ? 'Use Organizations to issue a key for an account, or assign your account to an active organization before creating a self key.'
    : 'An active organization membership is required to manage API keys.'
  const canDeleteKey = (key: ApiKey | null) => canReadKeys && !!user?.id && key?.created_by === user.id
  const queryClient = useQueryClient()
  const [createOpen, setCreateOpen] = useState(false)
  const [deleteKey, setDeleteKey] = useState<ApiKey | null>(null)
  const [newKeyName, setNewKeyName] = useState('')
  const [newKeyExpires, setNewKeyExpires] = useState('')
  const [selectedPermissions, setSelectedPermissions] = useState<ApiKeyPermission[]>(
    VIEWER_FIXED_PERMISSIONS
  )
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [showKey, setShowKey] = useState<string | null>(null)
  const [copiedId, setCopiedId] = useState<string | null>(null)
  const createdKeyCodeRef = useRef<HTMLElement | null>(null)
  const createAttempt = useRef(0)

  // Fetch API keys
  const {
    data: apiKeys,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: queryKeys.apiKeys(),
    queryFn: apiKeysApi.list,
    enabled: open && canReadKeys,
    // Plaintext keys are only shown once at creation time. We must not auto-refresh
    // this query while the modal is open, otherwise it becomes unclear whether the
    // key is still "copyable".
    refetchInterval: false,
    refetchOnWindowFocus: false,
  })

  const clearCreatedKeyState = () => {
    createAttempt.current++
    createMutation.reset()
    setCreatedKey(null)
    setShowKey(null)
    setCopiedId(null)
    cleanupManualCopyBuffer()
  }

  const resetCreateForm = () => {
    setNewKeyName('')
    setNewKeyExpires('')
    setSelectedPermissions(VIEWER_FIXED_PERMISSIONS)
  }

  const handleMainOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      clearCreatedKeyState()
      resetCreateForm()
      setCreateOpen(false)
    }
    onOpenChange(nextOpen)
  }

  const handleCreateOpenChange = (nextOpen: boolean) => {
    if (nextOpen && !canIssueSelfKey) return
    if (!nextOpen) {
      clearCreatedKeyState()
      resetCreateForm()
    }
    setCreateOpen(nextOpen)
  }

  const handleOpenCreateDialog = () => {
    if (!canIssueSelfKey) return
    clearCreatedKeyState()
    resetCreateForm()
    setCreateOpen(true)
  }

  const selectCreatedKeyForManualCopy = (text: string) => {
    if (typeof window === 'undefined' || typeof document === 'undefined') {
      return
    }

    const selection = window.getSelection()
    if (selection && createdKeyCodeRef.current) {
      const range = document.createRange()
      range.selectNodeContents(createdKeyCodeRef.current)
      selection.removeAllRanges()
      selection.addRange(range)
      return
    }

    selectTextForManualCopy(text)
  }

  // Create API key mutation
  const isCurrentCreateCommand = (command: CreateKeyCommand) =>
    command.generation === getSessionGeneration() && command.attempt === createAttempt.current
  const createMutation = useMutation({
    mutationFn: (command: CreateKeyCommand) => {
      if (!isCurrentCreateCommand(command)) {
        throw new Error('API key create command is no longer current')
      }
      return apiKeysApi.create(command.payload)
    },
    onSuccess: (data: CreateApiKeyResponse, command) => {
      if (!isCurrentCreateCommand(command)) return
      // Update list without refetching, so the "created key" stays visible/copyable
      // until the user explicitly refreshes or closes the modal.
      queryClient.setQueryData(queryKeys.apiKeys(), (old?: ApiKey[]) => {
        const next = Array.isArray(old) ? old : []
        const withoutDup = next.filter((k) => k.id !== data.id)
        const created: ApiKey = {
          id: data.id,
          name: data.name,
          created_by: user?.id,
          key_prefix: data.key_prefix,
          created_at: data.created_at,
          expires_at: data.expires_at,
          permissions: data.permissions,
          usage_account_id: data.usage_account_id,
          organization_id: data.organization_id,
        }
        return [created, ...withoutDup]
      })

      setCreatedKey(data.key)
      setShowKey(null)
      setCopiedId(null)
      resetCreateForm()
      setCreateOpen(false)
      toast({ title: 'API key created' })
    },
    onError: (error, command) => {
      if (!isCurrentCreateCommand(command)) return
      toast({
        title: 'Failed to create API key',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Delete API key mutation
  const deleteMutation = useMutation({
    mutationFn: (command: SessionCommand<string>) => {
      if (command.generation !== getSessionGeneration()) {
        throw new Error('Session changed before API key command')
      }
      return apiKeysApi.delete(command.payload)
    },
    onSuccess: (_data, command) => {
      if (command.generation !== getSessionGeneration()) return
      queryClient.invalidateQueries({ queryKey: queryKeys.apiKeys() })
      setDeleteKey(null)
      toast({ title: 'API key deleted' })
    },
    onError: (error, command) => {
      if (command.generation !== getSessionGeneration()) return
      toast({
        title: 'Failed to delete API key',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  const handleCopy = async (text: string, id: string) => {
    try {
      const { method } = await copyToClipboard(text)
      if (method !== 'manual') {
        setCopiedId(id)
        setTimeout(() => setCopiedId(null), 2000)
        toast({ title: 'Copied full API key' })
        return
      }

      setCopiedId(null)
      if (id === 'created') {
        setShowKey('created')
        window.setTimeout(() => selectCreatedKeyForManualCopy(text), 0)
      } else {
        selectTextForManualCopy(text)
      }
      toast({
        title: 'Auto copy unavailable',
        description: 'Press Ctrl+C to copy the selected value.',
      })
    } catch {
      toast({ title: 'Failed to copy', variant: 'destructive' })
    }
  }

  const handleCreate = () => {
    if (!canIssueSelfKey) return
    const payload: CreateApiKeyInput = {
      name: newKeyName,
      expires_at: newKeyExpires || undefined,
    }

    if (isAdmin) {
      payload.permissions = selectedPermissions
    }

    createMutation.mutate({ generation: getSessionGeneration(), attempt: ++createAttempt.current, payload })
  }

  const handleTogglePermission = (permission: ApiKeyPermission, checked: boolean) => {
    if (!isAdmin || !ADMIN_PERMISSION_OPTIONS.includes(permission)) return

    setSelectedPermissions((prev) => {
      if (checked) {
        if (prev.includes(permission)) return prev
        return [...prev, permission]
      }
      return prev.filter((p) => p !== permission)
    })
  }

  const isExpired = (expiresAt: string | null | undefined) => {
    if (!expiresAt) return false
    return new Date(expiresAt) < new Date()
  }

  return {
    isAdmin, canIssueSelfKey, issuanceHint, canDeleteSelectedKey: canDeleteKey(deleteKey),
    createOpen, deleteKey, setDeleteKey, newKeyName, setNewKeyName,
    newKeyExpires, setNewKeyExpires, selectedPermissions,
    permissionOptions: ADMIN_PERMISSION_OPTIONS,
    createdKey, showKey, copiedId, createdKeyCodeRef,
    isLoading, isCreating: createMutation.isPending, isDeleting: deleteMutation.isPending,
    canCreate: canIssueSelfKey && !!newKeyName && !createMutation.isPending && (!isAdmin || selectedPermissions.length > 0),
    apiKeys: canReadKeys ? apiKeys?.map((key) => ({ ...key,
      canDelete: canDeleteKey(key),
      createdAtLabel: formatRelativeTime(key.created_at),
      expired: isExpired(key.expires_at),
      expiresLabel: key.expires_at
        ? isExpired(key.expires_at) ? 'Expired' : formatRelativeTime(key.expires_at)
        : 'Never',
    })) : undefined,
    handleMainOpenChange, handleCreateOpenChange, handleOpenCreateDialog,
    handleCopy, handleCreate, handleTogglePermission,
    toggleCreatedKey: () => setShowKey(showKey === 'created' ? null : 'created'),
    handleRefresh: () => { clearCreatedKeyState(); if (canReadKeys) void refetch() },
    handleDelete: () => {
      if (deleteKey && canDeleteKey(deleteKey)) {
        deleteMutation.mutate({ generation: getSessionGeneration(), payload: deleteKey.id })
      }
    },
  }
}
