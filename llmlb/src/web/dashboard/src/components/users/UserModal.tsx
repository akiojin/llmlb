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
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import {
  Users,
  Plus,
  Trash2,
  Edit,
  Loader2,
  RefreshCw,
  Shield,
  User as UserIcon,
  Copy,
  Check,
} from 'lucide-react'

const EMAIL_HINT =
  'Destination for operational notifications, which are sent to administrators only. It is not a login identifier: users sign in with their username.'

interface NotificationEmailFieldProps {
  id: string
  value: string
  onChange: (value: string) => void
}

function NotificationEmailField({ id, value, onChange }: NotificationEmailFieldProps) {
  const hintId = `${id}-hint`
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>Notification email (optional)</Label>
      <Input
        id={id}
        type="email"
        autoComplete="off"
        placeholder="ops@example.com"
        aria-describedby={hintId}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
      <p id={hintId} className="text-xs text-muted-foreground">
        {EMAIL_HINT}
      </p>
    </div>
  )
}

interface UserModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function UserModal({ open, onOpenChange }: UserModalProps) {
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
    queryKey: ['users'],
    queryFn: usersApi.list,
    enabled: open,
  })

  // The notification recipients are the admins that have an email, so every
  // user change can alter them.
  const invalidateUsers = () => {
    queryClient.invalidateQueries({ queryKey: ['users'] })
    queryClient.invalidateQueries({ queryKey: ['notification-settings'] })
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

  const getRoleBadge = (role: string) => {
    if (role === 'admin') {
      return (
        <Badge variant="default" className="gap-1">
          <Shield className="h-3 w-3" />
          Admin
        </Badge>
      )
    }
    return (
      <Badge variant="secondary" className="gap-1">
        <UserIcon className="h-3 w-3" />
        Viewer
      </Badge>
    )
  }

  return (
    <>
      <Dialog open={open} onOpenChange={handleMainOpenChange}>
        <DialogContent className="max-w-3xl max-h-[80vh] overflow-hidden">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Users className="h-5 w-5" />
              User Management
            </DialogTitle>
            <DialogDescription>
              Manage dashboard users and their permissions.
            </DialogDescription>
          </DialogHeader>

          <div className="min-w-0 space-y-4 py-4">
            {/* Actions */}
            <div className="flex justify-between">
              <Button onClick={handleOpenCreateDialog}>
                <Plus className="mr-2 h-4 w-4" />
                Add User
              </Button>
              <Button variant="outline" size="icon" onClick={() => refetch()}>
                <RefreshCw className="h-4 w-4" />
              </Button>
            </div>

            {/* Users Table */}
            <ScrollArea className="h-64 rounded-md border">
              {isLoading ? (
                <div className="flex h-full items-center justify-center">
                  <Loader2 className="h-6 w-6 animate-spin" />
                </div>
              ) : !users || (users as User[]).length === 0 ? (
                <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
                  <Users className="h-8 w-8" />
                  <p>No users</p>
                </div>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Username</TableHead>
                      <TableHead>Role</TableHead>
                      <TableHead>Notification email</TableHead>
                      <TableHead>Created</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {(users as User[]).map((user) => (
                      <TableRow key={user.id}>
                        <TableCell className="break-all font-medium">
                          {user.username}
                        </TableCell>
                        <TableCell>{getRoleBadge(user.role)}</TableCell>
                        <TableCell className="break-all text-sm">
                          {user.email ?? (
                            <span className="text-muted-foreground">Not set</span>
                          )}
                        </TableCell>
                        <TableCell className="whitespace-nowrap text-sm text-muted-foreground">
                          {formatRelativeTime(user.created_at)}
                        </TableCell>
                        <TableCell className="text-right">
                          <div className="flex justify-end gap-1">
                            <Button
                              variant="outline"
                              size="icon"
                              className="h-8 w-8"
                              aria-label={`Edit ${user.username}`}
                              onClick={() => handleOpenEditDialog(user)}
                            >
                              <Edit className="h-4 w-4" />
                            </Button>
                            <Button
                              variant="outline"
                              size="icon"
                              className="h-8 w-8"
                              aria-label={`Delete ${user.username}`}
                              onClick={() => setDeleteUser(user)}
                            >
                              <Trash2 className="h-4 w-4 text-destructive" />
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </ScrollArea>
          </div>
        </DialogContent>
      </Dialog>

      {/* Create User Dialog */}
      <Dialog open={createOpen} onOpenChange={handleCreateOpenChange}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create User</DialogTitle>
            <DialogDescription>
              Add a new dashboard user. A password will be automatically generated.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="create-username">Username</Label>
              <Input
                id="create-username"
                placeholder="johndoe"
                value={formUsername}
                onChange={(e) => setFormUsername(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="create-role">Role</Label>
              <Select value={formRole} onValueChange={(v) => setFormRole(v as 'admin' | 'viewer')}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="viewer">Viewer</SelectItem>
                  <SelectItem value="admin">Admin</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <NotificationEmailField
              id="create-email"
              value={formEmail}
              onChange={setFormEmail}
            />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => handleCreateOpenChange(false)}>
              Cancel
            </Button>
            <Button
              onClick={handleCreate}
              disabled={!formUsername || createMutation.isPending}
            >
              {createMutation.isPending && (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              )}
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Generated Password Dialog */}
      <Dialog
        open={!!generatedPassword}
        onOpenChange={(open) => {
          if (!open) {
            setGeneratedPassword(null)
            setCopied(false)
            cleanupManualCopyBuffer()
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>User Created Successfully</DialogTitle>
            <DialogDescription>
              Please save this password. It will only be shown once. The user will be required to change it on first login.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="flex items-center gap-2">
              <code className="flex-1 rounded-md bg-muted px-3 py-2 font-mono text-sm">
                {generatedPassword}
              </code>
              <Button
                variant="outline"
                size="icon"
                onClick={handleCopyPassword}
              >
                {copied ? (
                  <Check className="h-4 w-4 text-green-500" />
                ) : (
                  <Copy className="h-4 w-4" />
                )}
              </Button>
            </div>
          </div>
          <DialogFooter>
            <Button
              onClick={() => {
                setGeneratedPassword(null)
                setCopied(false)
                cleanupManualCopyBuffer()
              }}
            >
              I have saved the password
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Edit User Dialog */}
      <Dialog
        open={!!editUser}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) {
            handleCloseEditDialog()
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Edit User</DialogTitle>
            <DialogDescription>
              {`Update user "${editUser?.username}".`}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="edit-username">Username</Label>
              <Input
                id="edit-username"
                value={formUsername}
                onChange={(e) => setFormUsername(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="edit-password">Password (leave blank to keep)</Label>
              <Input
                id="edit-password"
                type="password"
                placeholder="••••••••"
                value={formPassword}
                onChange={(e) => setFormPassword(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="edit-role">Role</Label>
              <Select value={formRole} onValueChange={(v) => setFormRole(v as 'admin' | 'viewer')}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="viewer">Viewer</SelectItem>
                  <SelectItem value="admin">Admin</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <NotificationEmailField
              id="edit-email"
              value={formEmail}
              onChange={setFormEmail}
            />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={handleCloseEditDialog}>
              Cancel
            </Button>
            <Button
              onClick={handleUpdate}
              disabled={!formUsername || updateMutation.isPending}
            >
              {updateMutation.isPending && (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              )}
              Update
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirmation Dialog */}
      <AlertDialog open={!!deleteUser} onOpenChange={() => setDeleteUser(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete User</AlertDialogTitle>
            <AlertDialogDescription>
              {`Are you sure you want to delete "${deleteUser?.username}"? This action cannot be undone.`}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => deleteUser && deleteMutation.mutate(deleteUser.id)}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {deleteMutation.isPending && (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              )}
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}
