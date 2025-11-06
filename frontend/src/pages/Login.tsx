import { useState, type FormEvent } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useAuth } from '../stores/auth'
import { toast } from 'sonner'

export const LoginPage = () => {
  const navigate = useNavigate()
  const { login, generateKeys, isLoading } = useAuth()
  const [privkey, setPrivkey] = useState('')
  const [useExisting, setUseExisting] = useState(false)

  const handleGenerateKeys = async () => {
    try {
      const keys = await generateKeys()
      setPrivkey(keys.privkey)
      setUseExisting(true)
      toast.success('Keys generated', {
        description: `Public key: ${keys.pubkey.slice(0, 20)}...`,
      })
    } catch (error) {
      toast.error('Failed to generate keys')
    }
  }

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (useExisting && !privkey.trim()) {
      toast.error('Private key is required')
      return
    }

    try {
      await login(useExisting ? privkey : undefined)
      navigate('/dashboard')
    } catch {
      // Error handled in store
    }
  }

  return (
    <div className="min-h-screen bg-gray-50 flex flex-col justify-center py-12 sm:px-6 lg:px-8">
      <div className="sm:mx-auto sm:w-full sm:max-w-md">
        <h2 className="mt-6 text-center text-3xl font-extrabold text-gray-900">
          Sign in to your account
        </h2>
        <p className="mt-2 text-center text-sm text-gray-600">
          Privacy-first authentication with ed25519 keys
        </p>
      </div>

      <div className="mt-8 sm:mx-auto sm:w-full sm:max-w-md">
        <div className="bg-white py-8 px-4 shadow sm:rounded-lg sm:px-10">
          <form className="space-y-6" onSubmit={handleSubmit}>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Authentication Method
              </label>
              <div className="mt-1 space-y-3">
                <div className="flex items-center">
                  <input
                    id="generate-new"
                    name="key-method"
                    type="radio"
                    checked={!useExisting}
                    onChange={() => setUseExisting(false)}
                    className="h-4 w-4 text-indigo-600 focus:ring-indigo-500 border-gray-300"
                  />
                  <label htmlFor="generate-new" className="ml-3 block text-sm font-medium text-gray-700">
                    Generate new keypair
                  </label>
                </div>
                <div className="flex items-center">
                  <input
                    id="use-existing"
                    name="key-method"
                    type="radio"
                    checked={useExisting}
                    onChange={() => setUseExisting(true)}
                    className="h-4 w-4 text-indigo-600 focus:ring-indigo-500 border-gray-300"
                  />
                  <label htmlFor="use-existing" className="ml-3 block text-sm font-medium text-gray-700">
                    Use existing private key
                  </label>
                </div>
              </div>
            </div>

            {!useExisting && (
              <div>
                <button
                  type="button"
                  onClick={handleGenerateKeys}
                  className="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500"
                >
                  Generate New Keys
                </button>
              </div>
            )}

            {useExisting && (
              <div>
                <label htmlFor="privkey" className="block text-sm font-medium text-gray-700">
                  Private Key (base64)
                </label>
                <div className="mt-1">
                  <textarea
                    id="privkey"
                    name="privkey"
                    rows={3}
                    value={privkey}
                    onChange={(e) => setPrivkey(e.target.value)}
                    className="appearance-none block w-full px-3 py-2 border border-gray-300 rounded-md placeholder-gray-400 focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm"
                    placeholder="Enter your base64-encoded private key..."
                  />
                </div>
              </div>
            )}

            <div>
              <button
                type="submit"
                disabled={isLoading || (useExisting && !privkey.trim())}
                className="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isLoading ? 'Signing in...' : 'Sign in'}
              </button>
            </div>
          </form>

          <div className="mt-6">
            <div className="relative">
              <div className="absolute inset-0 flex items-center">
                <div className="w-full border-t border-gray-300" />
              </div>
              <div className="relative flex justify-center text-sm">
                <span className="px-2 bg-white text-gray-500">Or</span>
              </div>
            </div>

            <div className="mt-6 text-center">
              <Link
                to="/"
                className="font-medium text-indigo-600 hover:text-indigo-500"
              >
                Back to home
              </Link>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
