import { existsSync, readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('model download View boundary (#872)', () => {
  it('keeps only the session key and rendering in the View', () => {
    const source = readFileSync('src/components/dashboard/ModelDownloadDialog.tsx', 'utf8')
    expect(source).toMatch(/useModelDownloadDialogViewModel/)
    expect(source).not.toMatch(/@tanstack\/react-query|queryKeys|\b\w+Api\s*\.|\btoast\s*\(/)
    expect(source).not.toMatch(/useState|useEffect|useRef|setInterval|clearInterval|buildProgressMessage|formatEta/)
    expect(source).toContain("key={`${endpoint?.id ?? 'empty'}:${open ? 'open' : 'closed'}`}")
  })

  it('owns query polling and presentation in a JSX-free hook with an empty allowlist', () => {
    const path = 'src/viewmodels/useModelDownloadDialogViewModel.ts'
    expect(existsSync(path)).toBe(true)
    const source = readFileSync(path, 'utf8')
    expect(source).toMatch(/export interface ModelDownloadDialogViewModel/)
    expect(source).toMatch(/useQuery\(/)
    expect(source).toMatch(/refetchInterval:/)
    expect(source).not.toMatch(/setInterval|clearInterval|\bfetch\s*\(|(?:return|=>)\s*\(?\s*<[A-Za-z>]/)
    const allowlist = readFileSync('../../../../scripts/checks/dashboard-data-hooks-allowlist.txt', 'utf8')
    expect(allowlist.split('\n').filter((line) => line.trim() && !line.trim().startsWith('#'))).toEqual([])
  })
})
