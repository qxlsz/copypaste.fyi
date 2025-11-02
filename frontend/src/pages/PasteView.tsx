import { useMemo } from 'react'
import { useParams, useSearchParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'

import { fetchPaste } from '../api/viewer'
import type { PasteViewResponse } from '../server/types'

const formatLabel = (format: string) => {
  switch (format) {
    case 'plain_text':
      return 'Plain Text'
    case 'markdown':
      return 'Markdown'
    case 'code':
      return 'Code'
    case 'json':
      return 'JSON'
    case 'go':
      return 'Go'
    case 'cpp':
      return 'C++'
    case 'kotlin':
      return 'Kotlin'
    case 'java':
      return 'Java'
    default:
      return format
  }
}

const formatEncryption = (requiresKey: boolean, algorithm: PasteViewResponse['encryption']['algorithm']) => {
  if (!requiresKey) {
    return 'Plaintext'
  }
  switch (algorithm) {
    case 'aes256_gcm':
      return 'AES-256-GCM'
    case 'chacha20_poly1305':
      return 'ChaCha20-Poly1305'
    case 'xchacha20_poly1305':
      return 'XChaCha20-Poly1305'
    default:
      return algorithm
  }
}

const formatTimeLock = (timeLock?: PasteViewResponse['timeLock']) => {
  if (!timeLock) return 'Not configured'
  const parts: string[] = []
  if (timeLock.notBefore) {
    parts.push(`After ${new Date(timeLock.notBefore * 1000).toLocaleString()}`)
  }
  if (timeLock.notAfter) {
    parts.push(`Before ${new Date(timeLock.notAfter * 1000).toLocaleString()}`)
  }
  return parts.length > 0 ? parts.join(' · ') : 'Configured'
}

const formatAttestation = (attestation?: PasteViewResponse['attestation']) => {
  if (!attestation) return 'None'
  if (attestation.kind === 'totp') {
    return attestation.issuer ? `TOTP (${attestation.issuer})` : 'TOTP'
  }
  if (attestation.kind === 'shared_secret') {
    return 'Shared secret'
  }
  return attestation.kind
}

const formatPersistence = (persistence?: PasteViewResponse['persistence']) => {
  if (!persistence) return 'Ephemeral (memory)'
  if (persistence.detail) {
    return `${persistence.kind} · ${persistence.detail}`
  }
  return persistence.kind
}

const formatWebhook = (webhook?: PasteViewResponse['webhook']) => {
  if (!webhook) return 'None'
  switch (webhook.provider) {
    case 'slack':
      return 'Slack'
    case 'teams':
      return 'Microsoft Teams'
    case 'generic':
      return 'Webhook'
    default:
      return 'Webhook'
  }
}

export const PasteViewPage = () => {
  const { id } = useParams<{ id: string }>()
  const [searchParams] = useSearchParams()
  const key = searchParams.get('key') ?? undefined

  const queryKey = useMemo(() => ['paste', id, key], [id, key])

  const { data, isLoading, isError, error } = useQuery({
    enabled: Boolean(id),
    queryKey,
    queryFn: () => fetchPaste(id!, key),
  })

  if (!id) {
    return (
      <div className="mx-auto max-w-3xl space-y-4 text-center">
        <h1 className="text-2xl font-semibold text-slate-100">Paste not found</h1>
        <p className="text-slate-400">The requested paste ID is missing or invalid.</p>
      </div>
    )
  }

  if (isLoading) {
    return <p className="text-slate-400">Loading paste…</p>
  }

  if (isError || !data) {
    const message = error instanceof Error ? error.message : 'Unknown error'
    const isBackendDown = message.includes('timed out') || message.includes('Failed to fetch')
    return (
      <div className="space-y-3">
        <h1 className="text-2xl font-semibold text-danger">
          {isBackendDown ? 'Backend unavailable' : 'Unable to load paste'}
        </h1>
        <p className="text-slate-400">
          {isBackendDown
            ? 'The paste service is currently unavailable. Please try again later or contact support if the issue persists.'
            : message
          }
        </p>
        {isBackendDown && (
          <p className="text-sm text-slate-500">
            Make sure the backend server is running on port 8000.
          </p>
        )}
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <header className="space-y-2">
        <h1 className="text-3xl font-semibold text-slate-100">Shared paste</h1>
        <p className="text-slate-400">
          Format: <span className="font-medium text-primary">{formatLabel(data.format)}</span>
        </p>
        <div className="flex flex-wrap gap-4 text-xs text-slate-500">
          <span>Created: {new Date(data.createdAt * 1000).toLocaleString()}</span>
          {data.expiresAt ? <span>Expires: {new Date(data.expiresAt * 1000).toLocaleString()}</span> : <span>Expires: Never</span>}
          {data.burnAfterReading ? <span className="text-danger font-medium">Burn after reading</span> : null}
        </div>
      </header>

      <section className="rounded-2xl border border-slate-800 bg-surface/80 p-6">
        <pre className="overflow-x-auto whitespace-pre-wrap break-words font-mono text-sm text-slate-100">
          {data.content}
        </pre>
      </section>

      <section className="rounded-2xl border border-slate-800 bg-surface/80 p-6">
        <h2 className="text-lg font-semibold text-slate-100">Paste options</h2>
        <dl className="mt-4 grid gap-3 sm:grid-cols-2">
          <div>
            <dt className="text-xs uppercase tracking-wide text-slate-500">Encryption</dt>
            <dd className="text-sm text-slate-200">
              {formatEncryption(data.encryption.requiresKey, data.encryption.algorithm)}
            </dd>
          </div>
          <div>
            <dt className="text-xs uppercase tracking-wide text-slate-500">Attestation</dt>
            <dd className="text-sm text-slate-200">{formatAttestation(data.attestation)}</dd>
          </div>
          <div>
            <dt className="text-xs uppercase tracking-wide text-slate-500">Time lock</dt>
            <dd className="text-sm text-slate-200">{formatTimeLock(data.timeLock)}</dd>
          </div>
          <div>
            <dt className="text-xs uppercase tracking-wide text-slate-500">Persistence</dt>
            <dd className="text-sm text-slate-200">{formatPersistence(data.persistence)}</dd>
          </div>
          <div>
            <dt className="text-xs uppercase tracking-wide text-slate-500">Webhook</dt>
            <dd className="text-sm text-slate-200">{formatWebhook(data.webhook)}</dd>
          </div>
          {data.bundle?.children?.length ? (
            <div>
              <dt className="text-xs uppercase tracking-wide text-slate-500">Bundle shares</dt>
              <dd className="text-sm text-slate-200">{data.bundle.children.length}</dd>
            </div>
          ) : null}
        </dl>
      </section>
    </div>
  )
}
