export type PasteFormat =
  | 'plain_text'
  | 'markdown'
  | 'code'
  | 'json'
  | 'javascript'
  | 'typescript'
  | 'python'
  | 'rust'
  | 'go'
  | 'cpp'
  | 'kotlin'
  | 'java'
  | 'csharp'
  | 'php'
  | 'ruby'
  | 'bash'
  | 'yaml'
  | 'sql'
  | 'swift'
  | 'html'
  | 'css'

export type EncryptionAlgorithm =
  | 'none'
  | 'aes256_gcm'
  | 'chacha20_poly1305'
  | 'xchacha20_poly1305'

export interface CreatePastePayload {
  content: string
  format: PasteFormat
  retention_minutes?: number
  encryption?: {
    algorithm: Exclude<EncryptionAlgorithm, 'none'>
    key: string
  }
  burn_after_reading?: boolean
  bundle?: {
    children: Array<{
      content: string
      format?: PasteFormat
      label?: string
    }>
  }
  time_lock?: {
    not_before?: string
    not_after?: string
  }
  webhook?: {
    url: string
    provider?: 'slack' | 'teams' | 'generic'
    view_template?: string
    burn_template?: string
  }
}

export interface CreatePasteResponse {
  path: string
  shareableUrl: string
}

export interface StatsSummary {
  totalPastes: number
  activePastes: number
  expiredPastes: number
  formats: Array<{
    format: PasteFormat
    count: number
  }>
  encryptionUsage: Array<{
    algorithm: EncryptionAlgorithm
    count: number
  }>
  burnAfterReadingCount: number
  timeLockedCount: number
  createdByDay: Array<{
    date: string
    count: number
  }>
}
