import { useMemo, useState, type FormEvent } from 'react'
import { useParams, useSearchParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'

import { fetchPaste } from '../api/viewer'
import type { PasteViewResponse } from '../server/types'
import { MonacoEditor } from '../components/editor/MonacoEditor'

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
    case 'javascript':
      return 'JavaScript'
    case 'typescript':
      return 'TypeScript'
    case 'python':
      return 'Python'
    case 'rust':
      return 'Rust'
    case 'go':
      return 'Go'
    case 'cpp':
      return 'C++'
    case 'kotlin':
      return 'Kotlin'
    case 'java':
      return 'Java'
    case 'csharp':
      return 'C#'
    case 'php':
      return 'PHP'
    case 'ruby':
      return 'Ruby'
    case 'bash':
      return 'Bash'
    case 'yaml':
      return 'YAML'
    case 'sql':
      return 'SQL'
    case 'swift':
      return 'Swift'
    case 'html':
      return 'HTML'
    case 'css':
      return 'CSS'
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
  const [searchParams, setSearchParams] = useSearchParams()
  const key = searchParams.get('key') ?? undefined
  const [enteredKey, setEnteredKey] = useState(() => key ?? '')

  const handleKeySubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const trimmed = enteredKey.trim()
    if (trimmed) {
      const next = new URLSearchParams(searchParams)
      next.set('key', trimmed)
      setSearchParams(next)
    }
  }

  const queryKey = useMemo(() => ['paste', id, key ?? null], [id, key])

  const { data, isLoading, isError, error } = useQuery({
    enabled: Boolean(id),
    retry: false,
    queryKey,
    queryFn: () => fetchPaste(id!, key),
  })

  const stegoDataUrl = useMemo(() => {
    if (!data?.stego) return null
    return `data:${data.stego.carrierMime};base64,${data.stego.carrierImage}`
  }, [data?.stego])

  const editorHeight = useMemo(() => {
    const lines = data?.content?.split('\n') ?? []
    const lineCount = lines.length > 0 ? lines.length : 12
    const clamped = Math.min(Math.max(lineCount, 12), 60)
    return `${clamped * 20}px`
  }, [data?.content])

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
    const isUnauthorized =
      message.includes('401') ||
      message.toLowerCase().includes('unauthorized') ||
      message.toLowerCase().includes('missing key')

    const requiresKey = isUnauthorized

    if (requiresKey) {
      return (
        <div className="mx-auto max-w-md space-y-6">
          <div className="space-y-2 text-center">
            <h1 className="text-2xl font-semibold text-slate-100">Encrypted paste</h1>
            <p className="text-slate-400">This paste requires an encryption key to view.</p>
          </div>

          <form onSubmit={handleKeySubmit} className="space-y-4">
            <div className="space-y-2">
              <label className="block text-sm font-medium text-slate-300" htmlFor="pasteKey">
                Encryption key
              </label>
              <input
                id="pasteKey"
                type="password"
                value={enteredKey}
                onChange={(event) => setEnteredKey(event.target.value)}
                placeholder="Enter the encryption key..."
                className="w-full rounded-lg border border-slate-600 bg-slate-800 px-3 py-2 text-sm text-slate-100 placeholder-slate-500 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20"
                required
                autoFocus
              />
              {key && (
                <p className="text-sm text-danger">
                  The provided key was rejected. Please double-check and try again.
                </p>
              )}
            </div>

            <button
              type="submit"
              className="w-full rounded-lg bg-primary px-4 py-2 text-sm font-semibold text-white shadow-lg shadow-primary/30 transition hover:bg-primary/90 focus:outline-none focus:ring focus:ring-primary/30"
            >
              View paste
            </button>
          </form>

          <div className="text-center text-xs text-slate-500">
            <p>The key was provided when the paste was created.</p>
            <p>If you don't have the key, the paste cannot be viewed.</p>
          </div>
        </div>
      )
    }

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
        <MonacoEditor
          value={data.content}
          format={data.format}
          readOnly
          height={editorHeight}
          className="rounded-xl border border-slate-700"
        />
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
          {data.attestation ? (
            <div>
              <dt className="text-xs uppercase tracking-wide text-slate-500">Attestation</dt>
              <dd className="text-sm text-slate-200">{formatAttestation(data.attestation)}</dd>
            </div>
          ) : null}
          {data.timeLock ? (
            <div>
              <dt className="text-xs uppercase tracking-wide text-slate-500">Time lock</dt>
              <dd className="text-sm text-slate-200">{formatTimeLock(data.timeLock)}</dd>
            </div>
          ) : null}
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

      {data.stego ? (
        <section className="rounded-2xl border border-emerald-700/40 bg-emerald-900/30 p-6">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
            <div className="space-y-2">
              <h2 className="text-lg font-semibold text-emerald-200">Steganographic carrier</h2>
              <p className="text-sm text-emerald-100/70">
                The encrypted payload is embedded in the carrier image below. Share this cover along with the encryption key to allow
                recipients to extract and decrypt the paste locally.
              </p>
              <dl className="mt-3 space-y-2 text-xs text-emerald-100/70">
                <div>
                  <dt className="font-semibold uppercase tracking-wide text-emerald-300">Mime type</dt>
                  <dd className="text-emerald-100">{data.stego.carrierMime}</dd>
                </div>
                <div>
                  <dt className="font-semibold uppercase tracking-wide text-emerald-300">Payload digest (SHA-256)</dt>
                  <dd className="font-mono text-emerald-100 break-all">{data.stego.payloadDigest}</dd>
                </div>
              </dl>
              {stegoDataUrl ? (
                <a
                  href={stegoDataUrl}
                  download={`copypaste-stego-${data.id}.png`}
                  className="inline-flex items-center gap-2 rounded-full bg-emerald-500/80 px-4 py-2 text-sm font-semibold text-emerald-950 shadow-sm shadow-emerald-500/20 transition hover:bg-emerald-400 focus:outline-none focus:ring focus:ring-emerald-400/40"
                >
                  Download carrier image
                </a>
              ) : null}
            </div>
            {stegoDataUrl ? (
              <div className="overflow-hidden rounded-xl border border-emerald-600/40 bg-black/20">
                <img src={stegoDataUrl} alt="Steganographic carrier" className="max-h-64 w-full object-contain" />
              </div>
            ) : null}
          </div>
        </section>
      ) : null}

      <footer className="rounded-xl border border-slate-200 bg-background/80 p-4 text-sm text-slate-600 dark:border-slate-700 dark:bg-background/60 dark:text-slate-300">
        <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
          Crafted by{' '}
          <a
            href="https://x.com/qxlsz"
            target="_blank"
            rel="noopener noreferrer"
            className="font-semibold text-primary underline-offset-2 hover:underline"
          >
            @qxlsz
          </a>{' '}
          © 2025 · copypaste.fyi
        </p>
      </footer>
    </div>
  )
}
