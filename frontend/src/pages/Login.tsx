import { useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import { useAuth } from "../stores/auth";
import { toast } from "sonner";

export const LoginPage = () => {
  const navigate = useNavigate();
  const { login, generateKeys, isLoading } = useAuth();
  const [privkey, setPrivkey] = useState("");
  const [useExisting, setUseExisting] = useState(false);

  // Check for HTTPS in production
  const isHttps =
    window.location.protocol === "https:" ||
    window.location.hostname === "localhost";
  const showHttpsWarning = !isHttps && window.location.hostname !== "localhost";

  const handleGenerateKeys = async () => {
    try {
      const keys = await generateKeys();
      setPrivkey(keys.privkey);
      setUseExisting(true);
      toast.success("Keys generated successfully!", {
        description: `Public key: ${keys.pubkey.slice(0, 20)}...\n\n⚠️ Save your private key securely - it's shown below and stored locally.`,
        duration: 8000,
      });
    } catch {
      toast.error("Failed to generate keys");
    }
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (useExisting && !privkey.trim()) {
      toast.error("Private key is required");
      return;
    }

    try {
      await login(useExisting ? privkey : undefined);
      navigate("/dashboard");
    } catch {
      // Error handled in store
    }
  };

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-slate-900 flex flex-col justify-center py-12 sm:px-6 lg:px-8">
      <div className="sm:mx-auto sm:w-full sm:max-w-md">
        <h2 className="mt-6 text-center text-3xl font-extrabold text-gray-900 dark:text-white">
          Sign in to your account
        </h2>
        <p className="mt-2 text-center text-sm text-gray-600 dark:text-slate-400">
          Privacy-first authentication with ed25519 keys
        </p>
        {showHttpsWarning && (
          <div className="mt-4 p-3 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-md">
            <div className="flex">
              <svg
                className="h-5 w-5 text-yellow-400"
                fill="currentColor"
                viewBox="0 0 20 20"
              >
                <path
                  fillRule="evenodd"
                  d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z"
                  clipRule="evenodd"
                />
              </svg>
              <div className="ml-3">
                <p className="text-sm text-yellow-800 dark:text-yellow-200">
                  HTTPS Required: Cryptographic operations require a secure
                  connection. Please ensure you're accessing this site via
                  HTTPS.
                </p>
              </div>
            </div>
          </div>
        )}
      </div>

      <div className="mt-8 sm:mx-auto sm:w-full sm:max-w-md">
        <div className="bg-white dark:bg-slate-800 py-8 px-4 shadow sm:rounded-lg sm:px-10 border dark:border-slate-700">
          <form className="space-y-6" onSubmit={handleSubmit}>
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-slate-300">
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
                    className="h-4 w-4 text-indigo-600 focus:ring-indigo-500 border-gray-300 dark:border-slate-600 dark:bg-slate-700"
                  />
                  <label
                    htmlFor="generate-new"
                    className="ml-3 block text-sm font-medium text-gray-700 dark:text-slate-300"
                  >
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
                    className="h-4 w-4 text-indigo-600 focus:ring-indigo-500 border-gray-300 dark:border-slate-600 dark:bg-slate-700"
                  />
                  <label
                    htmlFor="use-existing"
                    className="ml-3 block text-sm font-medium text-gray-700 dark:text-slate-300"
                  >
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
                  className="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-700 dark:bg-indigo-500 dark:hover:bg-indigo-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500"
                >
                  Generate New Keys
                </button>
              </div>
            )}

            {useExisting && (
              <div>
                <label
                  htmlFor="privkey"
                  className="block text-sm font-medium text-gray-700 dark:text-slate-300"
                >
                  Private Key (base64)
                </label>
                <div className="mt-1">
                  <textarea
                    id="privkey"
                    name="privkey"
                    rows={3}
                    value={privkey}
                    onChange={(e) => setPrivkey(e.target.value)}
                    className="appearance-none block w-full px-3 py-2 border border-gray-300 dark:border-slate-600 rounded-md placeholder-gray-400 dark:placeholder-slate-500 bg-white dark:bg-slate-700 text-gray-900 dark:text-white focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm"
                    placeholder="Enter your base64-encoded private key..."
                  />
                </div>
              </div>
            )}

            <div>
              <button
                type="submit"
                disabled={isLoading || (useExisting && !privkey.trim())}
                className="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-700 dark:bg-indigo-500 dark:hover:bg-indigo-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isLoading ? "Signing in..." : "Sign in"}
              </button>
            </div>
          </form>

          <div className="mt-6">
            <div className="relative">
              <div className="absolute inset-0 flex items-center">
                <div className="w-full border-t border-gray-300 dark:border-slate-600" />
              </div>
              <div className="relative flex justify-center text-sm">
                <span className="px-2 bg-white dark:bg-slate-800 text-gray-500 dark:text-slate-400">
                  Or
                </span>
              </div>
            </div>

            <div className="mt-6 text-center">
              <Link
                to="/"
                className="font-medium text-indigo-600 dark:text-indigo-400 hover:text-indigo-500 dark:hover:text-indigo-300"
              >
                Back to home
              </Link>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
