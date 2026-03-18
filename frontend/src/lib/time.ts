/** Format a Unix timestamp (seconds) or ISO string as relative time. */
export function formatRelative(value: number | string): string {
  const ms =
    typeof value === "number"
      ? value * 1000 // Unix seconds → ms
      : new Date(value).getTime();

  const diff = ms - Date.now();
  if (diff <= 0) return "expired";

  const hours = Math.floor(diff / 3_600_000);
  if (hours > 48) return `in ${Math.floor(hours / 24)}d`;
  if (hours > 0) return `in ${hours}h`;

  const mins = Math.floor(diff / 60_000);
  if (mins > 0) return `in ${mins}m`;

  return "soon";
}
