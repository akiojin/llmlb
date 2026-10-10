import { queryKeys } from '@/lib/queryKeys'
import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { usersApi, type User, type CreateUserResponse } from '@/lib/api'
import {
  copyToClipboard,
  formatRelativeTime,
  selectTextForManualCopy,
  cleanupManualCopyBuffer,
} from '@/lib/utils'
import { toast } from '@/hooks/use-toast'

interface UserModalOptions {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export interface UserRow extends User {
  createdAtLabel: string
}

export interface UserModalViewModel {
  createOpen: boolean
  editUser: User | null
  deleteUser: User | null
  setDeleteUser: (user: User | null) => void
  generatedPassword: string | null
  copied: boolean
  formUsername: string
  setFormUsername: (value: string) => void
  formPassword: string
  setFormPassword: (value: string) => void
  formRole: 'admin' | 'viewer'
  setFormRole: (value: 'admin' | 'viewer') => void
  formEmail: string
  setFormEmail: (value: string) => void
  users: UserRow[] | undefined
  isLoading: boolean
  isCreating: boolean
  isUpdating: boolean
  isDeleting: boolean
  canCreate: boolean
  canUpdate: boolean
  handleMainOpenChange: (open: boolean) => void
  handleCreateOpenChange: (open: boolean) => void
  handleOpenCreateDialog: () => void
  handleOpenEditDialog: (user: User) => void
  handleCloseEditDialog: () => void
  handleCreate: () => void
  handleUpdate: () => void
  handleCopyPassword: () => Promise<void>
  handleClosePassword: () => void
  handlePasswordOpenChange: (open: boolean) => void
  handleRefresh: () => void
  handleDelete: () => void
}

/** User forms, recipient refreshes and one-time generated-password state. */
export function useUserModalViewModel({ open, onOpenChange }: UserModalOptions): UserModalViewModel {
  const queryClient = useQueryClient()
  const [createOpen, setCreateOpen] = useState(false)
  const [editUser, setEditUser] = useState<User | null>(null)
  const [deleteUser, setDeleteUser] = useState<User | null>(null)
  const [generatedPassword, setGeneratedPassword] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  // Form state
  const [formUsername, setFormUsername] = useState('')
  const [formPassword, setFormPassword] = useState('')
  const [formRole, setFormRole] = useState<'admin' | 'viewer'>('viewer')
  const [formEmail, setFormEmail] = useState('')

  // Fetch users
  const { data: users, isLoading, refetch } = useQuery({
    queryKey: queryKeys.users(),
    queryFn: usersApi.list,
    enabled: open,
  })

  // The notification recipients are the admins that have an email, so every
  // user change can alter them.
  const invalidateUsers = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.users() })
    queryClient.invalidateQueries({ queryKey: queryKeys.notificationSettings() })
  }

  // Create user mutation
  const createMutation = useMutation({
    mutationFn: (data: { username: string; role: string; email?: string }) =>
      usersApi.create(data),
    onSuccess: (result: CreateUserResponse) => {
      invalidateUsers()
      resetForm()
      setCreateOpen(false)
      setGeneratedPassword(result.generated_password)
    },
    onError: (error) => {
      toast({
        title: 'Failed to create user',
        description:
          error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Update user mutation
  const updateMutation = useMutation({
    mutationFn: ({
      id,
      data,
    }: {
      id: string
      data: { username?: string; password?: string; role?: string; email?: string }
    }) => usersApi.update(id, data),
    onSuccess: () => {
      invalidateUsers()
      resetForm()
      setEditUser(null)
      toast({ title: 'User updated' })
    },
    onError: (error) => {
      toast({
        title: 'Failed to update user',
        description:
          error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Delete user mutation
  const deleteMutation = useMutation({
    mutationFn: (id: string) => usersApi.delete(id),
    onSuccess: () => {
      invalidateUsers()
      setDeleteUser(null)
      toast({ title: 'User deleted' })
    },
    onError: (error) => {
      toast({
        title: 'Failed to delete user',
        description:
          error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Reset form
  const resetForm = () => {
    setFormUsername('')
    setFormPassword('')
    setFormRole('viewer')
    setFormEmail('')
  }

  useEffect(() => {
    if (!open) {
      cleanupManualCopyBuffer()
    }
  }, [open])

  const handleMainOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      cleanupManualCopyBuffer()
    }
    onOpenChange(nextOpen)
  }

  const handleCreateOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      resetForm()
    }
    setCreateOpen(nextOpen)
  }

  const handleOpenCreateDialog = () => {
    resetForm()
    setCreateOpen(true)
  }

  const handleOpenEditDialog = (user: User) => {
    setFormUsername(user.username)
    setFormPassword('')
    setFormRole(user.role as 'admin' | 'viewer')
    setFormEmail(user.email ?? '')
    setEditUser(user)
  }

  const handleCloseEditDialog = () => {
    setEditUser(null)
    resetForm()
  }

  const handleCreate = () => {
    const email = formEmail.trim()
    createMutation.mutate({
      username: formUsername,
      role: formRole,
      ...(email ? { email } : {}),
    })
  }

  const handleUpdate = () => {
    if (!editUser) return
    const data: { username?: string; password?: string; role?: string; email?: string } = {}
    if (formUsername !== editUser.username) data.username = formUsername
    if (formPassword) data.password = formPassword
    if (formRole !== editUser.role) data.role = formRole
    // An empty string clears the address.
    const email = formEmail.trim()
    if (email !== (editUser.email ?? '')) data.email = email
    updateMutation.mutate({ id: editUser.id, data })
  }

  const handleCopyPassword = async () => {
    if (!generatedPassword) return
    try {
      const { method } = await copyToClipboard(generatedPassword)
      if (method !== 'manual') {
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
        toast({ title: 'Copied to clipboard' })
        return
      }

      setCopied(false)
      selectTextForManualCopy(generatedPassword)
      toast({
        title: 'Auto copy unavailable',
        description: 'Press Ctrl+C to copy the selected value.',
      })
    } catch {
      toast({ title: 'Failed to copy', variant: 'destructive' })
    }
  }

  const handleClosePassword = () => {
    setGeneratedPassword(null)
    setCopied(false)
    cleanupManualCopyBuffer()
  }

  return {
    createOpen, editUser, deleteUser, setDeleteUser, generatedPassword, copied,
    formUsername, setFormUsername, formPassword, setFormPassword,
    formRole, setFormRole, formEmail, setFormEmail, isLoading,
    users: users?.map((user) => ({ ...user, createdAtLabel: formatRelativeTime(user.created_at) })),
    isCreating: createMutation.isPending, isUpdating: updateMutation.isPending,
    isDeleting: deleteMutation.isPending,
    canCreate: !!formUsername && !createMutation.isPending,
    canUpdate: !!formUsername && !updateMutation.isPending,
    handleMainOpenChange, handleCreateOpenChange, handleOpenCreateDialog,
    handleOpenEditDialog, handleCloseEditDialog, handleCreate, handleUpdate,
    handleCopyPassword, handleClosePassword,
    handlePasswordOpenChange: (nextOpen: boolean) => { if (!nextOpen) handleClosePassword() },
    handleRefresh: () => { void refetch() },
    handleDelete: () => { if (deleteUser) deleteMutation.mutate(deleteUser.id) },
  }
}
