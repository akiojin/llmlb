import { useAuditLogViewModel } from '@/viewmodels/useAuditLogViewModel'
import { AuditLogTable } from '@/components/audit/AuditLogTable'
import { AuditLogFilters } from '@/components/audit/AuditLogFilters'
import { HashChainStatus } from '@/components/audit/HashChainStatus'
import { Button } from '@/components/ui/button'
import { TablePagination } from '@/components/ui/table-pagination'
import { ShieldCheck, ChevronLeft } from 'lucide-react'

interface AuditLogPageProps {
  onBack: () => void
}

export default function AuditLogPage({ onBack }: AuditLogPageProps) {
  const viewModel = useAuditLogViewModel()

  if (!viewModel.canView) {
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
          <HashChainStatus verification={viewModel.verification} />
        </div>
      </header>

      <main className="relative mx-auto max-w-[1600px] px-4 py-6 sm:px-6 lg:px-8">
        <div className="mb-4">
          <AuditLogFilters
            filters={viewModel.filters}
            searchText={viewModel.searchText}
            changeSearch={viewModel.changeSearch}
            changeFilter={viewModel.changeFilter}
          />
        </div>

        <AuditLogTable entries={viewModel.entries} loading={viewModel.isLoading} />

        <TablePagination
          currentPage={viewModel.currentPage}
          totalPages={viewModel.totalPages}
          totalCount={viewModel.totalCount}
          onPageChange={viewModel.changePage}
        />
      </main>
    </div>
  )
}
