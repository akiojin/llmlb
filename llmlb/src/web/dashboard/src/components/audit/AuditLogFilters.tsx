import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { AuditLogViewModel } from '@/viewmodels/useAuditLogViewModel'

type AuditLogFiltersProps = Pick<AuditLogViewModel,
  'filters' | 'searchText' | 'changeSearch' | 'changeFilter'
>

export function AuditLogFilters({ filters, searchText, changeSearch, changeFilter }: AuditLogFiltersProps) {

  return (
    <div className="flex flex-wrap gap-3">
      <Input
        placeholder="Search..."
        value={searchText}
        onChange={(e) => changeSearch(e.target.value)}
        className="w-[200px]"
      />
      <Select
        value={filters.actor_type || 'all'}
        onValueChange={(v) => changeFilter('actor_type', v)}
      >
        <SelectTrigger className="w-[130px]">
          <SelectValue placeholder="Actor Type" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="all">All Actors</SelectItem>
          <SelectItem value="user">User</SelectItem>
          <SelectItem value="api_key">API Key</SelectItem>
          <SelectItem value="anonymous">Anonymous</SelectItem>
        </SelectContent>
      </Select>
      <Select
        value={filters.http_method || 'all'}
        onValueChange={(v) => changeFilter('http_method', v)}
      >
        <SelectTrigger className="w-[110px]">
          <SelectValue placeholder="Method" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="all">All Methods</SelectItem>
          <SelectItem value="GET">GET</SelectItem>
          <SelectItem value="POST">POST</SelectItem>
          <SelectItem value="PUT">PUT</SelectItem>
          <SelectItem value="DELETE">DELETE</SelectItem>
          <SelectItem value="PATCH">PATCH</SelectItem>
        </SelectContent>
      </Select>
      <Select
        value={filters.status_code?.toString() || 'all'}
        onValueChange={(v) => changeFilter('status_code', v)}
      >
        <SelectTrigger className="w-[130px]">
          <SelectValue placeholder="Status" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="all">All Status</SelectItem>
          <SelectItem value="200">200 OK</SelectItem>
          <SelectItem value="201">201 Created</SelectItem>
          <SelectItem value="400">400 Bad Request</SelectItem>
          <SelectItem value="401">401 Unauthorized</SelectItem>
          <SelectItem value="403">403 Forbidden</SelectItem>
          <SelectItem value="404">404 Not Found</SelectItem>
          <SelectItem value="500">500 Server Error</SelectItem>
        </SelectContent>
      </Select>
    </div>
  )
}
