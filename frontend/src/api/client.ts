import type { CreatePastePayload, CreatePasteResponse, StatsSummary } from './types'

const API_BASE = import.meta.env.VITE_API_BASE ?? '/api'

const jsonFetch = async <T>(input: RequestInfo, init?: RequestInit): Promise<T> => {
  const response = await fetch(input, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...(init?.headers ?? {}),
    },
  })

  if (!response.ok) {
    const errorText = await response.text().catch(() => response.statusText)
    throw new Error(`Request failed: ${response.status} ${errorText}`)
  }

  if (response.status === 204) {
    return undefined as T
  }

  return (await response.json()) as T
}

export const createPaste = async (payload: CreatePastePayload): Promise<CreatePasteResponse> => {
  const url = `${API_BASE}/pastes`
  return jsonFetch<CreatePasteResponse>(url, {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export const fetchStatsSummary = async (): Promise<StatsSummary> => {
  const url = `${API_BASE}/stats/summary`
  return jsonFetch<StatsSummary>(url)
}
