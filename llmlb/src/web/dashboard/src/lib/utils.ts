import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function formatBytes(bytes: number | null | undefined, decimals = 2): string {
  if (bytes == null || bytes === 0) return '0 Bytes'

  const k = 1024
  const dm = decimals < 0 ? 0 : decimals
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB', 'PB']

  const i = Math.floor(Math.log(bytes) / Math.log(k))

  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`
}

export function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return '-'
  if (ms < 1000) return `${ms.toFixed(0)}ms`
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`
  if (ms < 3600000) return `${(ms / 60000).toFixed(1)}m`
  return `${(ms / 3600000).toFixed(1)}h`
}

export function formatUptime(seconds: number | null | undefined): string {
  if (seconds == null) return '-'
  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)

  if (days > 0) return `${days}d ${hours}h`
  if (hours > 0) return `${hours}h ${minutes}m`
  return `${minutes}m`
}

export function formatNumber(num: number | null | undefined): string {
  if (num == null) return '-'
  if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`
  if (num >= 1000) return `${(num / 1000).toFixed(1)}K`
  return num.toLocaleString()
}

export function formatFullNumber(num: number | null | undefined): string {
  if (num == null) return '-'
  return num.toLocaleString('ja-JP')
}

export function formatPercentage(value: number | null | undefined, decimals = 1): string {
  if (value == null) return '-'
  return `${value.toFixed(decimals)}%`
}

export function formatDate(date: Date | string): string {
  const d = typeof date === 'string' ? new Date(date) : date
  return d.toLocaleDateString('ja-JP', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

export function formatRelativeTime(date: Date | string | null | undefined): string {
  if (date == null) return '-'
  const d = typeof date === 'string' ? new Date(date) : date
  if (isNaN(d.getTime())) return '-'
  const now = new Date()
  const diff = now.getTime() - d.getTime()
  const seconds = Math.floor(diff / 1000)

  if (seconds < 60) return 'just now'
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`
  if (seconds < 604800) return `${Math.floor(seconds / 86400)}d ago`
  return formatDate(d)
}

export function truncate(str: string, length: number): string {
  if (str.length <= length) return str
  return `${str.slice(0, length)}...`
}

export function debounce<T extends (...args: unknown[]) => unknown>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout>
  return (...args: Parameters<T>) => {
    clearTimeout(timeoutId)
    timeoutId = setTimeout(() => fn(...args), delay)
  }
}

export function generateId(): string {
  return Math.random().toString(36).substring(2, 9)
}

export function copyToClipboard(text: string): Promise<void> {
  if (typeof text !== 'string' || text.length === 0) {
    return Promise.reject(new Error('Clipboard value is empty'))
  }

  const fallbackCopy = async (): Promise<void> => {
    return fallbackCopyToClipboard(text)
  }

  if (
    typeof navigator === 'undefined' ||
    !navigator.clipboard?.writeText ||
    (typeof window !== 'undefined' && !window.isSecureContext)
  ) {
    return fallbackCopy()
  }

  return navigator.clipboard.writeText(text).catch(() => fallbackCopy())
}

function fallbackCopyToClipboard(text: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const textarea = document.createElement('textarea')
    const activeElement = document.activeElement as HTMLElement | null
    const selection = window.getSelection()
    const activeRange = selection?.rangeCount ? selection.getRangeAt(0).cloneRange() : null

    textarea.value = text
    textarea.setAttribute('readonly', '')
    textarea.style.position = 'fixed'
    textarea.style.top = '0'
    textarea.style.left = '0'
    textarea.style.width = '1px'
    textarea.style.height = '1px'
    textarea.style.opacity = '0'
    textarea.style.pointerEvents = 'none'
    textarea.style.zIndex = '-1'

    const cleanup = () => {
      if (textarea.parentElement) {
        document.body.removeChild(textarea)
      }

      if (selection) {
        selection.removeAllRanges()
        if (activeRange) {
          selection.addRange(activeRange)
        }
      }

      if (activeElement?.isConnected) {
        activeElement.focus()
      }
    }

    document.body.appendChild(textarea)
    textarea.focus()
    textarea.select()
    textarea.setSelectionRange(0, textarea.value.length)

    try {
      const success = document.execCommand('copy')
      cleanup()
      if (success) {
        resolve()
      } else {
        reject(new Error('Fallback copy failed'))
      }
    } catch (error) {
      cleanup()
      reject(error)
    }
  })
}
