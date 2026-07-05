import { useAuth } from "../stores/auth";
import { Link } from "react-router-dom";

import { useCallback, useEffect, useState } from "react";
import { fetchUserPasteCount, fetchUserPastes } from "../api/client";
import type { UserPasteListItem } from "../api/types";

export const DashboardPage = () => {
  const { user } = useAuth();
  const [pasteCount, setPasteCount] = useState<number | null>(null);
  const [activeTab, setActiveTab] = useState<"pastes" | "account">("pastes");
  const [pastes, setPastes] = useState<UserPasteListItem[]>([]);
  const [loadingPastes, setLoadingPastes] = useState(false);
  const [showPrivateKey, setShowPrivateKey] = useState(false);
  const [keyFingerprint, setKeyFingerprint] = useState("");

  // Derive a real fingerprint: SHA-256 over the raw public key bytes,
  // hex-encoded, truncated to 16 chars.
  useEffect(() => {
    if (!user) {
      setKeyFingerprint("");
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const pubkeyBytes = new Uint8Array(
          atob(user.pubkey)
            .split("")
            .map((c) => c.charCodeAt(0)),
        );
        const digest = await crypto.subtle.digest("SHA-256", pubkeyBytes);
        const hex = Array.from(new Uint8Array(digest))
          .map((byte) => byte.toString(16).padStart(2, "0"))
          .join("");
        if (!cancelled) {
          setKeyFingerprint(hex.slice(0, 16).toUpperCase());
        }
      } catch {
        if (!cancelled) {
          setKeyFingerprint("");
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [user]);

  const creationTimestamp = user ? new Date(user.createdAt) : null;
  const keyAlgorithm = "Ed25519 / Curve25519";
  const keyStrength = "256-bit elliptic curve";
  const gpgKeyId = user ? user.pubkeyHash.slice(0, 16).toUpperCase() : "";

  const loadPastes = useCallback(async () => {
    if (!user) return;

    setLoadingPastes(true);
    try {
      const data = await fetchUserPastes(user.pubkeyHash);
      setPastes(data.pastes);
    } catch (err) {
      console.error("Failed to fetch user pastes:", err);
      setPastes([]);
    } finally {
      setLoadingPastes(false);
    }
  }, [user]);

  useEffect(() => {
    if (user) {
      fetchUserPasteCount(user.pubkeyHash)
        .then((data) => setPasteCount(data.pasteCount))
        .catch((err) => {
          console.error("Failed to fetch paste count:", err);
          setPasteCount(0);
        });
      loadPastes();
    } else {
      setPasteCount(null);
      setPastes([]);
    }
  }, [user, loadPastes]);

  if (!user) {
    return (
      <div className="mx-auto max-w-md space-y-4 py-16 text-center">
        <h2 className="text-xl font-semibold tracking-tight text-text">
          Access denied
        </h2>
        <p className="text-sm text-muted-foreground">
          Please log in to view your dashboard.
        </p>
        <Link
          to="/login"
          className="inline-flex items-center justify-center rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-foreground transition hover:bg-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background"
        >
          Go to Login
        </Link>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-5xl">
      {/* Small Tabs */}
      <div className="mb-6">
        <div className="border-b border-border">
          <nav className="flex space-x-6" aria-label="Tabs">
            <button
              onClick={() => setActiveTab("pastes")}
              className={`whitespace-nowrap border-b-2 px-1 py-2 text-sm font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ${
                activeTab === "pastes"
                  ? "border-accent text-text"
                  : "border-transparent text-muted-foreground hover:border-border hover:text-text"
              }`}
            >
              Your pastes ({pasteCount !== null ? pasteCount : 0})
            </button>
            <button
              onClick={() => setActiveTab("account")}
              className={`whitespace-nowrap border-b-2 px-1 py-2 text-sm font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ${
                activeTab === "account"
                  ? "border-accent text-text"
                  : "border-transparent text-muted-foreground hover:border-border hover:text-text"
              }`}
            >
              Account info
            </button>
          </nav>
        </div>
      </div>

      {/* Tab Content */}
      {activeTab === "pastes" && (
        <div className="overflow-hidden rounded-lg border border-border bg-surface">
          <div className="border-b border-border px-4 py-4 sm:px-6">
            <h3 className="text-base font-semibold tracking-tight text-text">
              Your pastes
            </h3>
            <p className="mt-1 max-w-2xl text-sm text-muted-foreground">
              All your created pastes
            </p>
          </div>
          <div className="divide-y divide-border">
            {loadingPastes ? (
              <div className="px-4 py-8 text-center text-muted-foreground">
                Loading pastes...
              </div>
            ) : pastes.length === 0 ? (
              <div className="px-4 py-8 text-center text-muted-foreground">
                No pastes found
              </div>
            ) : (
              pastes.map((paste: UserPasteListItem) => (
                <div key={paste.id} className="px-4 py-4 sm:px-6">
                  <div className="flex items-center justify-between">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center">
                        <p className="truncate font-mono text-sm font-medium text-accent">
                          <Link to={paste.url} className="hover:underline">
                            {paste.id}
                          </Link>
                        </p>
                        <span className="ml-2 inline-flex items-center rounded border border-border px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                          {paste.format}
                        </span>
                        {paste.burnAfterReading && (
                          <span className="ml-2 inline-flex items-center rounded border border-danger/40 bg-danger/10 px-1.5 py-0.5 font-mono text-[10px] text-danger">
                            burn-after-read
                          </span>
                        )}
                      </div>
                      <div className="mt-2 flex items-center font-mono text-xs text-muted-foreground">
                        <span>
                          Created{" "}
                          {new Date(paste.createdAt * 1000).toLocaleDateString(
                            "en-US",
                            {
                              year: "numeric",
                              month: "short",
                              day: "numeric",
                              hour: "2-digit",
                              minute: "2-digit",
                            },
                          )}
                        </span>
                        <span className="mx-2">•</span>
                        <span>
                          {paste.accessCount} view
                          {paste.accessCount !== 1 ? "s" : ""}
                        </span>
                      </div>
                    </div>
                    <div className="flex-shrink-0">
                      <Link
                        to={paste.url}
                        className="inline-flex items-center rounded-md border border-border bg-surface px-3 py-1.5 text-xs font-medium text-text transition hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface"
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

      {activeTab === "account" && (
        <div className="space-y-6">
          {/* Key Information Card */}
          <div className="overflow-hidden rounded-lg border border-border bg-surface">
            <div className="border-b border-border px-4 py-4 sm:px-6">
              <h3 className="text-base font-semibold tracking-tight text-text">
                Cryptographic key information
              </h3>
              <p className="mt-1 max-w-2xl text-sm text-muted-foreground">
                Secure Ed25519 elliptic curve cryptography
              </p>
            </div>
            <dl className="sm:divide-y sm:divide-border">
              <div className="py-4 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6 sm:py-5">
                <dt className="text-sm font-medium text-muted-foreground">
                  Key Algorithm
                </dt>
                <dd className="mt-1 font-mono text-sm text-text sm:col-span-2 sm:mt-0">
                  {keyAlgorithm}
                </dd>
              </div>
              <div className="py-4 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6 sm:py-5">
                <dt className="text-sm font-medium text-muted-foreground">
                  Key Strength
                </dt>
                <dd className="mt-1 font-mono text-sm text-text sm:col-span-2 sm:mt-0">
                  {keyStrength}
                </dd>
              </div>
              <div className="py-4 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6 sm:py-5">
                <dt className="text-sm font-medium text-muted-foreground">
                  Key Fingerprint
                </dt>
                <dd className="mt-1 break-all font-mono text-sm text-text sm:col-span-2 sm:mt-0">
                  {keyFingerprint || "Computing…"}
                </dd>
              </div>
              <div className="py-4 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6 sm:py-5">
                <dt className="text-sm font-medium text-muted-foreground">
                  GPG Key ID
                </dt>
                <dd className="mt-1 font-mono text-sm text-text sm:col-span-2 sm:mt-0">
                  {gpgKeyId}
                </dd>
              </div>
              <div className="py-4 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6 sm:py-5">
                <dt className="text-sm font-medium text-muted-foreground">
                  Key Usage
                </dt>
                <dd className="mt-1 text-sm text-text sm:col-span-2 sm:mt-0">
                  Digital signatures, key exchange
                </dd>
              </div>
              <div className="py-4 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6 sm:py-5">
                <dt className="text-sm font-medium text-muted-foreground">
                  Curve Parameters
                </dt>
                <dd className="mt-1 font-mono text-sm text-text sm:col-span-2 sm:mt-0">
                  y² = x³ + 486662x² + x (Curve25519)
                </dd>
              </div>
              <div className="py-4 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6 sm:py-5">
                <dt className="text-sm font-medium text-muted-foreground">
                  Key Format
                </dt>
                <dd className="mt-1 text-sm text-text sm:col-span-2 sm:mt-0">
                  RFC 8032 Ed25519
                </dd>
              </div>
              <div className="py-4 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6 sm:py-5">
                <dt className="text-sm font-medium text-muted-foreground">
                  Entropy Source
                </dt>
                <dd className="mt-1 text-sm text-text sm:col-span-2 sm:mt-0">
                  Cryptographically secure RNG
                </dd>
              </div>
              <div className="py-4 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6 sm:py-5">
                <dt className="text-sm font-medium text-muted-foreground">
                  Creation Date
                </dt>
                <dd className="mt-1 text-sm text-text sm:col-span-2 sm:mt-0">
                  {creationTimestamp
                    ? `${creationTimestamp.toLocaleDateString()} at ${creationTimestamp.toLocaleTimeString()}`
                    : "Unknown"}
                </dd>
              </div>
            </dl>
          </div>

          {/* Raw Key Data Card */}
          <div className="overflow-hidden rounded-lg border border-border bg-surface">
            <div className="border-b border-border px-4 py-4 sm:px-6">
              <h3 className="text-base font-semibold tracking-tight text-text">
                Raw cryptographic data
              </h3>
              <p className="mt-1 max-w-2xl text-sm text-muted-foreground">
                Base64-encoded key material
              </p>
            </div>
            <dl className="sm:divide-y sm:divide-border">
              <div className="py-4 sm:px-6 sm:py-5">
                <dt className="mb-2 text-sm font-medium text-muted-foreground">
                  Public Key (Base64)
                </dt>
                <dd className="break-all rounded-md border border-border bg-muted p-3 font-mono text-xs text-text">
                  {user.pubkey}
                </dd>
              </div>
              <div className="py-4 sm:px-6 sm:py-5">
                <div className="mb-2 flex items-center justify-between">
                  <dt className="text-sm font-medium text-muted-foreground">
                    Private Key (Base64)
                  </dt>
                  {user.privkey && (
                    <button
                      onClick={() => setShowPrivateKey(!showPrivateKey)}
                      className="inline-flex items-center rounded-md border border-border bg-surface px-2 py-1 text-xs font-medium text-text transition hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface"
                    >
                      <svg
                        className={`mr-1 h-3 w-3 transition-transform ${showPrivateKey ? "rotate-180" : ""}`}
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                        />
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                        />
                      </svg>
                      {showPrivateKey ? "Hide" : "Show"}
                    </button>
                  )}
                </div>
                {user.privkey ? (
                  showPrivateKey && (
                    <dd className="break-all rounded-md border border-danger/30 bg-danger/5 p-3 font-mono text-xs text-text">
                      <div className="mb-1 text-xs font-semibold text-danger">
                        Security warning: never share your private key
                      </div>
                      {user.privkey}
                    </dd>
                  )
                ) : (
                  <dd className="rounded-md border border-border bg-muted p-3 text-xs text-muted-foreground">
                    Private key is only held in memory for the current session
                    and was cleared on reload — import your key again to view
                    it.
                  </dd>
                )}
              </div>
              <div className="py-4 sm:px-6 sm:py-5">
                <dt className="mb-2 text-sm font-medium text-muted-foreground">
                  Key Hash (SHA-256)
                </dt>
                <dd className="break-all rounded-md border border-border bg-muted p-3 font-mono text-xs text-text">
                  {user.pubkeyHash}
                </dd>
              </div>
            </dl>
          </div>
        </div>
      )}
    </div>
  );
};
