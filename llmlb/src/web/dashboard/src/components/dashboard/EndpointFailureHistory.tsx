import { useEndpointFailureHistoryViewModel } from '@/viewmodels/useEndpointFailureHistoryViewModel'
import { Button } from '@/components/ui/button'
import { RequestHistoryTable } from './RequestHistoryTable'

export function EndpointFailureHistory({ endpointId }: { endpointId: string }) {
  const { history, page, setPage, totalPages, isLoading, unavailableReason, isEmpty, isEmptyPage, canRetry, retry } = useEndpointFailureHistoryViewModel(endpointId)
  return (
    <section id="endpoint-failures" aria-label="Failed requests" className="min-w-0 space-y-3">
      <p className="text-xs text-muted-foreground">
        Failed requests for this endpoint, newest first. Only retained history is available;
        older records may have been deleted by retention. Cumulative success rates include older requests.
        Open a request to see its recorded error and timestamp.
      </p>
      {unavailableReason ? (
        <div role="status" className="space-y-2">
          <p>{unavailableReason}</p>
          {canRetry && <Button variant="outline" size="sm" onClick={retry}>Retry history</Button>}
        </div>
      ) : isEmpty ? (
        <p role="status">No matching failures in retained history. The cause cannot be determined from the success rate alone.</p>
      ) : isEmptyPage ? (
        <p role="status">No failures on this page. Return to the previous page to see retained records.</p>
      ) : (
        <RequestHistoryTable history={history} isLoading={isLoading} failuresOnly />
      )}
      {!unavailableReason && (totalPages > 1 || page > 1) && (
        <div className="flex flex-wrap items-center justify-between gap-2">
          <Button variant="outline" size="sm" disabled={page <= 1 || isLoading} onClick={() => setPage(page - 1)}>Previous failures</Button>
          <span aria-live="polite">Page {page}{page <= totalPages ? ` / ${totalPages}` : ''}</span>
          <Button variant="outline" size="sm" disabled={page >= totalPages || isLoading} onClick={() => setPage(page + 1)}>Next failures</Button>
        </div>
      )}
    </section>
  )
}
