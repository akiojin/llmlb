import { useState, useCallback } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useAuth } from '@/hooks/useAuth'
import {
  auditLogApi,
  type AuditLogFilters as FilterType,
  type AuditLogListResponse,
} from '@/lib/api'
import { AuditLogTable } from '@/components/audit/AuditLogTable'
import { AuditLogFilters } from '@/components/audit/AuditLogFilters'
import { HashChainStatus } from '@/components/audit/HashChainStatus'
import { Button } from '@/components/ui/button'
import { ShieldCheck, ChevronLeft, ChevronRight } from 'lucide-react'

interface AuditLogPageProps {
  onBack: () => void
}

export default function AuditLogPage({ onBack }: AuditLogPageProps) {
  const { user } = useAuth()
  const [filters, setFilters] = useState<FilterType>({
    page: 1,
    per_page: 50,
  })

  const { data, isLoading } = useQuery<AuditLogListResponse>({
    queryKey: ['audit-logs', filters],
    queryFn: () => auditLogApi.list(filters),
    enabled: user?.role === 'admin',
  })

  const handleFiltersChange = useCallback((newFilters: FilterType) => {
    setFilters(newFilters)
  }, [])

  const totalPages = data ? Math.ceil(data.total / (filters.per_page || 50)) : 0
  const currentPage = filters.page || 1

  const handlePageChange = useCallback(
    (page: number) => {
      setFilters((prev) => ({ ...prev, page }))
    },
    [],
  )

  if (user?.role !== 'admin') {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-background">
        <div className="text-center">
          <ShieldCheck className="mx-auto h-12 w-12 text-muted-foreground" />
          <h2 className="mt-4 text-lg font-semibold">Access Denied</h2>
          <p className="mt-1 text-sm text-muted-foreground">
            Admin role is required to view audit logs.
          </p>
          <Button variant="link" onClick={onBack} className="mt-4">
            Back to Dashboard
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-background">
      <div className="fixed inset-0 bg-grid opacity-20 pointer-events-none" />

      <header className="sticky top-0 z-40 border-b border-border/50 bg-background/80 backdrop-blur-xl">
        <div className="mx-auto flex h-16 max-w-[1600px] items-center justify-between px-4 sm:px-6 lg:px-8">
          <div className="flex items-center gap-3">
            <Button variant="ghost" size="sm" onClick={onBack}>
              <ChevronLeft className="mr-1 h-4 w-4" />
              Dashboard
            </Button>
            <div className="h-6 w-px bg-border" />
            <div className="flex items-center gap-2">
              <ShieldCheck className="h-5 w-5 text-primary" />
              <h1 className="font-display text-lg font-semibold tracking-tight">
                Audit Log
              </h1>
            </div>
          </div>
          <HashChainStatus />
        </div>
      </header>

      <main className="relative mx-auto max-w-[1600px] px-4 py-6 sm:px-6 lg:px-8">
        <div className="mb-4">
          <AuditLogFilters filters={filters} onFiltersChange={handleFiltersChange} />
        </div>

        <AuditLogTable entries={data?.items || []} loading={isLoading} />

        {totalPages > 1 && (
          <div className="mt-4 flex items-center justify-between">
            <p className="text-sm text-muted-foreground">
              {data?.total.toLocaleString()} entries total
            </p>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                disabled={currentPage <= 1}
                onClick={() => handlePageChange(currentPage - 1)}
              >
                <ChevronLeft className="h-4 w-4" />
                Previous
              </Button>
              <span className="text-sm text-muted-foreground">
                Page {currentPage} / {totalPages}
              </span>
              <Button
                variant="outline"
                size="sm"
                disabled={currentPage >= totalPages}
                onClick={() => handlePageChange(currentPage + 1)}
              >
                Next
                <ChevronRight className="h-4 w-4" />
              </Button>
            </div>
          </div>
        )}
      </main>
    </div>
  )
}
