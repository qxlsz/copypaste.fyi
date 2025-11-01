import { useState } from 'react'
import type { FormEvent } from 'react'
import { useMutation } from '@tanstack/react-query'
import { toast } from 'sonner'

import { createPaste } from '../api/client'
import type { CreatePastePayload, EncryptionAlgorithm, PasteFormat } from '../api/types'

const formatOptions: Array<{ label: string; value: PasteFormat }> = [
  { label: 'Plain text', value: 'plain_text' },
  { label: 'Markdown', value: 'markdown' },
  { label: 'Code', value: 'code' },
  { label: 'JSON', value: 'json' },
  { label: 'Go', value: 'go' },
  { label: 'C++', value: 'cpp' },
  { label: 'Kotlin', value: 'kotlin' },
  { label: 'Java', value: 'java' },
]

const encryptionOptions: Array<{ label: string; value: EncryptionAlgorithm }> = [
  { label: 'None', value: 'none' },
  { label: 'AES-256-GCM', value: 'aes256_gcm' },
  { label: 'ChaCha20-Poly1305', value: 'chacha20_poly1305' },
  { label: 'XChaCha20-Poly1305', value: 'xchacha20_poly1305' },
]

export const PasteFormPage = () => {
  const [content, setContent] = useState('')
  const [format, setFormat] = useState<PasteFormat>('plain_text')
  const [retentionMinutes, setRetentionMinutes] = useState<number>(0)
  const [encryption, setEncryption] = useState<EncryptionAlgorithm>('none')
  const [encryptionKey, setEncryptionKey] = useState('')
  const [burnAfterReading, setBurnAfterReading] = useState(false)

  const mutation = useMutation({
    mutationFn: async () => {
      const payload: CreatePastePayload = {
        content,
        format,
        retention_minutes: retentionMinutes || undefined,
        burn_after_reading: burnAfterReading || undefined,
      }

      if (encryption !== 'none') {
        payload.encryption = {
          algorithm: encryption,
          key: encryptionKey,
        }
      }

      return createPaste(payload)
    },
    onSuccess: (result) => {
      toast.success('Paste created', {
        description: result.shareableUrl,
      })
      setContent('')
    },
    onError: (error: unknown) => {
      const message = error instanceof Error ? error.message : 'Unknown error'
      toast.error('Failed to create paste', { description: message })
    },
  })

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    mutation.mutate()
  }

  const requiresKey = encryption !== 'none'

  return (
    <div className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_380px]">
      <section className="space-y-6">
        <header className="space-y-2">
          <h1 className="text-3xl font-semibold text-gray-100">Create a secure paste</h1>
          <p className="text-gray-400">
            Encrypt, time-limit, or burn after reading. Your keys never leave the browser.
          </p>
        </header>
        <form className="space-y-6" onSubmit={handleSubmit}>
          <div className="space-y-2">
            <label className="block text-sm font-medium text-gray-300" htmlFor="content">
              Your text
            </label>
            <textarea
              id="content"
              value={content}
              onChange={(event) => setContent(event.target.value)}
              placeholder="Paste or type your content here..."
              className="h-72 w-full rounded-xl border border-slate-700 bg-surface p-4 font-mono text-sm text-gray-100 transition focus:border-primary focus:outline-none focus:ring focus:ring-primary/20"
              required
            />
          </div>

          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="block text-sm font-medium text-gray-300" htmlFor="format">
                Format
              </label>
              <select
                id="format"
                value={format}
                onChange={(event) => setFormat(event.target.value as PasteFormat)}
                className="w-full rounded-lg border border-slate-700 bg-surface px-3 py-2 text-sm text-gray-100 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20"
              >
                {formatOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-2">
              <label className="block text-sm font-medium text-gray-300" htmlFor="retention">
                Retention (minutes)
              </label>
              <input
                id="retention"
                type="number"
                min={0}
                value={retentionMinutes}
                onChange={(event) => setRetentionMinutes(Number(event.target.value))}
                className="w-full rounded-lg border border-slate-700 bg-surface px-3 py-2 text-sm text-gray-100 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20"
              />
            </div>
          </div>

          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <label className="block text-sm font-medium text-gray-300" htmlFor="encryption">
                Encryption
              </label>
              <select
                id="encryption"
                value={encryption}
                onChange={(event) => setEncryption(event.target.value as EncryptionAlgorithm)}
                className="w-full rounded-lg border border-slate-700 bg-surface px-3 py-2 text-sm text-gray-100 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20"
              >
                {encryptionOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-2">
              <label className="block text-sm font-medium text-gray-300" htmlFor="encryptionKey">
                Encryption key {requiresKey && <span className="text-danger">(required)</span>}
              </label>
              <input
                id="encryptionKey"
                type="text"
                value={encryptionKey}
                onChange={(event) => setEncryptionKey(event.target.value)}
                disabled={!requiresKey}
                placeholder="Provide a shared secret or passphrase"
                className="w-full rounded-lg border border-slate-700 bg-surface px-3 py-2 text-sm text-gray-100 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20 disabled:cursor-not-allowed disabled:bg-surface/40"
                required={requiresKey}
              />
            </div>
          </div>

          <label className="inline-flex items-center gap-2 text-sm text-gray-300">
            <input
              type="checkbox"
              checked={burnAfterReading}
              onChange={(event) => setBurnAfterReading(event.target.checked)}
              className="h-4 w-4 rounded border-slate-700 bg-surface text-primary focus:ring-primary/30"
            />
            Burn after first view
          </label>

          <button
            type="submit"
            disabled={mutation.isPending}
            className="inline-flex items-center justify-center rounded-full bg-primary px-5 py-2 text-sm font-semibold text-white shadow-lg shadow-primary/30 transition hover:bg-primary/90 focus:outline-none focus:ring focus:ring-primary/30 disabled:cursor-not-allowed disabled:bg-primary/40"
          >
            {mutation.isPending ? 'Creating…' : 'Create paste'}
          </button>
        </form>
      </section>
      <aside className="space-y-4 rounded-2xl border border-slate-800 bg-surface/80 p-6">
        <h2 className="text-lg font-semibold text-gray-100">Share & status</h2>
        <p className="text-sm text-gray-400">
          After creating a paste, you&apos;ll get a shareable link and QR code. Encryption keys are never sent to the
          server—share them out-of-band.
        </p>
        <div className="rounded-xl border border-slate-700 bg-background/80 p-4">
          <p className="text-sm text-gray-300">
            Toggle <span className="font-medium text-primary">Burn after first view</span> when you need a one-time link.
            Combine with end-to-end encryption for maximum privacy.
          </p>
        </div>
      </aside>
    </div>
  )
}
