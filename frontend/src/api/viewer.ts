import type { PasteViewResponse } from '../server/types'

export const fetchPaste = async (id: string, key?: string): Promise<PasteViewResponse> => {
  const params = new URLSearchParams()
  if (key) {
    params.set('key', key)
  }
  const query = params.toString()
  const url = query ? `/api/pastes/${encodeURIComponent(id)}?${query}` : `/api/pastes/${encodeURIComponent(id)}`
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`Request failed: ${response.status}`)
  }
  return (await response.json()) as PasteViewResponse
}
