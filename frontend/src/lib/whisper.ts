/** A share note that does not pretend the paste is listed or forever. */
export const whisperNote = (url: string): string =>
  `${url}\n\nNot listed. If that page says you're lost, it burned, expired, or never existed.`;
