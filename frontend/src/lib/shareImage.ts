import QRCode from "qrcode";

import { injectPngText } from "./pngText";

export interface ShareImageColors {
  ink: string;
  paper: string;
}

const SIZE = 1080;
const QR = 640;

const drawMark = (
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  size: number,
  ink: string,
) => {
  const scale = size / 32;
  ctx.save();
  ctx.translate(x, y);
  ctx.scale(scale, scale);
  ctx.strokeStyle = ink;
  ctx.fillStyle = ink;
  ctx.lineWidth = 2.25;
  ctx.beginPath();
  ctx.roundRect(2.125, 2.125, 16.75, 16.75, 3.75);
  ctx.stroke();
  ctx.beginPath();
  ctx.roundRect(13.25, 13.25, 16.75, 16.75, 3.75);
  ctx.fill();
  ctx.restore();
};

const loadImage = (src: string): Promise<HTMLImageElement> =>
  new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("QR image failed to load"));
    image.src = src;
  });

const canvasPng = (canvas: HTMLCanvasElement): Promise<Uint8Array> =>
  new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (!blob) {
        reject(new Error("PNG encode failed"));
        return;
      }
      void blob.arrayBuffer().then((buffer) => resolve(new Uint8Array(buffer)), reject);
    }, "image/png");
  });

export interface ShareImageOptions {
  url: string;
  colors: ShareImageColors;
  burn?: boolean;
  encryptionLabel?: string;
}

/** Square share card: mark + QR. Burn and encrypt print under the code. The image is a carrier, not a vault. */
export const renderShareImage = async (
  url: string,
  colors: ShareImageColors,
  extras: { burn?: boolean; encryptionLabel?: string } = {},
): Promise<Blob> => {
  const canvas = document.createElement("canvas");
  canvas.width = SIZE;
  canvas.height = SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Canvas unavailable");
  }

  ctx.fillStyle = colors.paper;
  ctx.fillRect(0, 0, SIZE, SIZE);

  drawMark(ctx, (SIZE - 96) / 2, 72, 96, colors.ink);

  const qrDataUrl = await QRCode.toDataURL(url, {
    errorCorrectionLevel: "H",
    margin: 1,
    width: QR,
    color: { dark: colors.ink, light: colors.paper },
  });
  const qr = await loadImage(qrDataUrl);
  ctx.drawImage(qr, (SIZE - QR) / 2, 220, QR, QR);

  const notes = [
    extras.burn ? "BURN AFTER READ" : "",
    extras.encryptionLabel ? extras.encryptionLabel.toUpperCase() : "",
  ].filter(Boolean);
  if (notes.length > 0) {
    ctx.fillStyle = colors.ink;
    ctx.font = "600 28px ui-sans-serif, system-ui, sans-serif";
    ctx.textAlign = "center";
    ctx.fillText(notes.join("  ·  "), SIZE / 2, 900);
  }

  const png = await canvasPng(canvas);
  const withSoftware = injectPngText(png, "Software", "copypaste.fyi");
  const withUrl = injectPngText(withSoftware, "URL", url);
  const comment = notes.length > 0 ? notes.join(" ") : "copypaste share card";
  const withComment = injectPngText(withUrl, "Comment", comment);
  const copy = new ArrayBuffer(withComment.byteLength);
  new Uint8Array(copy).set(withComment);
  return new Blob([copy], { type: "image/png" });
};

export const shareImageColorsFromDocument = (): ShareImageColors => {
  if (typeof document === "undefined") {
    return { ink: "#1c1b18", paper: "#f4f1ea" };
  }
  const styles = getComputedStyle(document.documentElement);
  return {
    ink: styles.getPropertyValue("--color-text").trim() || "#1c1b18",
    paper: styles.getPropertyValue("--color-background").trim() || "#f4f1ea",
  };
};

export const pasteIdFromShareUrl = (url: string): string => {
  try {
    const path = new URL(url).pathname;
    const match = path.match(/\/p\/([^/]+)/);
    return match?.[1] ?? "paste";
  } catch {
    return "paste";
  }
};

export const downloadBlob = (blob: Blob, filename: string) => {
  const href = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = href;
  anchor.download = filename;
  anchor.rel = "noopener";
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(href);
};
