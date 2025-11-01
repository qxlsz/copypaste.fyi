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
}
