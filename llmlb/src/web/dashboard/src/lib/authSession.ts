/** A request or command keeps this snapshot until its final callback runs. */
export interface AuthSessionSnapshot {
  readonly generation: number
  readonly identity: string | null
  readonly scope: string | null
  /** DB account observed through `/auth/me`, independently of its shared scope. */
  readonly accountId?: string | null
}

let current: AuthSessionSnapshot = { generation: 0, identity: null, scope: null, accountId: null }
const invalidationListeners = new Set<(recheck: boolean) => void>()

export class AuthScopeChangedError extends Error {
  constructor() {
    super('The authenticated session or scope changed. Please retry in the current session.')
    this.name = 'AuthScopeChangedError'
  }
}

export function getAuthSessionSnapshot(): AuthSessionSnapshot {
  return current
}

export function isAuthSessionCurrent(snapshot: AuthSessionSnapshot): boolean {
  return snapshot.generation === current.generation
}

/** AuthProvider alone establishes an identity from the current server response. */
export function setAuthSession(identity: string | null, scope: string | null, accountId: string | null = null): AuthSessionSnapshot {
  if (current.identity !== identity || current.scope !== scope || current.accountId !== accountId) {
    current = { generation: current.generation + 1, identity, scope, accountId }
  }
  return current
}

/** Invalidate synchronously, before an old response can reach a cache or callback. */
export function invalidateAuthSession(recheck = false): void {
  current = { generation: current.generation + 1, identity: null, scope: null, accountId: null }
  for (const listener of invalidationListeners) listener(recheck)
}

export function subscribeToAuthSessionInvalidation(listener: (recheck: boolean) => void): () => void {
  invalidationListeners.add(listener)
  return () => { invalidationListeners.delete(listener) }
}
