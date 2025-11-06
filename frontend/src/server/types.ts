export interface PasteViewResponse {
  id: string
  format:
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
  content: string
  createdAt: number
  expiresAt?: number | null
  burnAfterReading: boolean
  bundle: {
    children: Array<{
      id: string
      label?: string | null
    }>
  } | null
  encryption: {
    algorithm: 'none' | 'aes256_gcm' | 'chacha20_poly1305' | 'xchacha20_poly1305'
    requiresKey: boolean
  }
  stego?: {
    carrierMime: string
    carrierImage: string
    payloadDigest: string
  } | null
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
