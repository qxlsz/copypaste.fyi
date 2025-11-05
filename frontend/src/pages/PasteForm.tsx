import { useState, useMemo } from 'react'
import type { FormEvent } from 'react'
import { useMutation } from '@tanstack/react-query'
import { toast } from 'sonner'

import { createPaste } from '../api/client'
import type { CreatePastePayload, EncryptionAlgorithm, PasteFormat } from '../api/types'
import { MonacoEditor } from '../components/editor/MonacoEditor'

const formatOptions: Array<{ label: string; value: PasteFormat }> = [
  { label: 'Plain text', value: 'plain_text' },
  { label: 'Markdown', value: 'markdown' },
  { label: 'Generic code', value: 'code' },
  { label: 'JSON', value: 'json' },
  { label: 'JavaScript', value: 'javascript' },
  { label: 'TypeScript', value: 'typescript' },
  { label: 'Python', value: 'python' },
  { label: 'Rust', value: 'rust' },
  { label: 'Go', value: 'go' },
  { label: 'C++', value: 'cpp' },
  { label: 'Kotlin', value: 'kotlin' },
  { label: 'Java', value: 'java' },
  { label: 'C#', value: 'csharp' },
  { label: 'PHP', value: 'php' },
  { label: 'Ruby', value: 'ruby' },
  { label: 'Bash', value: 'bash' },
  { label: 'YAML', value: 'yaml' },
  { label: 'SQL', value: 'sql' },
  { label: 'Swift', value: 'swift' },
  { label: 'HTML', value: 'html' },
  { label: 'CSS', value: 'css' },
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
      const usedEncryption = encryption
      const usedEncryptionKey = encryptionKey
      toast.success('Paste created')
      // Store the encryption settings used for this paste
      setPasteEncryption(usedEncryption)
      setPasteEncryptionKey(usedEncryptionKey)
      setContent('')
      setShareUrl(result.shareableUrl)
      setEncryptionKey('')
      if (usedEncryption !== 'none') {
        setEncryption('none')
      }
    },
    onError: (error: unknown) => {
      const message = error instanceof Error ? error.message : 'Unknown error'
      toast.error('Failed to create paste', { description: message })
    },
  })

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (!content.trim()) {
      toast.error('Content is required')
      return
    }
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
        <form className="space-y-5" onSubmit={handleSubmit}>
          {shareLink && (
            <div className="rounded-2xl border border-primary/40 bg-primary/10 p-4 text-sm text-primary">
              <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
                <span className="font-semibold">Shareable link:</span>
                <a
                  href={shareLink}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="flex-1 break-all rounded-lg bg-slate-900/70 px-3 py-2 text-xs font-semibold text-white underline-offset-2 transition hover:bg-slate-900/80 hover:underline"
                >
                  {shareLink}
                </a>
                <button
                  type="button"
                  onClick={handleCopyShareUrl}
                  className="inline-flex items-center justify-center rounded-full bg-primary p-2 text-white shadow-sm shadow-primary/30 transition hover:bg-primary/90 focus:outline-none focus:ring focus:ring-primary/30"
                  disabled={isCopying}
                >
                  <svg
                    className="h-4 w-4"
                    xmlns="http://www.w3.org/2000/svg"
                    fill="none"
                    viewBox="0 0 24 24"
                    strokeWidth="1.5"
                    stroke="currentColor"
                    aria-hidden="true"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M8 16h8a2 2 0 002-2V6a2 2 0 00-2-2H8a2 2 0 00-2 2v8a2 2 0 002 2z"
                    />
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M16 8h2a2 2 0 012 2v8a2 2 0 01-2 2h-8a2 2 0 01-2-2v-2"
                    />
                  </svg>
                  <span className="sr-only">{isCopying ? 'Copying link…' : 'Copy link'}</span>
                </button>
              </div>
              {pasteEncryption !== 'none' && pasteEncryptionKey && (
                <p className="mt-2 text-xs text-primary/80">
                  Remember to share the encryption key separately: <span className="font-semibold">{pasteEncryptionKey}</span>
                </p>
              )}
            </div>
          )}

          <div className="space-y-2">
            <label className="block text-sm font-medium text-slate-700 dark:text-slate-300" htmlFor="content">
              Your text
            </label>
            <div className="relative">
              <MonacoEditor
                value={content}
                onChange={setContent}
                format={format}
                height="min(75vh, 52rem)"
                className="w-full rounded-2xl border border-slate-200 bg-surface pr-36 text-base transition focus-within:border-primary focus-within:outline-none focus-within:ring focus-within:ring-primary/20 dark:border-slate-700 dark:bg-surface md:min-h-[60vh] min-h-[45vh]"
              />
              <label className="sr-only" htmlFor="format">
                Format
              </label>
              <select
                id="format"
                value={format}
                onChange={(event) => setFormat(event.target.value as PasteFormat)}
                className="absolute top-4 right-4 flex items-center gap-2 rounded-full border border-slate-200 bg-white/90 pl-3 pr-8 py-1 text-xs font-semibold text-slate-600 shadow-sm transition hover:border-primary/60 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20 dark:border-slate-600 dark:bg-slate-900/80 dark:text-slate-200"
              >
                {formatOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="space-y-3">
            <div className="flex flex-col gap-3 lg:flex-row lg:flex-wrap lg:items-end">
              <div className="flex min-w-[220px] flex-col gap-2">
                <div className="flex items-center gap-2">
                  <label className="text-sm font-medium text-slate-700 dark:text-slate-300" htmlFor="retention">
                    Retention
                  </label>
                  <span className="relative inline-flex group">
                    <button
                      type="button"
                      className="cursor-help text-xs text-slate-500 transition hover:text-slate-300"
                      aria-label="Retention info"
                    >
                      ⓘ
                    </button>
                    <span className="pointer-events-none absolute left-1/2 top-full z-10 hidden w-56 -translate-x-1/2 translate-y-1 rounded-md bg-slate-900 px-3 py-2 text-xs text-slate-100 shadow-lg group-hover:block">
                      Paste expires after retention, or instantly after first view if burn after use is enabled.
                    </span>
                  </span>
                </div>
                <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-3">
                  <select
                    id="retention"
                    value={retentionMinutes}
                    onChange={(event) => setRetentionMinutes(Number(event.target.value))}
                    className="rounded-lg border border-slate-200 bg-surface px-3 py-2 text-sm text-slate-900 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20 dark:border-slate-700 dark:bg-surface dark:text-slate-100"
                  >
                    <option value={1}>1 minute</option>
                    <option value={10}>10 minutes</option>
                    <option value={60}>1 hour</option>
                    <option value={180}>3 hours</option>
                    <option value={1440}>1 day</option>
                    <option value={10080}>7 days</option>
                    <option value={43200}>30 days</option>
                  </select>

                  <label className="inline-flex items-center gap-2 text-sm text-slate-700 dark:text-slate-300">
                    <input
                      type="checkbox"
                      checked={burnAfterReading}
                      onChange={(event) => setBurnAfterReading(event.target.checked)}
                      className="h-4 w-4 rounded border-slate-700 bg-surface text-primary focus:ring-primary/30"
                    />
                    <span className="inline-flex items-center gap-1">
                      <span role="img" aria-label="fire">🔥</span>
                      Burn after use
                    </span>
                  </label>
                </div>
              </div>

              <div className="flex min-w-[160px] flex-col gap-1">
                <div className="flex items-center gap-2">
                  <label className="text-sm font-medium text-slate-700 dark:text-slate-300" htmlFor="encryption">
                    Encryption
                  </label>
                  <span className="relative inline-flex group">
                    <button
                      type="button"
                      className="cursor-help text-xs text-slate-500 transition hover:text-slate-300"
                      aria-label="Encryption info"
                    >
                      ⓘ
                    </button>
                    <span className="pointer-events-none absolute left-1/2 top-full z-10 hidden w-56 -translate-x-1/2 translate-y-1 rounded-md bg-slate-900 px-3 py-2 text-xs text-slate-100 shadow-lg group-hover:block">
                      Keys stay client-side—share them out-of-band.
                    </span>
                  </span>
                </div>
                <select
                  id="encryption"
                  value={encryption}
                  onChange={(event) => setEncryption(event.target.value as EncryptionAlgorithm)}
                  className="rounded-lg border border-slate-200 bg-surface px-3 py-2 text-sm text-slate-900 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20 dark:border-slate-700 dark:bg-surface dark:text-slate-100"
                >
                  {encryptionOptions.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </div>

              <div className="flex min-w-[220px] flex-col gap-1 lg:flex-1">
                <label className="text-sm font-medium text-slate-700 dark:text-slate-300" htmlFor="encryptionKey">
                  Encryption key
                </label>
                <div className="relative lg:max-w-md">
                  <input
                    id="encryptionKey"
                    type="text"
                    value={encryptionKey}
                    onChange={(event) => setEncryptionKey(event.target.value)}
                    disabled={!requiresKey}
                    placeholder={requiresKey ? 'Shared secret or passphrase' : 'Encryption disabled'}
                    className="w-full rounded-lg border border-slate-200 bg-surface px-3 py-2 pr-24 text-sm text-slate-900 focus:border-primary focus:outline-none focus:ring focus:ring-primary/20 disabled:cursor-not-allowed disabled:bg-surface/40 dark:border-slate-700 dark:bg-surface dark:text-slate-100"
                    required={requiresKey}
                  />
                  <button
                    type="button"
                    onClick={generatePassphrase}
                    className="absolute inset-y-1 right-1 inline-flex items-center justify-center rounded-md border border-primary/40 bg-primary/10 px-4 text-xs font-semibold text-primary transition hover:bg-primary/20 focus:outline-none focus:ring focus:ring-primary/30"
                  >
                    Generate
                  </button>
                </div>
              </div>

              <div className="flex w-full justify-end lg:w-auto lg:justify-start">
                <button
                  type="submit"
                  className="inline-flex w-full items-center justify-center gap-3 rounded-full bg-primary px-8 py-3 text-sm font-semibold text-white shadow-lg shadow-primary/30 transition hover:bg-primary/90 focus:outline-none focus:ring focus:ring-primary/30 lg:w-auto"
                  disabled={mutation.isPending}
                >
                  {mutation.isPending ? 'Creating…' : 'CopyPaste'}
                </button>
              </div>
            </div>
          </div>
        </form>
      </section>
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
