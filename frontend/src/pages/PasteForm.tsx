import { useState, useMemo } from 'react'
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

const PASS_ADJECTIVES = ['stellar', 'quantum', 'radiant', 'luminous', 'hyper', 'galactic', 'neon', 'cosmic', 'orbital', 'sonic']
const PASS_NOUNS = ['otter', 'phoenix', 'nebula', 'flux', 'cipher', 'tachyon', 'comet', 'formula', 'byte', 'matrix']
const PASS_SUFFIXES = ['42', '9000', '1337', '7g', 'mk2', 'ix', 'hyperlane', 'vortex']

export const PasteFormPage = () => {
  const [content, setContent] = useState('')
  const [format, setFormat] = useState<PasteFormat>('plain_text')
  const [retentionMinutes, setRetentionMinutes] = useState<number>(0)
  const [encryption, setEncryption] = useState<EncryptionAlgorithm>('none')
  const [encryptionKey, setEncryptionKey] = useState('')
  const [burnAfterReading, setBurnAfterReading] = useState(false)
  const [shareUrl, setShareUrl] = useState<string | null>(null)
  const [isCopying, setIsCopying] = useState(false)
  const [pasteEncryption, setPasteEncryption] = useState<EncryptionAlgorithm>('none')
  const [pasteEncryptionKey, setPasteEncryptionKey] = useState('')

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
      // Store the encryption settings used for this paste
      setPasteEncryption(encryption)
      setPasteEncryptionKey(encryptionKey)
      setContent('')
      setShareUrl(result.shareableUrl)
    },
    onError: (error: unknown) => {
      const message = error instanceof Error ? error.message : 'Unknown error'
      toast.error('Failed to create paste', { description: message })
    },
  })

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setShareUrl(null)
    mutation.mutate()
  }

  const requiresKey = encryption !== 'none'

  const generatePassphrase = () => {
    const randomElement = <T,>(items: T[]) => items[Math.floor(Math.random() * items.length)]
    const phrase = `${randomElement(PASS_ADJECTIVES)}-${randomElement(PASS_NOUNS)}-${randomElement(PASS_SUFFIXES)}`
    setEncryptionKey(phrase)
    if (encryption === 'none') {
      setEncryption('aes256_gcm')
    }
    toast.message('Geeky passphrase generated', { description: phrase })
  }

  const shareLink = useMemo(() => {
    if (!shareUrl) {
      return null
    }

    try {
      const path = `/p${shareUrl}`
      const url = new URL(path, window.location.origin)
      if (pasteEncryption !== 'none' && pasteEncryptionKey.trim()) {
        url.searchParams.set('key', pasteEncryptionKey)
      }
      if (retentionMinutes && retentionMinutes > 0) {
        url.searchParams.set('ttl', retentionMinutes.toString())
      }
      return url.toString()
    } catch {
      return `/p${shareUrl}`
    }
  }, [shareUrl, pasteEncryption, pasteEncryptionKey, retentionMinutes])

  const handleCopyShareUrl = async () => {
    const urlToCopy = shareLink || shareUrl
    if (!urlToCopy) return
    try {
      setIsCopying(true)
      await navigator.clipboard.writeText(urlToCopy)
      toast.success('Link copied to clipboard')
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error'
      toast.error('Unable to copy link', { description: message })
    } finally {
      setIsCopying(false)
    }
  }

  return (
    <div className="space-y-6">
      <section className="space-y-6">
        <form className="space-y-6" onSubmit={handleSubmit}>

          <div className="space-y-2">
            <label className="block text-sm font-medium text-slate-700 dark:text-slate-300" htmlFor="content">
              Your text
            </label>
            <div className="relative">
              <textarea
                id="content"
                value={content}
                onChange={(event) => setContent(event.target.value)}
                placeholder="Paste or type your content here..."
                className="min-h-[34rem] w-full rounded-2xl border border-slate-200 bg-surface p-5 pr-36 font-mono text-base text-slate-900 transition focus:border-primary focus:outline-none focus:ring focus:ring-primary/20 dark:border-slate-700 dark:bg-surface dark:text-slate-100"
                required
              />
              <label className="sr-only" htmlFor="format">
                Format
              </label>
              <select
                id="format"
                value={format}
                onChange={(event) => setFormat(event.target.value as PasteFormat)}
                className="absolute top-4 right-4 flex items-center gap-2 rounded-full border border-slate-200 bg-white/90 px-3 py-1 text-xs font-semibold text-slate-600 shadow-sm transition hover:border-primary/60 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20 dark:border-slate-600 dark:bg-slate-900/80 dark:text-slate-200"
              >
                {formatOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
            <div className="space-y-3">
              <div className="space-y-2">
                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300" htmlFor="retention">
                  Retention
                </label>
                <select
                  id="retention"
                  value={retentionMinutes}
                  onChange={(event) => setRetentionMinutes(Number(event.target.value))}
                  className="w-full rounded-lg border border-slate-200 bg-surface px-3 py-2 text-sm text-slate-900 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20 dark:border-slate-700 dark:bg-surface dark:text-slate-100"
                >
                  <option value={0}>No expiry</option>
                  <option value={5}>5 minutes</option>
                  <option value={10}>10 minutes</option>
                  <option value={30}>30 minutes</option>
                  <option value={60}>1 hour</option>
                  <option value={1440}>1 day</option>
                  <option value={4320}>3 days</option>
                  <option value={10080}>7 days</option>
                  <option value={20160}>14 days</option>
                  <option value={43200}>30 days</option>
                  <option value={86400}>90 days</option>
                </select>
                <p className="text-xs text-slate-500 dark:text-slate-400">Up to 90 days. "No expiry" requires extra permissions.</p>
              </div>

              <label className="inline-flex items-center gap-2 text-sm text-slate-700 dark:text-slate-300">
                <input
                  type="checkbox"
                  checked={burnAfterReading}
                  onChange={(event) => setBurnAfterReading(event.target.checked)}
                  className="h-4 w-4 rounded border-slate-700 bg-surface text-primary focus:ring-primary/30"
                />
                Burn after first view
              </label>
            </div>

            <div className="space-y-3">
              <div className="space-y-2">
                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300" htmlFor="encryption">
                  Encryption
                </label>
                <select
                  id="encryption"
                  value={encryption}
                  onChange={(event) => setEncryption(event.target.value as EncryptionAlgorithm)}
                  className="w-full rounded-lg border border-slate-200 bg-surface px-3 py-2 text-sm text-slate-900 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20 dark:border-slate-700 dark:bg-surface dark:text-slate-100"
                >
                  {encryptionOptions.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <label className="block text-sm font-medium text-slate-700 dark:text-slate-300" htmlFor="encryptionKey">
                    Encryption key {requiresKey && <span className="text-danger">(required)</span>}
                  </label>
                  <button
                    type="button"
                    onClick={generatePassphrase}
                    className="text-xs font-medium text-primary transition hover:text-primary/80"
                  >
                    Generate passphrase
                  </button>
                </div>
                <input
                  id="encryptionKey"
                  type="text"
                  value={encryptionKey}
                  onChange={(event) => setEncryptionKey(event.target.value)}
                  disabled={!requiresKey}
                  placeholder="Provide a shared secret or passphrase"
                  className="w-full rounded-lg border border-slate-200 bg-surface px-3 py-2 text-sm text-slate-900 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20 disabled:cursor-not-allowed disabled:bg-surface/40 dark:border-slate-700 dark:bg-surface dark:text-slate-100"
                  required={requiresKey}
                />
              </div>
            </div>
          </div>

          <button
            type="submit"
            disabled={mutation.isPending}
            className="inline-flex items-center justify-center rounded-full bg-primary px-5 py-2 text-sm font-semibold text-white shadow-lg shadow-primary/30 transition hover:bg-primary/90 focus:outline-none focus:ring focus:ring-primary/30 disabled:cursor-not-allowed disabled:bg-primary/40"
          >
            {mutation.isPending ? 'Creating…' : 'Create paste'}
          </button>
        </form>
      </section>
      {shareUrl && (
        <aside className="space-y-4">
          <div className="space-y-3 rounded-xl border border-slate-200 bg-background/80 p-4 dark:border-slate-700 dark:bg-background/60">
            
            <p className="text-sm font-semibold text-slate-900 dark:text-slate-100">Shareable link</p>
            {shareLink ? (
              <a
                href={shareLink}
                target="_blank"
                rel="noopener noreferrer"
                className="block break-all rounded-md bg-surface px-3 py-2 text-xs font-medium text-primary underline-offset-2 hover:underline dark:bg-surface/60"
              >
                {shareLink}
              </a>
            ) : (
              <code className="block break-all rounded-md bg-surface px-3 py-2 text-xs text-slate-700 dark:bg-surface/60 dark:text-slate-200">
                {shareUrl}
              </code>
            )}
            <button
              type="button"
              onClick={handleCopyShareUrl}
              disabled={isCopying}
              className="inline-flex items-center justify-center rounded-full border border-primary bg-primary/10 px-4 py-1.5 text-xs font-semibold text-primary transition hover:bg-primary/20 focus:outline-none focus:ring focus:ring-primary/20 disabled:cursor-not-allowed"
            >
              {isCopying ? 'Copying…' : 'Copy link'}
            </button>
          </div>
        </aside>
      )}

      <footer className="rounded-xl border border-slate-200 bg-background/80 p-4 text-sm text-slate-600 dark:border-slate-700 dark:bg-background/60 dark:text-slate-300">
        <p>
          Toggle <span className="font-medium text-primary">Burn after first view</span> when you need a one-time link. Combine with end-to-end encryption for maximum privacy.
        </p>
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
