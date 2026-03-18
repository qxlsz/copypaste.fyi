import { useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import { useAuth } from "../context/AuthContext";
import { toast } from "sonner";

export function Login() {
  const navigate = useNavigate();
  const { login, generateKeys, isLoading } = useAuth();
  const [privkey, setPrivkey] = useState("");
  const [useExisting, setUseExisting] = useState(false);

  const isHttps =
    window.location.protocol === "https:" ||
    window.location.hostname === "localhost";

  const handleGenerateKeys = async () => {
    try {
      const keys = await generateKeys();
      setPrivkey(keys.privkey);
      setUseExisting(true);
      toast.success("Keys generated", {
        description: "Save your private key — it won't be shown again.",
        duration: 8000,
      });
    } catch {
      toast.error("Failed to generate keys");
    }
  };

  const handleSubmit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (useExisting && !privkey.trim()) {
      toast.error("Private key is required");
      return;
    }
    try {
      await login(useExisting ? privkey : undefined);
      navigate("/dashboard");
    } catch {
      // Error handled in AuthContext
    }
  };

  return (
    <div className="min-h-screen bg-[#0a0a0a] text-[#fafafa] flex flex-col">
      <header className="flex items-center px-4 py-3 border-b border-[#262626]">
        <Link to="/" className="flex items-center gap-2">
          <span className="w-5 h-5 rounded-full bg-[#3b82f6]" />
          <span className="font-semibold text-sm">copypaste</span>
        </Link>
      </header>

      <main className="flex-1 flex items-center justify-center p-4">
        <div className="w-full max-w-sm">
          <h1 className="text-lg font-semibold mb-1">Sign in</h1>
          <p className="text-sm text-[#a1a1a1] mb-6">
            Ed25519 key-based authentication
          </p>

          {!isHttps && (
            <div className="mb-4 px-3 py-2 bg-[#ca8a04]/10 border border-[#ca8a04]/30 rounded-md text-xs text-[#ca8a04]">
              HTTPS required for cryptographic operations outside localhost.
            </div>
          )}

          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <label className="text-xs text-[#a1a1a1]">Method</label>
              <div className="flex gap-3">
                <label className="flex items-center gap-2 text-sm cursor-pointer">
                  <input
                    type="radio"
                    checked={!useExisting}
                    onChange={() => setUseExisting(false)}
                    className="accent-[#3b82f6]"
                  />
                  Generate new
                </label>
                <label className="flex items-center gap-2 text-sm cursor-pointer">
                  <input
                    type="radio"
                    checked={useExisting}
                    onChange={() => setUseExisting(true)}
                    className="accent-[#3b82f6]"
                  />
                  Use existing key
                </label>
              </div>
            </div>

            {!useExisting && (
              <button
                type="button"
                onClick={handleGenerateKeys}
                className="w-full py-2 bg-[#262626] hover:bg-[#333] border border-[#333] rounded-md text-sm transition-colors"
              >
                Generate Keys
              </button>
            )}

            {useExisting && (
              <div className="flex flex-col gap-1.5">
                <label className="text-xs text-[#a1a1a1]">
                  Private key (base64)
                </label>
                <textarea
                  rows={3}
                  value={privkey}
                  onChange={(e) => setPrivkey(e.target.value)}
                  placeholder="Paste your base64-encoded private key…"
                  className="bg-[#141414] border border-[#262626] rounded-md px-3 py-2
                             text-sm text-[#fafafa] placeholder-[#a1a1a1] focus:border-[#3b82f6]
                             focus:outline-none transition-colors resize-none font-mono"
                />
              </div>
            )}

            <button
              type="submit"
              disabled={isLoading || (useExisting && !privkey.trim())}
              className="w-full py-2 bg-[#3b82f6] hover:bg-[#2563eb] disabled:opacity-50
                         rounded-md text-sm font-medium transition-colors"
            >
              {isLoading ? "Signing in…" : "Sign in"}
            </button>
          </form>

          <p className="mt-6 text-center text-xs text-[#a1a1a1]">
            <Link to="/" className="hover:text-[#fafafa] transition-colors">
              ← Back to home
            </Link>
          </p>
        </div>
      </main>
    </div>
  );
}

// Keep backward-compat export alias
export const LoginPage = Login;
