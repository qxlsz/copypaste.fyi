const QUIPS = [
  "Either you're lost, or you're chasing a secret that isn't here.",
  "Burned, expired, or imaginary — we won't say which.",
  "This link went for a walk and never came back.",
  "No paste. No listing. No breadcrumbs.",
  "If this was a treasure map, X is not this spot.",
  "Someone might have burned this. Or they only dreamed it.",
] as const;

export const quipFor = (seed?: string): string => {
  if (!seed) {
    return QUIPS[0];
  }
  let hash = 2166136261;
  for (let i = 0; i < seed.length; i += 1) {
    hash ^= seed.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return QUIPS[Math.abs(hash) % QUIPS.length];
};
