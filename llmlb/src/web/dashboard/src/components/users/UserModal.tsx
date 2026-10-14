import { useUserModalViewModel } from '@/viewmodels/useUserModalViewModel'
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
  const {
    registrations, canDemoteEditUser, isAdmin,
    createOpen, editUser, deleteUser, setDeleteUser, generatedPassword, copied,
    formUsername, setFormUsername, formPassword, setFormPassword,
    formRole, setFormRole, formEmail, setFormEmail, users, isLoading,
    isCreating, isUpdating, isDeleting, canCreate, canUpdate,
    handleMainOpenChange, handleCreateOpenChange, handleOpenCreateDialog,
    handleOpenEditDialog, handleCloseEditDialog, handleCreate, handleUpdate,
    handleCopyPassword, handleClosePassword, handlePasswordOpenChange, handleRefresh, handleDelete,
  } = useUserModalViewModel({ open, onOpenChange })

  if (!isAdmin) return null

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

          <div className="min-h-0 min-w-0 max-h-[60vh] space-y-4 overflow-y-auto py-4">
            {/* Actions */}
            <div className="flex justify-between">
              <Button onClick={handleOpenCreateDialog}>
                <Plus className="mr-2 h-4 w-4" />
                Add User
              </Button>
              <Button variant="outline" size="icon" onClick={handleRefresh} aria-label="Refresh users and registrations">
                <RefreshCw className="h-4 w-4" />
              </Button>
            </div>

            <section aria-label="Pending registrations" className="space-y-2">
              <h3 className="text-sm font-semibold">Pending registrations</h3>
              <ScrollArea className={registrations.requests?.length ? 'h-44 rounded-md border' : 'rounded-md border'}>
                {registrations.isLoading ? (
                  <div className="flex h-40 items-center justify-center" role="status" aria-label="Loading registrations">
                    <Loader2 className="h-6 w-6 animate-spin" />
                  </div>
                ) : registrations.error ? (
                  <p role="alert" className="p-4 text-sm text-destructive">{registrations.error}</p>
                ) : !registrations.requests?.length ? (
                  <p className="p-4 text-sm text-muted-foreground">No pending registrations</p>
                ) : (
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Email</TableHead>
                        <TableHead>Requested</TableHead>
                        <TableHead className="text-right">Actions</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {registrations.requests.map((request) => (
                        <TableRow key={request.id} data-testid={`registration-request-${request.id}`}>
                          <TableCell className="break-all font-medium">{request.username}</TableCell>
                          <TableCell className="whitespace-nowrap text-sm text-muted-foreground">{request.createdAtLabel}</TableCell>
                          <TableCell className="text-right">
                            <div className="flex justify-end gap-2">
                              <Button size="sm" aria-label={`Approve ${request.username}`} onClick={() => registrations.handleOpenApprove(request)}>
                                Approve
                              </Button>
                              <Button size="sm" variant="outline" aria-label={`Reject ${request.username}`} onClick={() => registrations.handleOpenReject(request)}>
                                Reject
                              </Button>
                            </div>
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                )}
              </ScrollArea>
            </section>

            <h3 className="text-sm font-semibold">Users</h3>
            {/* Users Table */}
            <ScrollArea className="h-64 rounded-md border">
              {isLoading ? (
                <div className="flex h-full items-center justify-center">
                  <Loader2 className="h-6 w-6 animate-spin" />
                </div>
              ) : !users || users.length === 0 ? (
                <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
                  <Users className="h-8 w-8" />
                  <p>No users</p>
                </div>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Email</TableHead>
                      <TableHead>Role</TableHead>
                      <TableHead>Notification email</TableHead>
                      <TableHead>Created</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {users.map((user) => (
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
                          {user.createdAtLabel}
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
                              disabled={!user.canDelete}
                              title={!user.canDelete ? 'The last administrator cannot be deleted.' : undefined}
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

      <Dialog open={!!registrations.approveRequest} onOpenChange={(nextOpen) => { if (!nextOpen) registrations.handleCloseApprove() }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Approve registration</DialogTitle>
            <DialogDescription>{`Choose the access role for ${registrations.approveRequest?.username}.`}</DialogDescription>
          </DialogHeader>
          <div className="space-y-2 py-4">
            <Label htmlFor="approval-role">Role</Label>
            <Select value={registrations.approvalRole} onValueChange={(role) => registrations.setApprovalRole(role as 'admin' | 'viewer')}>
              <SelectTrigger id="approval-role" aria-label="Approval role">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="viewer">Viewer</SelectItem>
                <SelectItem value="admin">Admin</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-sm text-muted-foreground">Viewers can use inference. Administrators can also manage users and settings.</p>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={registrations.handleCloseApprove}>Cancel</Button>
            <Button onClick={registrations.handleApprove} disabled={registrations.isApproving}>
              {registrations.isApproving && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Approve
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <AlertDialog open={!!registrations.rejectRequest} onOpenChange={(nextOpen) => { if (!nextOpen) registrations.handleCloseReject() }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Reject registration</AlertDialogTitle>
            <AlertDialogDescription>{`Reject the request from ${registrations.rejectRequest?.username}? They can submit a new request later.`}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={registrations.handleReject} disabled={registrations.isRejecting} className="bg-destructive text-destructive-foreground hover:bg-destructive/90">
              {registrations.isRejecting && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Reject
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

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
              <Label htmlFor="create-username">Email</Label>
              <Input
                id="create-username"
                type="email"
                placeholder="user@example.com"
                value={formUsername}
                onChange={(e) => setFormUsername(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="create-role">Role</Label>
              <Select value={formRole} onValueChange={(v) => setFormRole(v as 'admin' | 'viewer')}>
                <SelectTrigger id="create-role">
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
              disabled={!canCreate}
            >
              {isCreating && (
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
        onOpenChange={handlePasswordOpenChange}
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
              onClick={handleClosePassword}
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
              <Label htmlFor="edit-username">Email</Label>
              <Input
                id="edit-username"
                type="email"
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
                <SelectTrigger id="edit-role">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="viewer" disabled={!canDemoteEditUser}>Viewer</SelectItem>
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
              disabled={!canUpdate}
            >
              {isUpdating && (
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
              onClick={handleDelete}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {isDeleting && (
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
