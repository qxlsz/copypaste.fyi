export interface PasteViewResponse {
  id: string
  format:
    | 'plain_text'
    | 'markdown'
    | 'code'
    | 'json'
    | 'go'
    | 'cpp'
    | 'kotlin'
    | 'java'
  content: string
  createdAt: number
  expiresAt?: number | null
  burnAfterReading: boolean
  bundle?: {
    children: Array<{
      id: string
      label?: string | null
    }>
  } | null
  encryption: {
    algorithm: 'none' | 'aes256_gcm' | 'chacha20_poly1305' | 'xchacha20_poly1305'
    requiresKey: boolean
  }
  timeLock?: {
    notBefore?: number | null
    notAfter?: number | null
  } | null
  attestation?: {
    kind: string
    issuer?: string | null
  } | null
  persistence?: {
    kind: string
    detail?: string | null
  } | null
  webhook?: {
    provider?: 'slack' | 'teams' | 'generic' | null
  } | null
}
