import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import * as ed25519 from '@noble/ed25519'
import { fetchAuthChallenge, loginWithSignature, logoutUser } from '../api/client'
import { toast } from 'sonner'

export interface User {
  pubkeyHash: string
  pubkey: string
  privkey: string
  createdAt: number
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
    (set, get) => ({
      user: null,
      token: null,
      isLoading: false,

      generateKeys: async () => {
        try {
          console.log('🔐 Initializing Ed25519...')
          if ('init' in ed25519 && typeof (ed25519 as { init?: () => Promise<void> }).init === 'function') {
            await (ed25519 as { init?: () => Promise<void> }).init?.()
          }
          
          console.log('🔑 Generating private key...')
          const privkey = ed25519.utils.randomPrivateKey()
          
          console.log('🔓 Deriving public key...')
          const pubkey = await ed25519.getPublicKey(privkey)

          console.log('✅ Key generation successful')
          const createdAt = Date.now()

          return {
            pubkey: btoa(String.fromCharCode(...pubkey)),
            privkey: btoa(String.fromCharCode(...privkey)),
            createdAt,
          }
        } catch (error) {
          console.error('❌ Key generation failed:', error)
          throw new Error(`Key generation failed: ${error instanceof Error ? error.message : 'Unknown error'}`)
        }
      },

      login: async (privkey) => {
        set({ isLoading: true })
        try {
          // Check for HTTPS in production
          if (window.location.protocol !== 'https:' && window.location.hostname !== 'localhost') {
            throw new Error('HTTPS is required for cryptographic operations')
          }

          console.log('🔐 Initializing Ed25519 for login...')
          if ('init' in ed25519 && typeof (ed25519 as { init?: () => Promise<void> }).init === 'function') {
            await (ed25519 as { init?: () => Promise<void> }).init?.()
          }
          
          console.log('🔑 Processing private key...')
          const privkeyBytes = privkey
            ? new Uint8Array(atob(privkey).split('').map((c) => c.charCodeAt(0)))
            : ed25519.utils.randomPrivateKey()

          console.log('🔓 Deriving public key...')
          const pubkeyBytes = await ed25519.getPublicKey(privkeyBytes)
          const pubkey = btoa(String.fromCharCode(...pubkeyBytes))

          console.log('📡 Fetching auth challenge...')
          const { challenge } = await fetchAuthChallenge()

          console.log('✍️  Signing challenge...')
          const challengeBytes = new TextEncoder().encode(challenge)
          const signatureBytes = await ed25519.sign(challengeBytes, privkeyBytes)
          const signature = btoa(String.fromCharCode(...signatureBytes))

          console.log('🔐 Logging in with signature...')
          const { token, pubkeyHash } = await loginWithSignature(challenge, signature, pubkey)

          console.log('✅ Login successful')
          const existingCreatedAt = get().user?.createdAt
          const createdAt = existingCreatedAt ?? Date.now()
          const user: User = {
            pubkeyHash,
            pubkey,
            privkey: btoa(String.fromCharCode(...privkeyBytes)),
            createdAt,
          }

          set({ user, token, isLoading: false })
          toast.success('Logged in successfully')
        } catch (error) {
          console.error('❌ Login failed:', error)
          set({ isLoading: false })
          const errorMessage = error instanceof Error ? error.message : 'Unknown error'
          toast.error('Login failed', { description: errorMessage })
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
