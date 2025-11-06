import type { CreatePastePayload, CreatePasteResponse, StatsSummary, AuthChallengeResponse, AuthLoginResponse } from './types'
import type { PasteViewResponse } from '../server/types'

export const API_BASE = import.meta.env.VITE_API_BASE ?? '/api'

const jsonFetch = async <T>(input: RequestInfo, init?: RequestInit): Promise<T> => {
  const controller = new AbortController()
  const timeoutId = setTimeout(() => controller.abort(), 10000) // 10 second timeout

  try {
    const response = await fetch(input, {
      ...init,
      headers: {
        'Content-Type': 'application/json',
        ...(init?.headers ?? {}),
      },
      signal: controller.signal,
    })
    clearTimeout(timeoutId)

    if (!response.ok) {
      const errorText = await response.text().catch(() => response.statusText)
      throw new Error(`Request failed: ${response.status} ${errorText}`)
    }

    if (response.status === 204) {
      return undefined as T
    }

    return (await response.json()) as T
  } catch (error) {
    clearTimeout(timeoutId)
    if (error instanceof Error && error.name === 'AbortError') {
      throw new Error('Request timed out. Please check if the backend is running.')
    }
    throw error
  }
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

export const fetchPaste = async (id: string, key?: string): Promise<PasteViewResponse> => {
  const params = new URLSearchParams()
  if (key) {
    params.set('key', key)
  }
  const url = `${API_BASE}/pastes/${encodeURIComponent(id)}${params.toString() ? `?${params.toString()}` : ''}`
  return jsonFetch<PasteViewResponse>(url)
}

export const fetchAuthChallenge = async (): Promise<AuthChallengeResponse> => {
  const url = `${API_BASE}/auth/challenge`
  return jsonFetch<AuthChallengeResponse>(url)
}

export const loginWithSignature = async (
  challenge: string,
  signature: string,
  pubkey: string
): Promise<AuthLoginResponse> => {
  const url = `${API_BASE}/auth/login`
  return jsonFetch<AuthLoginResponse>(url, {
    method: 'POST',
    body: JSON.stringify({
      challenge,
      signature,
      pubkey,
    }),
  })
}
