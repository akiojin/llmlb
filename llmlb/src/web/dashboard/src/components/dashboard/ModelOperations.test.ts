import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

// SPEC #821 T012c: Views render; JSX-free ViewModels own model operations.
describe('model operation View boundaries', () => {
  for (const component of ['ModelAddWizard', 'ModelDeleteDialog']) {
    it(`${component} delegates acquisition, state and commands to its ViewModel`, () => {
      const source = readFileSync(`src/components/dashboard/${component}.tsx`, 'utf8')
      const hooks = [...source.matchAll(/\b(use[A-Z]\w*)\s*(?:<[^>]*>)?\s*\(/g)]
        .map((match) => match[1])
      expect(hooks).toEqual([`use${component}ViewModel`])
      expect(source).not.toMatch(/@tanstack\/react-query|queryKeys|\b\w+Api\s*\./)
      const viewModel = readFileSync(`src/viewmodels/use${component}ViewModel.ts`, 'utf8')
      expect(viewModel).not.toMatch(/react\/jsx-runtime|(?:return|=>)\s*\(?\s*<[A-Za-z>]/)
      expect(viewModel).not.toMatch(/\.invalidateQueries\s*\(/)
    })
  }
})
