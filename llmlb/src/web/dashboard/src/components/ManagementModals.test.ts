import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

// SPEC #821 T012b: Views render; JSX-free ViewModels own data and commands.
describe('management modal View boundaries', () => {
  for (const [directory, component] of [
    ['api-keys', 'ApiKeyModal'],
    ['users', 'UserModal'],
    ['invitations', 'InvitationModal'],
  ]) {
    it(`${component} delegates acquisition, state and commands to its ViewModel`, () => {
      const source = readFileSync(`src/components/${directory}/${component}.tsx`, 'utf8')
      const hooks = [...source.matchAll(/\b(use[A-Z]\w*)\s*(?:<[^>]*>)?\s*\(/g)]
        .map((match) => match[1])
      expect(hooks).toEqual([`use${component}ViewModel`])
      expect(source).not.toMatch(/@tanstack\/react-query|queryKeys|\b\w+Api\s*\./)
      expect(source).not.toMatch(/formatRelativeTime|new Date|toLocaleString|copyToClipboard|cleanupManualCopyBuffer/)
      const viewModel = readFileSync(`src/viewmodels/use${component}ViewModel.ts`, 'utf8')
      expect(viewModel).not.toMatch(/react\/jsx-runtime|(?:return|=>)\s*\(?\s*<[A-Za-z>]/)
    })
  }
})
