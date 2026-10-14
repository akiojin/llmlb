import { queryKeys } from '@/lib/queryKeys'
import { useState, useRef, type RefObject } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  apiKeysApi,
  type ApiKey,
  type ApiKeyPermission,
  type CreateApiKeyResponse,
} from '@/lib/api'
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

export interface ApiKeyRow extends ApiKey {
  createdAtLabel: string
  expired: boolean
  expiresLabel: string
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
  'endpoints.read',
  'endpoints.manage',
  'api_keys.manage',
  'users.manage',
  'invitations.manage',
  'models.manage',
  'registry.read',
  'logs.read',
  'metrics.read',
]

/** API-key state and commands; plaintext stays outside the query cache. */
export function useApiKeyModalViewModel({ open, onOpenChange }: ApiKeyModalOptions): ApiKeyModalViewModel {
  const { user } = useAuth()
  const isAdmin = user?.role === 'admin'
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

  // Fetch API keys
  const {
    data: apiKeys,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: queryKeys.apiKeys(),
    queryFn: apiKeysApi.list,
    enabled: open,
    // Plaintext keys are only shown once at creation time. We must not auto-refresh
    // this query while the modal is open, otherwise it becomes unclear whether the
    // key is still "copyable".
    refetchInterval: false,
    refetchOnWindowFocus: false,
  })

  const clearCreatedKeyState = () => {
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
    }
    onOpenChange(nextOpen)
  }

  const handleCreateOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      resetCreateForm()
    }
    setCreateOpen(nextOpen)
  }

  const handleOpenCreateDialog = () => {
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
  const createMutation = useMutation({
    mutationFn: (data: {
      name: string
      expires_at?: string
      permissions?: ApiKeyPermission[]
    }) =>
      apiKeysApi.create(data),
    onSuccess: (data: CreateApiKeyResponse) => {
      // Update list without refetching, so the "created key" stays visible/copyable
      // until the user explicitly refreshes or closes the modal.
      queryClient.setQueryData(queryKeys.apiKeys(), (old?: ApiKey[]) => {
        const next = Array.isArray(old) ? old : []
        const withoutDup = next.filter((k) => k.id !== data.id)
        const created: ApiKey = {
          id: data.id,
          name: data.name,
          key_prefix: data.key_prefix,
          created_at: data.created_at,
          expires_at: data.expires_at,
          permissions: data.permissions,
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
    onError: (error) => {
      toast({
        title: 'Failed to create API key',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Delete API key mutation
  const deleteMutation = useMutation({
    mutationFn: (id: string) => apiKeysApi.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.apiKeys() })
      setDeleteKey(null)
      toast({ title: 'API key deleted' })
    },
    onError: (error) => {
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
    const payload: {
      name: string
      expires_at?: string
      permissions?: ApiKeyPermission[]
    } = {
      name: newKeyName,
      expires_at: newKeyExpires || undefined,
    }

    if (isAdmin) {
      payload.permissions = selectedPermissions
    }

    createMutation.mutate(payload)
  }

  const handleTogglePermission = (permission: ApiKeyPermission, checked: boolean) => {
    if (!isAdmin) return

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
    isAdmin, createOpen, deleteKey, setDeleteKey, newKeyName, setNewKeyName,
    newKeyExpires, setNewKeyExpires, selectedPermissions,
    permissionOptions: ADMIN_PERMISSION_OPTIONS,
    createdKey, showKey, copiedId, createdKeyCodeRef,
    isLoading, isCreating: createMutation.isPending, isDeleting: deleteMutation.isPending,
    canCreate: !!newKeyName && !createMutation.isPending && (!isAdmin || selectedPermissions.length > 0),
    apiKeys: apiKeys?.map((key) => ({ ...key,
      createdAtLabel: formatRelativeTime(key.created_at),
      expired: isExpired(key.expires_at),
      expiresLabel: key.expires_at
        ? isExpired(key.expires_at) ? 'Expired' : formatRelativeTime(key.expires_at)
        : 'Never',
    })),
    handleMainOpenChange, handleCreateOpenChange, handleOpenCreateDialog,
    handleCopy, handleCreate, handleTogglePermission,
    toggleCreatedKey: () => setShowKey(showKey === 'created' ? null : 'created'),
    handleRefresh: () => { clearCreatedKeyState(); void refetch() },
    handleDelete: () => { if (deleteKey) deleteMutation.mutate(deleteKey.id) },
  }
}
