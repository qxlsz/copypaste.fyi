import { useAuth } from "../stores/auth";
import { useEffect, useState } from "react";
import { fetchUserPastes } from "../api/client";
import type { UserPasteListItem } from "../api/types";
import { Link } from "react-router-dom";

export const UserPastesPage = () => {
  const { user } = useAuth();
  const [pastes, setPastes] = useState<UserPasteListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!user) {
      setPastes([]);
      setLoading(false);
      return;
    }

    let isActive = true;
    setLoading(true);

    fetchUserPastes(user.pubkeyHash)
      .then((data) => {
        if (!isActive) return;
        setPastes(data.pastes);
        setLoading(false);
      })
      .catch((err) => {
        console.error("Failed to fetch user pastes:", err);
        if (!isActive) return;
        setError("Failed to load pastes");
        setLoading(false);
      });

    return () => {
      isActive = false;
    };
  }, [user]);

  const formatDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleDateString("en-US", {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  const formatExpiration = (paste: UserPasteListItem) => {
    if (paste.burnAfterReading) {
      return "Burn after reading";
    }
    if (paste.expiresAt) {
      const now = Date.now() / 1000;
      if (paste.expiresAt < now) {
        return "Expired";
      }
      const days = Math.ceil((paste.expiresAt - now) / (24 * 60 * 60));
      return `Expires in ${days} day${days !== 1 ? "s" : ""}`;
    }
    return "Never expires";
  };

  if (!user) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-slate-900 flex flex-col justify-center py-12 sm:px-6 lg:px-8">
        <div className="sm:mx-auto sm:w-full sm:max-w-md text-center">
          <h2 className="text-3xl font-extrabold text-gray-900 dark:text-white">
            Access Denied
          </h2>
          <p className="mt-2 text-gray-600 dark:text-slate-400">
            Please log in to view your pastes.
          </p>
          <Link
            to="/login"
            className="mt-4 inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-indigo-600 hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500"
          >
            Go to Login
          </Link>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-slate-900 py-12">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="text-center">
            <h1 className="text-3xl font-bold text-gray-900 dark:text-white">
              Your Pastes
            </h1>
            <p className="mt-2 text-gray-600 dark:text-slate-400">
              Loading your pastes...
            </p>
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-slate-900 py-12">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="text-center">
            <h1 className="text-3xl font-bold text-gray-900 dark:text-white">
              Your Pastes
            </h1>
            <p className="mt-2 text-red-600 dark:text-red-400">{error}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-slate-900 py-12">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="mb-8">
          <h1 className="text-3xl font-bold text-gray-900 dark:text-white">
            Your Pastes
          </h1>
          <p className="mt-2 text-gray-600 dark:text-slate-400">
            You have {pastes.length} paste{pastes.length !== 1 ? "s" : ""}
          </p>
        </div>

        {pastes.length === 0 ? (
          <div className="text-center py-12">
            <svg
              className="mx-auto h-12 w-12 text-gray-400"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
              />
            </svg>
            <h3 className="mt-2 text-sm font-medium text-gray-900 dark:text-white">
              No pastes
            </h3>
            <p className="mt-1 text-sm text-gray-500 dark:text-slate-400">
              Get started by creating your first paste.
            </p>
            <div className="mt-6">
              <Link
                to="/"
                className="inline-flex items-center px-4 py-2 border border-transparent shadow-sm text-sm font-medium rounded-md text-white bg-indigo-600 hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500"
              >
                Create a Paste
              </Link>
            </div>
          </div>
        ) : (
          <div className="bg-white dark:bg-slate-800 shadow overflow-hidden sm:rounded-md border border-gray-200 dark:border-slate-700">
            <ul className="divide-y divide-gray-200 dark:divide-slate-700">
              {pastes.map((paste) => (
                <li key={paste.id}>
                  <div className="px-4 py-4 sm:px-6">
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
                          <span>Created {formatDate(paste.createdAt)}</span>
                          <span className="mx-2">•</span>
                          <span>{formatExpiration(paste)}</span>
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
                          className="inline-flex items-center px-3 py-2 border border-gray-300 dark:border-slate-600 shadow-sm text-sm leading-4 font-medium rounded-md text-gray-700 dark:text-slate-200 bg-white dark:bg-slate-700 hover:bg-gray-50 dark:hover:bg-slate-600 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500"
                        >
                          View
                        </Link>
                      </div>
                    </div>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
  );
};
