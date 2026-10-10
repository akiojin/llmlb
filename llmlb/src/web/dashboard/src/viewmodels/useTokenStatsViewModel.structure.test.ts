import { existsSync, readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

// SPEC #821 T012d: acquisition and display data belong to a JSX-free ViewModel.
describe('token statistics View boundaries', () => {
  it('delegates acquisition and display formatting while keeping Radix tab lifetime in the View', () => {
    const source = readFileSync('src/components/dashboard/TokenStatsSection.tsx', 'utf8')
    const hooks = [...source.matchAll(/\b(use[A-Z]\w*)\s*(?:<[^>]*>)?\s*\(/g)]
      .map((match) => match[1])
    expect(hooks).toEqual(['useTokenStatsViewModel'])
    expect(source).not.toMatch(/@tanstack\/react-query|queryKeys|\b\w+Api\s*\./)
    expect(source).not.toMatch(/formatNumber|toLocaleString|toFixed|total_input_tokens|total_output_tokens/)
    expect(source).toMatch(/<Tabs\s+defaultValue="daily"/)
    expect(source).toMatch(/<TabsContent\s+value="daily">/)
    expect(source).toMatch(/<TabsContent\s+value="monthly">/)
    expect(source).not.toMatch(/forceMount/)
  })

  it('exposes an explicit presentation contract without JSX, direct transport or new subscriptions', () => {
    const path = 'src/viewmodels/useTokenStatsViewModel.ts'
    expect(existsSync(path)).toBe(true)
    const source = readFileSync(path, 'utf8')
    expect(source).toMatch(/export interface TokenStatsViewModel\b/)
    expect(source).toMatch(/export function useTokenStatsViewModel\(\): TokenStatsViewModel/)
    expect(source).not.toMatch(/react\/jsx-runtime|(?:return|=>)\s*\(?\s*<[A-Za-z>]/)
    expect(source).not.toMatch(/\bfetch\s*\(|WebSocket|useInvalidateOn|useDashboardDataViewModel|useModelEndpointStatsViewModel/)
    expect(source).toMatch(/from ['"]@\/lib\/utils['"]/)
  })
})
