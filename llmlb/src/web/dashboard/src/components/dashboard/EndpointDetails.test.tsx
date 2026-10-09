import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

// SPEC #821 T012a: data ownership and presentation derivation belong to JSX-free VMs.
describe('endpoint detail View boundaries', () => {
  for (const [component, viewModel] of [
    ['EndpointDetailModal', 'useEndpointDetailViewModel'],
    ['EndpointModelsTable', 'useEndpointModelsTableViewModel'],
    ['EndpointRequestChart', 'useEndpointRequestChartViewModel'],
  ]) {
    it(`${component} renders its ViewModel without acquiring or deriving data`, () => {
      const source = readFileSync(`src/components/dashboard/${component}.tsx`, 'utf8')
      const hooks = [...source.matchAll(/\b(use[A-Z]\w*)\s*(?:<[^>]*>)?\s*\(/g)]
        .map((match) => match[1])
      expect(hooks).toEqual([viewModel])
      expect(source).not.toMatch(/\b\w+Api\s*\.\s*\w+\s*\(/)
      expect(source).not.toMatch(/formatRelativeTime|toLocaleString|toFixed|parseInt/)
      expect(source).not.toMatch(/queryKeys|classifyEndpointLastError/)
      expect(source).not.toMatch(/\$\{(?:endpoint\.model_count|consolidatedRows\.length)\}/)
    })
  }
})
