import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import * as ed25519 from '@noble/ed25519'
import { fetchAuthChallenge, loginWithSignature, logoutUser } from '../api/client'
import { toast } from 'sonner'

export interface User {
  pubkeyHash: string
  pubkey: string
  privkey: string
}

interface AuthState {
  user: User | null
  token: string | null
  isLoading: boolean
  login: (privkey?: string) => Promise<void>
  logout: () => void
  generateKeys: () => Promise<{ pubkey: string; privkey: string }>
}

export const useAuth = create<AuthState>()(
  persist(
    (set) => ({
      user: null,
      token: null,
      isLoading: false,

      generateKeys: async () => {
        await (ed25519 as any).init?.()
        const privkey = ed25519.utils.randomPrivateKey()
        const pubkey = await ed25519.getPublicKey(privkey)

        return {
          pubkey: btoa(String.fromCharCode(...pubkey)),
          privkey: btoa(String.fromCharCode(...privkey)),
        }
      },

      login: async (privkey) => {
        set({ isLoading: true })
        try {
          await (ed25519 as any).init?.()
          const privkeyBytes = privkey
            ? new Uint8Array(atob(privkey).split('').map((c) => c.charCodeAt(0)))
            : ed25519.utils.randomPrivateKey()

          const pubkeyBytes = await ed25519.getPublicKey(privkeyBytes)
          const pubkey = btoa(String.fromCharCode(...pubkeyBytes))

          // Get challenge
          const { challenge } = await fetchAuthChallenge()

          // Sign challenge
          const challengeBytes = new TextEncoder().encode(challenge)
          const signatureBytes = await ed25519.sign(challengeBytes, privkeyBytes)
          const signature = btoa(String.fromCharCode(...signatureBytes))

          // Login
          const { token, pubkeyHash } = await loginWithSignature(challenge, signature, pubkey)

          const user: User = {
            pubkeyHash,
            pubkey,
            privkey: btoa(String.fromCharCode(...privkeyBytes)),
          }

          set({ user, token, isLoading: false })
          toast.success('Logged in successfully')
        } catch (error) {
          set({ isLoading: false })
          toast.error('Login failed', { description: error instanceof Error ? error.message : 'Unknown error' })
          throw error
        }
      },

      logout: () => {
        // Call server logout endpoint (non-blocking)
        logoutUser().catch(() => {}) // Ignore errors, logout is client-side primarily
        set({ user: null, token: null })
        toast.success('Logged out')
      },
    }),
    {
      name: 'auth-storage',
      partialize: (state) => ({
        user: state.user,
        token: state.token,
      }),
    }
  )
)
