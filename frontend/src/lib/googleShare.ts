import { publicPasteUrl } from "./openAgents";

const shareText = (url: string): string => `A paste from copypaste.fyi\n\n${publicPasteUrl(url)}`;

/** Gmail compose with the paste URL. No OAuth. Keys stay out. */
export const gmailShareHref = (url: string): string => {
  const body = encodeURIComponent(shareText(url));
  const su = encodeURIComponent("copypaste");
  return `https://mail.google.com/mail/?view=cm&fs=1&to=&su=${su}&body=${body}`;
};

/** WhatsApp share. Keys stay out. */
export const whatsappShareHref = (url: string): string =>
  `https://wa.me/?text=${encodeURIComponent(shareText(url))}`;
