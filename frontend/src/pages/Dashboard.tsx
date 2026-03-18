import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { useAuth } from "../context/AuthContext";
import { fetchUserPastes } from "../api/client";
import type { UserPasteListItem } from "../api/types";

export function Dashboard() {
  const { user, logout } = useAuth();
  const [pastes, setPastes] = useState<UserPasteListItem[]>([]);
  const [loading, setLoading] = useState(false);

  const loadPastes = useCallback(async () => {
    if (!user) return;
    setLoading(true);
    try {
      const data = await fetchUserPastes(user.pubkeyHash);
      setPastes(data.pastes);
    } catch {
      setPastes([]);
    } finally {
      setLoading(false);
    }
  }, [user]);

  useEffect(() => {
    if (user) {
      loadPastes();
    } else {
      setPastes([]);
    }
  }, [user, loadPastes]);

  if (!user) {
    return (
      <div className="min-h-screen bg-[#0a0a0a] text-[#fafafa] flex items-center justify-center">
        <div className="text-center">
          <p className="text-[#a1a1a1] mb-4">You need to log in first.</p>
          <Link
            to="/login"
            className="px-4 py-2 bg-[#3b82f6] hover:bg-[#2563eb] rounded-md text-sm transition-colors"
          >
            Go to Login
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-[#0a0a0a] text-[#fafafa] flex flex-col">
      <header className="flex items-center justify-between px-4 py-3 border-b border-[#262626]">
        <Link to="/" className="flex items-center gap-2">
          <span className="w-5 h-5 rounded-full bg-[#3b82f6]" />
          <span className="font-semibold text-sm">copypaste</span>
        </Link>
        <div className="flex items-center gap-3">
          <span className="text-xs text-[#a1a1a1] font-mono">
            {user.pubkeyHash.slice(0, 12)}…
          </span>
          <button
            onClick={logout}
            className="text-xs text-[#a1a1a1] hover:text-[#fafafa] transition-colors"
          >
            Logout
          </button>
        </div>
      </header>

      <main className="flex-1 p-4 max-w-5xl mx-auto w-full">
        <div className="flex items-center justify-between mb-4">
          <h1 className="text-sm font-medium text-[#fafafa]">
            Your Pastes
            <span className="ml-2 text-[#a1a1a1]">({pastes.length})</span>
          </h1>
          <Link
            to="/"
            className="px-3 py-1.5 bg-[#3b82f6] hover:bg-[#2563eb] rounded-md text-xs transition-colors"
          >
            + New paste
          </Link>
        </div>

        <div className="border border-[#262626] rounded-lg overflow-hidden">
          {loading ? (
            <div className="px-4 py-8 text-center text-[#a1a1a1] text-sm">
              Loading…
            </div>
          ) : pastes.length === 0 ? (
            <div className="px-4 py-8 text-center text-[#a1a1a1] text-sm">
              No pastes yet
            </div>
          ) : (
            pastes.map((paste, i) => (
              <div
                key={paste.id}
                className={`flex items-center justify-between px-4 py-3 ${
                  i < pastes.length - 1 ? "border-b border-[#262626]" : ""
                }`}
              >
                <div className="flex items-center gap-3 min-w-0">
                  <Link
                    to={paste.url}
                    className="text-sm font-mono text-[#3b82f6] hover:text-[#60a5fa] transition-colors truncate"
                  >
                    {paste.id}
                  </Link>
                  <span className="text-xs bg-[#262626] px-2 py-0.5 rounded text-[#a1a1a1] shrink-0">
                    {paste.format}
                  </span>
                  {paste.burnAfterReading && (
                    <span className="text-xs text-[#ef4444] shrink-0">
                      burn
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-4 shrink-0">
                  <span className="text-xs text-[#a1a1a1]">
                    {paste.accessCount} view{paste.accessCount !== 1 ? "s" : ""}
                  </span>
                  <span className="text-xs text-[#a1a1a1]">
                    {new Date(paste.createdAt * 1000).toLocaleDateString()}
                  </span>
                </div>
              </div>
            ))
          )}
        </div>
      </main>
    </div>
  );
}

// Keep backward-compat export alias
export const DashboardPage = Dashboard;
