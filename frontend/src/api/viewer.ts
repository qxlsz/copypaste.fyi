import type { PasteViewResponse } from '../server/types'

export const fetchPaste = async (id: string, key?: string): Promise<PasteViewResponse> => {
  const params = new URLSearchParams()
  if (key) {
    params.set('key', key)
  }
  const query = params.toString()
  const url = query ? `/api/pastes/${encodeURIComponent(id)}?${query}` : `/api/pastes/${encodeURIComponent(id)}`

  const controller = new AbortController()
  const timeoutId = setTimeout(() => controller.abort(), 10000) // 10 second timeout

  try {
    const response = await fetch(url, {
      signal: controller.signal,
    })
    clearTimeout(timeoutId)

    if (!response.ok) {
      throw new Error(`Request failed: ${response.status}`)
    }
    return (await response.json()) as PasteViewResponse
  } catch (error) {
    clearTimeout(timeoutId)
    if (error instanceof Error && error.name === 'AbortError') {
      throw new Error('Request timed out. Please check if the backend is running.')
    }
    throw error
  }
}
