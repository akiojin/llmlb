import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('client View boundaries', () => {
  for (const component of ['ClientsTab', 'ClientDrilldown']) {
    it(`${component} only calls its dedicated ViewModel`, () => {
      const source = readFileSync(`src/components/dashboard/${component}.tsx`, 'utf8')
      const hooks = [...source.matchAll(/\b(use[A-Z]\w*)\s*(?:<[^>]*>)?\s*\(/g)]
        .map((match) => match[1])
      expect(hooks).toEqual([`use${component}ViewModel`])
      expect(source).not.toMatch(/@tanstack\/react-query|queryKeys|\b\w+Api\s*\./)
      expect(source).not.toMatch(/toLocaleString|toLocaleTimeString|new Date|window\.location|URLSearchParams/)

      const viewModel = readFileSync(`src/viewmodels/use${component}ViewModel.ts`, 'utf8')
      expect(viewModel).toMatch(new RegExp(`function use${component}ViewModel\\([^)]*\\): ${component}ViewModel`))
      expect(viewModel).not.toMatch(/return\s*\(\s*</)
    })
  }
})
