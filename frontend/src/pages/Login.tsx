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
        description: `Public key: ${keys.pubkey.slice(0, 20)}...\n\nSave your private key securely - it's shown below and stored locally.`,
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
    <div className="flex min-h-screen flex-col items-center justify-center bg-background px-4 py-12 text-text transition-colors">
      <div className="w-full max-w-sm space-y-6">
        <div className="space-y-1 text-center">
          <Link
            to="/"
            className="inline-flex items-baseline gap-px rounded-md font-mono text-sm font-medium lowercase text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background"
            aria-label="copypaste.fyi home"
          >
            copypaste
            <span
              aria-hidden="true"
              className="ml-0.5 inline-block h-[0.9em] w-[0.5em] translate-y-px bg-accent"
            />
          </Link>
          <h1 className="pt-2 text-xl font-semibold tracking-tight text-text">
            Sign in
          </h1>
          <p className="text-sm text-muted-foreground">
            Privacy-first authentication with ed25519 keys
          </p>
        </div>

        {showHttpsWarning && (
          <div className="rounded-md border border-warning/40 bg-warning/10 p-3">
            <p className="text-xs text-text">
              HTTPS Required: Cryptographic operations require a secure
              connection. Please ensure you're accessing this site via HTTPS.
            </p>
          </div>
        )}

        <div className="rounded-lg border border-border bg-surface p-6">
          <form className="space-y-5" onSubmit={handleSubmit}>
            <fieldset className="space-y-2">
              <legend className="text-xs font-medium text-muted-foreground">
                authentication method
              </legend>
              <div className="flex items-center">
                <input
                  id="generate-new"
                  name="key-method"
                  type="radio"
                  checked={!useExisting}
                  onChange={() => setUseExisting(false)}
                  className="h-4 w-4 border-border text-accent focus:ring-accent"
                />
                <label
                  htmlFor="generate-new"
                  className="ml-2.5 block text-sm text-text"
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
                  className="h-4 w-4 border-border text-accent focus:ring-accent"
                />
                <label
                  htmlFor="use-existing"
                  className="ml-2.5 block text-sm text-text"
                >
                  Use existing private key
                </label>
              </div>
            </fieldset>

            {!useExisting && (
              <button
                type="button"
                onClick={handleGenerateKeys}
                className="flex w-full justify-center rounded-md border border-border bg-surface px-4 py-2 text-sm font-medium text-text transition hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface"
              >
                Generate New Keys
              </button>
            )}

            {useExisting && (
              <div className="space-y-1.5">
                <label
                  htmlFor="privkey"
                  className="block text-xs font-medium text-muted-foreground"
                >
                  private key (base64)
                </label>
                <textarea
                  id="privkey"
                  name="privkey"
                  rows={3}
                  value={privkey}
                  onChange={(e) => setPrivkey(e.target.value)}
                  className="block w-full appearance-none rounded-md border border-border bg-surface px-3 py-2 font-mono text-xs text-text placeholder:text-muted-foreground focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent"
                  placeholder="Enter your base64-encoded private key…"
                />
              </div>
            )}

            <button
              type="submit"
              disabled={isLoading || (useExisting && !privkey.trim())}
              className="flex w-full justify-center rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-foreground transition hover:bg-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface disabled:cursor-not-allowed disabled:opacity-50"
            >
              {isLoading ? "Signing in…" : "Sign in"}
            </button>
          </form>
        </div>

        <p className="text-center text-xs text-muted-foreground">
          <Link to="/" className="transition hover:text-text">
            ← Back to home
          </Link>
        </p>
      </div>
    </div>
  );
};
