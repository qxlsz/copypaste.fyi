import { describe, expect, it } from "vitest";

import { gmailShareHref, whatsappShareHref } from "../googleShare";

describe("gmailShareHref", () => {
  it("opens Gmail compose without a paste key", () => {
    const href = gmailShareHref("https://www.copypaste.fyi/p/abc#key=secret");
    expect(href.startsWith("https://mail.google.com/mail/")).toBe(true);
    expect(href).toContain("view=cm");
    expect(decodeURIComponent(href)).toContain("https://www.copypaste.fyi/p/abc");
    expect(decodeURIComponent(href)).not.toContain("secret");
    expect(decodeURIComponent(href)).not.toContain("key=");
  });

  it("opens WhatsApp without a paste key", () => {
    const href = whatsappShareHref("https://www.copypaste.fyi/p/abc#key=secret");
    expect(href.startsWith("https://wa.me/")).toBe(true);
    expect(decodeURIComponent(href)).toContain("https://www.copypaste.fyi/p/abc");
    expect(decodeURIComponent(href)).not.toContain("secret");
  });
});
