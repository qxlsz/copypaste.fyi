import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export async function copyText(value: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(value);
    return true;
  } catch {
    try {
      const area = document.createElement("textarea");
      area.value = value;
      area.setAttribute("readonly", "");
      area.style.position = "fixed";
      area.style.left = "-9999px";
      document.body.appendChild(area);
      area.select();
      const ok = document.execCommand("copy");
      area.remove();
      return ok;
    } catch {
      return false;
    }
  }
}

export function downloadText(filename: string, content: string) {
  const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

export async function readTextFile(file: File, maxBytes: number): Promise<string> {
  if (file.size > maxBytes) {
    throw new Error("Paste exceeds the 1 MiB limit");
  }
  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  let nulls = 0;
  const sample = Math.min(bytes.length, 1024);
  for (let i = 0; i < sample; i += 1) {
    if (bytes[i] === 0) nulls += 1;
  }
  if (nulls > 8) {
    throw new Error("That file looks binary. Paste text instead.");
  }
  return new TextDecoder("utf-8").decode(bytes);
}
