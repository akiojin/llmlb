import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('logs and threshold ViewModel boundaries', () => {
  it.each(['LogViewer', 'AlertThresholdSettings'])('%s delegates state and operations to a JSX-free ViewModel', (name) => {
    const view = readFileSync(`src/components/dashboard/${name}.tsx`, 'utf8')
    expect(view).not.toMatch(/\b(?:useQuery|useMutation|useQueryClient|useState|useEffect|useRef)\b/)
    expect(view).not.toMatch(/\b(?:dashboardApi|clientsApi|queryKeys|toast)\b/)
    expect(view).not.toMatch(/new Date\(/)
    expect(view).toContain(`use${name}ViewModel`)

    const viewModel = readFileSync(`src/viewmodels/use${name}ViewModel.ts`, 'utf8')
    expect(viewModel).toMatch(new RegExp(`export function use${name}ViewModel\\([^]*?\\): ${name}ViewModel`))
    expect(viewModel).not.toMatch(/return\s*\(?\s*<[A-Za-z]/)
  })

  it('declares only the original threshold and ranking keys for manual refresh', () => {
    const source = readFileSync('src/viewmodels/useAlertThresholdSettingsViewModel.ts', 'utf8')
    expect(source).toContain('useInvalidateOn([], queryKeys.alertThreshold())')
    expect(source).toContain('useInvalidateOn([], queryKeys.clientRanking())')
    expect(source).not.toContain('invalidateQueries')
  })
})
