// Format a remaining duration (in milliseconds) as a compact countdown
// string: "2d 4h", "23h 59m", "4m 12s", "42s", or "expired" once elapsed.
export const formatCountdown = (remainingMs: number): string => {
  if (remainingMs <= 0) {
    return "expired";
  }
  const totalSeconds = Math.floor(remainingMs / 1000);
  const days = Math.floor(totalSeconds / 86_400);
  const hours = Math.floor((totalSeconds % 86_400) / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;

  if (days > 0) {
    return `${days}d ${hours}h`;
  }
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
};
