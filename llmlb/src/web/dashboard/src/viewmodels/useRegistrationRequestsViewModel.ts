import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useAuth } from '@/hooks/useAuth'
import { toast } from '@/hooks/use-toast'
import { registrationRequestsApi, type RegistrationRequest } from '@/lib/api'
import { queryKeys } from '@/lib/queryKeys'
import { formatRelativeTime } from '@/lib/utils'

export interface RegistrationRequestRow extends RegistrationRequest {
  createdAtLabel: string
}

export interface RegistrationRequestsViewModel {
  requests: RegistrationRequestRow[] | undefined
  isLoading: boolean
  error: string | undefined
  approveRequest: RegistrationRequest | null
  rejectRequest: RegistrationRequest | null
  approvalRole: 'admin' | 'viewer'
  setApprovalRole: (role: 'admin' | 'viewer') => void
  isApproving: boolean
  isRejecting: boolean
  handleOpenApprove: (request: RegistrationRequest) => void
  handleCloseApprove: () => void
  handleOpenReject: (request: RegistrationRequest) => void
  handleCloseReject: () => void
  handleApprove: () => void
  handleReject: () => void
  handleRefresh: () => void
}

export interface PendingRegistrationCountViewModel {
  pendingCount: number
}

/** Share the pending list between the admin badge and the management dialog. */
function useRegistrationRequestsQuery(enabled: boolean) {
  const { user } = useAuth()
  return useQuery({
    queryKey: queryKeys.registrationRequests(),
    queryFn: registrationRequestsApi.list,
    enabled: enabled && user?.role === 'admin',
  })
}

export function usePendingRegistrationCountViewModel(enabled: boolean): PendingRegistrationCountViewModel {
  const { user } = useAuth()
  const query = useRegistrationRequestsQuery(enabled)
  return { pendingCount: user?.role === 'admin' ? query.data?.length ?? 0 : 0 }
}

export function useRegistrationRequestsViewModel(open: boolean): RegistrationRequestsViewModel {
  const { user } = useAuth()
  const isAdmin = user?.role === 'admin'
  const queryClient = useQueryClient()
  const query = useRegistrationRequestsQuery(open)
  const [approveRequest, setApproveRequest] = useState<RegistrationRequest | null>(null)
  const [rejectRequest, setRejectRequest] = useState<RegistrationRequest | null>(null)
  const [approvalRole, setApprovalRole] = useState<'admin' | 'viewer'>('viewer')

  const invalidateRequests = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.registrationRequests() })
  }
  const approveMutation = useMutation({
    mutationFn: ({ id, role }: { id: string; role: 'admin' | 'viewer' }) =>
      registrationRequestsApi.approve(id, role),
    onSuccess: () => {
      invalidateRequests()
      void queryClient.invalidateQueries({ queryKey: queryKeys.users() })
      void queryClient.invalidateQueries({ queryKey: queryKeys.notificationSettings() })
      setApproveRequest(null)
      toast({ title: 'Registration approved' })
    },
    onError: (error) => {
      toast({ title: 'Failed to approve registration', description: error.message, variant: 'destructive' })
    },
  })
  const rejectMutation = useMutation({
    mutationFn: (id: string) => registrationRequestsApi.reject(id),
    onSuccess: () => {
      invalidateRequests()
      setRejectRequest(null)
      toast({ title: 'Registration rejected' })
    },
    onError: (error) => {
      toast({ title: 'Failed to reject registration', description: error.message, variant: 'destructive' })
    },
  })

  return {
    requests: isAdmin ? query.data?.map((request) => ({ ...request, createdAtLabel: formatRelativeTime(request.created_at) })) : undefined,
    isLoading: query.isLoading,
    error: query.error?.message,
    approveRequest, rejectRequest, approvalRole, setApprovalRole,
    isApproving: approveMutation.isPending,
    isRejecting: rejectMutation.isPending,
    handleOpenApprove: (request: RegistrationRequest) => {
      if (!isAdmin) return
      setApprovalRole('viewer')
      setApproveRequest(request)
    },
    handleCloseApprove: () => setApproveRequest(null),
    handleOpenReject: (request: RegistrationRequest) => { if (isAdmin) setRejectRequest(request) },
    handleCloseReject: () => setRejectRequest(null),
    handleApprove: () => {
      if (isAdmin && approveRequest && !approveMutation.isPending) {
        approveMutation.mutate({ id: approveRequest.id, role: approvalRole })
      }
    },
    handleReject: () => {
      if (isAdmin && rejectRequest && !rejectMutation.isPending) rejectMutation.mutate(rejectRequest.id)
    },
    handleRefresh: () => { if (isAdmin && open) void query.refetch() },
  }
}
