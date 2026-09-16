import { publicPasteUrl } from "./openAgents";

/** Gmail compose with the paste URL. No OAuth. Keys stay out. */
export const gmailShareHref = (url: string): string => {
  const safe = publicPasteUrl(url);
  const body = encodeURIComponent(`A paste from copypaste.fyi\n\n${safe}`);
  const su = encodeURIComponent("copypaste");
  return `https://mail.google.com/mail/?view=cm&fs=1&to=&su=${su}&body=${body}`;
};
