import { Link } from "react-router-dom";
import type { UserPasteListItem } from "../api/types";

interface Props {
  pastes: UserPasteListItem[];
  loading: boolean;
}

export function PasteTable({ pastes, loading }: Props) {
  if (loading) {
    return (
      <div className="flex items-center justify-center py-12 text-[#a1a1a1] text-sm">
        Loading pastes…
      </div>
    );
  }

  if (pastes.length === 0) {
    return (
      <div className="flex items-center justify-center py-12 text-[#a1a1a1] text-sm">
        No pastes yet.{" "}
        <Link to="/" className="ml-1 text-[#3b82f6] hover:underline">
          Create one →
        </Link>
      </div>
    );
  }

  return (
    <div className="overflow-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-[#262626] text-left text-xs text-[#a1a1a1] uppercase tracking-wide">
            <th className="pb-2 pr-4 font-medium">ID</th>
            <th className="pb-2 pr-4 font-medium">Format</th>
            <th className="pb-2 pr-4 font-medium">Views</th>
            <th className="pb-2 pr-4 font-medium">Created</th>
            <th className="pb-2 pr-4 font-medium">Expires</th>
            <th className="pb-2 font-medium">Flags</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-[#1a1a1a]">
          {pastes.map((paste) => (
            <tr key={paste.id} className="group">
              <td className="py-3 pr-4">
                <Link
                  to={paste.url}
                  className="font-mono text-[#3b82f6] hover:underline"
                >
                  {paste.id.slice(0, 8)}…
                </Link>
              </td>
              <td className="py-3 pr-4">
                <span className="bg-[#1a1a1a] text-[#a1a1a1] px-1.5 py-0.5 rounded text-xs font-mono">
                  {paste.format}
                </span>
              </td>
              <td className="py-3 pr-4 text-[#a1a1a1]">{paste.accessCount}</td>
              <td className="py-3 pr-4 text-[#a1a1a1] text-xs">
                {new Date(paste.createdAt * 1000).toLocaleDateString()}
              </td>
              <td className="py-3 pr-4 text-[#a1a1a1] text-xs">
                {paste.expiresAt
                  ? new Date(paste.expiresAt * 1000).toLocaleDateString()
                  : "never"}
              </td>
              <td className="py-3 text-xs">
                {paste.burnAfterReading && (
                  <span className="text-[#ef4444]">🔥 burn</span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
