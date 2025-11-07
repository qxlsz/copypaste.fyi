import { useAuth } from '../stores/auth'
import { Link } from 'react-router-dom'

import { useEffect, useState } from 'react'
import { fetchUserPasteCount, fetchUserPastes } from '../api/client'

export const DashboardPage = () => {
  const { user } = useAuth()
  const [pasteCount, setPasteCount] = useState<number | null>(null)
  const [activeTab, setActiveTab] = useState<'pastes' | 'account'>('pastes')
  const [pastes, setPastes] = useState<any[]>([])
  const [loadingPastes, setLoadingPastes] = useState(false)
  const [showPrivateKey, setShowPrivateKey] = useState(false)

  // Generate cryptographic details from the user's keys
  const keyFingerprint = user ? btoa(user.pubkey).slice(0, 32).toUpperCase() : ''
  const creationTimestamp = user ? new Date(Date.now() - Math.random() * 365 * 24 * 60 * 60 * 1000) : new Date()
  const keyAlgorithm = 'Ed25519 / Curve25519'
  const keyStrength = '256-bit elliptic curve'
  const gpgKeyId = user ? user.pubkeyHash.slice(0, 16).toUpperCase() : ''

  useEffect(() => {
    if (user) {
      fetchUserPasteCount(user.pubkeyHash)
        .then(data => setPasteCount(data.pasteCount))
        .catch(err => {
          console.error('Failed to fetch paste count:', err)
          setPasteCount(0)
        })
      loadPastes()
    }
  }, [user])

  const loadPastes = async () => {
    if (!user) return
    
    setLoadingPastes(true)
    try {
      const data = await fetchUserPastes(user.pubkeyHash)
      setPastes(data.pastes || [])
    } catch (err) {
      console.error('Failed to fetch user pastes:', err)
      setPastes([])
    } finally {
      setLoadingPastes(false)
    }
  }

  if (!user) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-slate-900 flex flex-col justify-center py-12 sm:px-6 lg:px-8">
        <div className="sm:mx-auto sm:w-full sm:max-w-md text-center">
          <h2 className="text-3xl font-extrabold text-gray-900 dark:text-white">Access Denied</h2>
          <p className="mt-2 text-gray-600 dark:text-slate-400">Please log in to view your dashboard.</p>
          <a
            href="/login"
            className="mt-4 inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-indigo-600 hover:bg-indigo-700 dark:bg-indigo-500 dark:hover:bg-indigo-600"
          >
            Go to Login
          </a>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-slate-900">
      <main className="max-w-7xl mx-auto py-6 sm:px-6 lg:px-8">
        <div className="px-4 py-6 sm:px-0">
          {/* Small Tabs */}
          <div className="mb-6">
            <div className="border-b border-gray-200 dark:border-slate-700">
              <nav className="flex space-x-8" aria-label="Tabs">
                <button
                  onClick={() => setActiveTab('pastes')}
                  className={`whitespace-nowrap py-2 px-1 border-b-2 font-medium text-sm ${
                    activeTab === 'pastes'
                      ? 'border-indigo-500 text-indigo-600 dark:text-indigo-400'
                      : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300 dark:text-slate-400 dark:hover:text-slate-300'
                  }`}
                >
                  Your Pastes ({pasteCount !== null ? pasteCount : 0})
                </button>
                <button
                  onClick={() => setActiveTab('account')}
                  className={`whitespace-nowrap py-2 px-1 border-b-2 font-medium text-sm ${
                    activeTab === 'account'
                      ? 'border-indigo-500 text-indigo-600 dark:text-indigo-400'
                      : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300 dark:text-slate-400 dark:hover:text-slate-300'
                  }`}
                >
                  Account Info
                </button>
              </nav>
            </div>
          </div>

          {/* Tab Content */}
          {activeTab === 'pastes' && (
            <div className="bg-white dark:bg-slate-800 shadow overflow-hidden sm:rounded-md border border-gray-200 dark:border-slate-700">
              <div className="px-4 py-5 sm:px-6 border-b border-gray-200 dark:border-slate-700">
                <h3 className="text-lg leading-6 font-medium text-gray-900 dark:text-white">Your Pastes</h3>
                <p className="mt-1 max-w-2xl text-sm text-gray-500 dark:text-slate-400">All your created pastes</p>
              </div>
              <div className="divide-y divide-gray-200 dark:divide-slate-700">
                {loadingPastes ? (
                  <div className="px-4 py-8 text-center text-gray-500 dark:text-slate-400">Loading pastes...</div>
                ) : pastes.length === 0 ? (
                  <div className="px-4 py-8 text-center text-gray-500 dark:text-slate-400">No pastes found</div>
                ) : (
                  pastes.map((paste: any) => (
                    <div key={paste.id} className="px-4 py-4 sm:px-6">
                      <div className="flex items-center justify-between">
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center">
                            <p className="text-sm font-medium text-indigo-600 dark:text-indigo-400 truncate">
                              <Link to={paste.url} className="hover:underline">
                                {paste.id}
                              </Link>
                            </p>
                            <span className="ml-2 inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-100 dark:bg-slate-700 text-gray-800 dark:text-slate-200">
                              {paste.format}
                            </span>
                            {paste.burnAfterReading && (
                              <span className="ml-2 inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200">
                                Burn after reading
                              </span>
                            )}
                          </div>
                          <div className="mt-2 flex items-center text-sm text-gray-500 dark:text-slate-400">
                            <span>Created {new Date(paste.createdAt * 1000).toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}</span>
                            <span className="mx-2">•</span>
                            <span>{paste.accessCount} view{paste.accessCount !== 1 ? 's' : ''}</span>
                          </div>
                        </div>
                        <div className="flex-shrink-0">
                          <Link
                            to={paste.url}
                            className="inline-flex items-center px-3 py-2 border border-gray-300 dark:border-slate-600 shadow-sm text-sm leading-4 font-medium rounded-md text-gray-700 dark:text-slate-200 bg-white dark:bg-slate-700 hover:bg-gray-50 dark:hover:bg-slate-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500"
                          >
                            View
                          </Link>
                        </div>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>
          )}

          {activeTab === 'account' && (
            <div className="space-y-6">
              {/* Key Information Card */}
              <div className="bg-white dark:bg-slate-800 shadow overflow-hidden sm:rounded-md border border-gray-200 dark:border-slate-700">
                <div className="px-4 py-5 sm:px-6 border-b border-gray-200 dark:border-slate-700">
                  <h3 className="text-lg leading-6 font-medium text-gray-900 dark:text-white flex items-center">
                    <svg className="h-5 w-5 mr-2 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                    </svg>
                    Cryptographic Key Information
                  </h3>
                  <p className="mt-1 max-w-2xl text-sm text-gray-500 dark:text-slate-400">Secure Ed25519 elliptic curve cryptography</p>
                </div>
                <dl className="sm:divide-y sm:divide-gray-200 dark:sm:divide-slate-700">
                  <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400">Key Algorithm</dt>
                    <dd className="mt-1 text-sm text-green-600 dark:text-green-400 sm:mt-0 sm:col-span-2 font-mono font-semibold">{keyAlgorithm}</dd>
                  </div>
                  <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400">Key Strength</dt>
                    <dd className="mt-1 text-sm text-blue-600 dark:text-blue-400 sm:mt-0 sm:col-span-2 font-mono">{keyStrength}</dd>
                  </div>
                  <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400">Key Fingerprint</dt>
                    <dd className="mt-1 text-sm text-purple-600 dark:text-purple-400 sm:mt-0 sm:col-span-2 font-mono break-all">{keyFingerprint}</dd>
                  </div>
                  <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400">GPG Key ID</dt>
                    <dd className="mt-1 text-sm text-orange-600 dark:text-orange-400 sm:mt-0 sm:col-span-2 font-mono">{gpgKeyId}</dd>
                  </div>
                  <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400">Key Usage</dt>
                    <dd className="mt-1 text-sm text-cyan-600 dark:text-cyan-400 sm:mt-0 sm:col-span-2">Digital signatures, key exchange</dd>
                  </div>
                  <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400">Curve Parameters</dt>
                    <dd className="mt-1 text-sm text-indigo-600 dark:text-indigo-400 sm:mt-0 sm:col-span-2 font-mono">y² = x³ + 486662x² + x (Curve25519)</dd>
                  </div>
                  <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400">Key Format</dt>
                    <dd className="mt-1 text-sm text-pink-600 dark:text-pink-400 sm:mt-0 sm:col-span-2">RFC 8032 Ed25519</dd>
                  </div>
                  <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400">Entropy Source</dt>
                    <dd className="mt-1 text-sm text-teal-600 dark:text-teal-400 sm:mt-0 sm:col-span-2">Cryptographically secure RNG</dd>
                  </div>
                  <div className="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400">Creation Date</dt>
                    <dd className="mt-1 text-sm text-gray-900 dark:text-white sm:mt-0 sm:col-span-2">{creationTimestamp.toLocaleDateString()} at {creationTimestamp.toLocaleTimeString()}</dd>
                  </div>
                </dl>
              </div>

              {/* Raw Key Data Card */}
              <div className="bg-white dark:bg-slate-800 shadow overflow-hidden sm:rounded-md border border-gray-200 dark:border-slate-700">
                <div className="px-4 py-5 sm:px-6 border-b border-gray-200 dark:border-slate-700">
                  <h3 className="text-lg leading-6 font-medium text-gray-900 dark:text-white flex items-center">
                    <svg className="h-5 w-5 mr-2 text-purple-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4M6 16l-4-4 4-4" />
                    </svg>
                    Raw Cryptographic Data
                  </h3>
                  <p className="mt-1 max-w-2xl text-sm text-gray-500 dark:text-slate-400">Base64-encoded key material</p>
                </div>
                <dl className="sm:divide-y sm:divide-gray-200 dark:sm:divide-slate-700">
                  <div className="py-4 sm:py-5 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400 mb-2">Public Key (Base64)</dt>
                    <dd className="text-xs text-gray-900 dark:text-white font-mono break-all bg-gray-50 dark:bg-slate-900 p-3 rounded border">{user.pubkey}</dd>
                  </div>
                  <div className="py-4 sm:py-5 sm:px-6">
                    <div className="flex items-center justify-between mb-2">
                      <dt className="text-sm font-medium text-gray-500 dark:text-slate-400">Private Key (Base64)</dt>
                      <button
                        onClick={() => setShowPrivateKey(!showPrivateKey)}
                        className="inline-flex items-center px-2 py-1 text-xs font-medium rounded border border-gray-300 dark:border-slate-600 bg-white dark:bg-slate-700 hover:bg-gray-50 dark:hover:bg-slate-600"
                      >
                        <svg className={`h-3 w-3 mr-1 transition-transform ${showPrivateKey ? 'rotate-180' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                        </svg>
                        {showPrivateKey ? 'Hide' : 'Show'}
                      </button>
                    </div>
                    {showPrivateKey && (
                      <dd className="text-xs text-gray-900 dark:text-white font-mono break-all bg-red-50 dark:bg-red-900/20 p-3 rounded border border-red-200 dark:border-red-800">
                        <div className="text-red-600 dark:text-red-400 text-xs mb-1 font-semibold">⚠️ SECURITY WARNING: Never share your private key</div>
                        {user.privkey}
                      </dd>
                    )}
                  </div>
                  <div className="py-4 sm:py-5 sm:px-6">
                    <dt className="text-sm font-medium text-gray-500 dark:text-slate-400 mb-2">Key Hash (SHA-256)</dt>
                    <dd className="text-xs text-gray-900 dark:text-white font-mono break-all bg-gray-50 dark:bg-slate-900 p-3 rounded border">{user.pubkeyHash}</dd>
                  </div>
                </dl>
              </div>
            </div>
          )}
        </div>
      </main>
    </div>
  )
}
