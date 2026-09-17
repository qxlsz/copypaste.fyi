/** Text handed over by a browser share target or query paste. */
export const incomingShareText = (search: string): string => {
  const query = new URLSearchParams(search.startsWith("?") ? search.slice(1) : search);
  const parts = [query.get("title"), query.get("text"), query.get("url")]
    .map((value) => value?.trim() ?? "")
    .filter(Boolean);
  return [...new Set(parts)].join("\n");
};
