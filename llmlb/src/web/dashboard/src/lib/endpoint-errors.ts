export type EndpointErrorKind =
  | 'model_loading'
  | 'timeout'
  | 'connection_error'
  | 'endpoint_request_error'
  | 'unknown'

export interface EndpointErrorDisplay {
  kind: EndpointErrorKind
  label: string
}

function includesAny(source: string, patterns: string[]): boolean {
  return patterns.some((pattern) => source.includes(pattern))
}

export function classifyEndpointLastError(
  lastError?: string | null
): EndpointErrorDisplay | null {
  if (!lastError) {
    return null
  }

  const normalized = lastError.toLowerCase()

  if (includesAny(normalized, ['still loading', 'model_loading', 'cold start'])) {
    return { kind: 'model_loading', label: 'Model loading' }
  }

  if (includesAny(normalized, ['timed out', 'timeout'])) {
    return { kind: 'timeout', label: 'Timeout' }
  }

  if (
    includesAny(normalized, [
      'failed to connect',
      'connection refused',
      'connect error',
      'dns error',
      'tcp connect',
    ])
  ) {
    return { kind: 'connection_error', label: 'Connection' }
  }

  if (includesAny(normalized, ['proxy error', 'endpoint request failed', 'bad gateway'])) {
    return { kind: 'endpoint_request_error', label: 'Proxy' }
  }

  return { kind: 'unknown', label: 'Error' }
}

