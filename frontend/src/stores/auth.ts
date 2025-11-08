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
  importKey: (keyData: string, format: KeyFormat) => Promise<{ pubkey: string; privkey: string }>
  validateKeyPair: (privkey: string, pubkey: string) => Promise<boolean>
}

export type KeyFormat = 'hex' | 'base64' | 'pem' | 'raw'

export const useAuth = create<AuthState>()(
  persist(
    (set, get) => ({
      user: null,
      token: null,
      isLoading: false,

      generateKeys: async () => {
        try {
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

      validateKeyPair: async (privkey: string, pubkey: string) => {
        try {
          const privkeyBytes = new Uint8Array(atob(privkey).split('').map((c) => c.charCodeAt(0)))
          const pubkeyBytes = new Uint8Array(atob(pubkey).split('').map((c) => c.charCodeAt(0)))
          
          // Verify the public key matches the private key
          const derivedPubkey = await ed25519.getPublicKey(privkeyBytes)
          
          return derivedPubkey.every((byte, i) => byte === pubkeyBytes[i])
        } catch (error) {
          console.error('Key pair validation failed:', error)
          return false
        }
      },

      importKey: async (keyData: string, format: KeyFormat) => {
        try {
          console.log('🔑 Importing key with format:', format)
          
          let privkeyBytes: Uint8Array
          
          switch (format) {
            case 'hex': {
              // Remove 0x prefix if present and validate hex
              const hexData = keyData.replace(/^0x/, '')
              if (!/^[0-9a-fA-F]{64}$/.test(hexData)) {
                throw new Error('Invalid hex format - must be 64 characters (32 bytes)')
              }
              privkeyBytes = new Uint8Array(hexData.match(/.{2}/g)!.map(byte => parseInt(byte, 16)))
              break
            }
              
            case 'base64': {
              try {
                privkeyBytes = new Uint8Array(atob(keyData).split('').map(c => c.charCodeAt(0)))
              } catch {
                throw new Error('Invalid base64 format')
              }
              break
            }
              
            case 'pem': {
              // Basic PEM parsing for Ed25519 private keys
              const pemMatch = keyData.match(/-----BEGIN (?:.* )?PRIVATE KEY-----([\s\S]*?)-----END (?:.* )?PRIVATE KEY-----/)
              if (!pemMatch) {
                throw new Error('Invalid PEM format - expected Ed25519 private key')
              }
              const pemBody = pemMatch[1].replace(/\s/g, '')
              try {
                const derBytes = new Uint8Array(atob(pemBody).split('').map(c => c.charCodeAt(0)))
                // For Ed25519, the private key is typically at offset 16 in PKCS#8 format
                if (derBytes.length >= 48) { // PKCS#8 minimum length
                  privkeyBytes = derBytes.slice(16, 48) // Extract 32-byte private key
                } else {
                  throw new Error('PEM key too short')
                }
              } catch {
                throw new Error('Failed to decode PEM body')
              }
              break
            }
              
            case 'raw': {
              // Raw binary data as base64
              try {
                privkeyBytes = new Uint8Array(atob(keyData).split('').map(c => c.charCodeAt(0)))
              } catch {
                throw new Error('Invalid raw key format')
              }
              break
            }
              
            default:
              throw new Error(`Unsupported key format: ${format}`)
          }
          
          // Validate key length (Ed25519 private keys are 32 bytes)
          if (privkeyBytes.length !== 32) {
            throw new Error(`Invalid key length: ${privkeyBytes.length} bytes (expected 32 bytes for Ed25519)`)
          }
          
          // Validate the key is valid for Ed25519
          try {
            const pubkey = await ed25519.getPublicKey(privkeyBytes)
            console.log('✅ Key import successful')
            
            return {
              pubkey: btoa(String.fromCharCode(...pubkey)),
              privkey: btoa(String.fromCharCode(...privkeyBytes)),
            }
          } catch {
            throw new Error('Invalid Ed25519 private key')
          }
          
        } catch (error) {
          console.error('❌ Key import failed:', error)
          throw new Error(`Key import failed: ${error instanceof Error ? error.message : 'Unknown error'}`)
        }
      },

      login: async (privkey) => {
        set({ isLoading: true })
        try {
          // Check for HTTPS in production
          if (window.location.protocol !== 'https:' && window.location.hostname !== 'localhost') {
            throw new Error('HTTPS is required for cryptographic operations')
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
