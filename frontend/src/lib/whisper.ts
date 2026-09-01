/** A share note that does not pretend the paste is listed or forever. */
export const whisperLine =
  "Not listed. If that page says you're lost, it burned, expired, or never existed.";

export const whisperNote = (url: string): string => `${url}\n\n${whisperLine}`;

/** Payload for navigator.share. Keys never belong here. */
export const sharePayload = (url: string): ShareData => ({
  title: "copypaste.fyi",
  text: whisperLine,
  url,
});
