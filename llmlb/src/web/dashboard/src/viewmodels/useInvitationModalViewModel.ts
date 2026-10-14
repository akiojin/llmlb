import { queryKeys } from '@/lib/queryKeys'
import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { invitationsApi, type Invitation, type CreateInvitationResponse } from '@/lib/api'
import {
  copyToClipboard,
  formatRelativeTime,
  selectTextForManualCopy,
  cleanupManualCopyBuffer,
} from '@/lib/utils'
import { toast } from '@/hooks/use-toast'

interface InvitationModalOptions {
  open: boolean
}

export interface InvitationRow extends Invitation {
  idLabel: string
  displayStatus: Invitation['status'] | 'expired'
  createdAtLabel: string
  expiresAtLabel: string
  usedByLabel: string
  canRevoke: boolean
}

export interface InvitationModalViewModel {
  createOpen: boolean
  setCreateOpen: (open: boolean) => void
  expiresInHours: number
  setExpiresInHours: (hours: number) => void
  revokeInvitation: Invitation | null
  setRevokeInvitation: (invitation: Invitation | null) => void
  createdCode: CreateInvitationResponse | null
  copied: boolean
  invitations: InvitationRow[] | undefined
  isLoading: boolean
  isCreating: boolean
  isRevoking: boolean
  createdCodeExpiresLabel: string
  handleCreate: () => void
  handleCopy: () => Promise<void>
  handleCloseCreatedDialog: () => void
  handleRefresh: () => void
  handleRevoke: () => void
}

/** Invitation acquisition, expiry values, code lifetime and commands. */
export function useInvitationModalViewModel({ open }: InvitationModalOptions): InvitationModalViewModel {
  const queryClient = useQueryClient()
  const [createOpen, setCreateOpen] = useState(false)
  const [expiresInHours, setExpiresInHours] = useState<number>(72)
  const [revokeInvitation, setRevokeInvitation] = useState<Invitation | null>(null)
  const [createdCode, setCreatedCode] = useState<CreateInvitationResponse | null>(null)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (!open) {
      cleanupManualCopyBuffer()
    }
  }, [open])

  // Fetch invitations
  const { data: invitations, isLoading, refetch } = useQuery({
    queryKey: queryKeys.invitations(),
    queryFn: invitationsApi.list,
    enabled: open,
  })

  // Create invitation mutation
  const createMutation = useMutation({
    mutationFn: (hours: number) => invitationsApi.create(hours),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.invitations() })
      setCreatedCode(data)
      toast({ title: 'Invitation code created' })
    },
    onError: (error) => {
      toast({
        title: 'Failed to create invitation code',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  // Revoke invitation mutation
  const revokeMutation = useMutation({
    mutationFn: (id: string) => invitationsApi.revoke(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.invitations() })
      setRevokeInvitation(null)
      toast({ title: 'Invitation code revoked' })
    },
    onError: (error) => {
      toast({
        title: 'Failed to revoke invitation code',
        description: error instanceof Error ? error.message : 'Unknown error',
        variant: 'destructive',
      })
    },
  })

  const handleCreate = () => {
    createMutation.mutate(expiresInHours)
  }

  const handleCopy = async () => {
    if (createdCode?.code) {
      try {
        const { method } = await copyToClipboard(createdCode.code)
        if (method !== 'manual') {
          setCopied(true)
          setTimeout(() => setCopied(false), 2000)
          toast({ title: 'Copied to clipboard' })
          return
        }

        setCopied(false)
        selectTextForManualCopy(createdCode.code)
        toast({
          title: 'Auto copy unavailable',
          description: 'Press Ctrl+C to copy the selected value.',
        })
      } catch {
        toast({ title: 'Failed to copy', variant: 'destructive' })
      }
    }
  }

  const handleCloseCreatedDialog = () => {
    setCreatedCode(null)
    setCreateOpen(false)
    setCopied(false)
    cleanupManualCopyBuffer()
  }

  const isExpired = (expiresAt: string) => {
    return new Date(expiresAt) < new Date()
  }

  return {
    createOpen, setCreateOpen, expiresInHours, setExpiresInHours,
    revokeInvitation, setRevokeInvitation, createdCode, copied, isLoading,
    isCreating: createMutation.isPending, isRevoking: revokeMutation.isPending,
    createdCodeExpiresLabel: createdCode ? new Date(createdCode.expires_at).toLocaleString() : '',
    invitations: invitations?.map((invitation): InvitationRow => {
      const expired = isExpired(invitation.expires_at)
      return { ...invitation,
        idLabel: `${invitation.id.slice(0, 8)}...`,
        displayStatus: invitation.status === 'active' && expired ? 'expired' : invitation.status,
        createdAtLabel: formatRelativeTime(invitation.created_at),
        expiresAtLabel: formatRelativeTime(invitation.expires_at),
        usedByLabel: invitation.used_by ? `${invitation.used_by.slice(0, 8)}...` : '-',
        canRevoke: invitation.status === 'active' && !expired,
      }
    }),
    handleCreate, handleCopy, handleCloseCreatedDialog,
    handleRefresh: () => { void refetch() },
    handleRevoke: () => { if (revokeInvitation) revokeMutation.mutate(revokeInvitation.id) },
  }
}
