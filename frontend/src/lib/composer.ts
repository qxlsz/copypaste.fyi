import type { PasteFormat } from "../api/types";

/** Matches fly.toml COPYPASTE_MAX_PASTE_SIZE. */
export const MAX_PASTE_BYTES = 1_048_576;

export const countLines = (text: string): number => {
  if (!text) return 0;
  return text.split(/\r\n|\n|\r/u).length;
};

export const utf8Bytes = (text: string): number => new TextEncoder().encode(text).length;

export const composerStats = (text: string): { chars: number; lines: number; bytes: number } => ({
  chars: [...text].length,
  lines: countLines(text),
  bytes: utf8Bytes(text),
});

const EXT_FORMAT: Record<string, PasteFormat> = {
  md: "markdown",
  markdown: "markdown",
  json: "json",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  ts: "typescript",
  tsx: "typescript",
  jsx: "javascript",
  py: "python",
  rs: "rust",
  go: "go",
  cpp: "cpp",
  cc: "cpp",
  cxx: "cpp",
  h: "cpp",
  hpp: "cpp",
  kt: "kotlin",
  java: "java",
  cs: "csharp",
  php: "php",
  rb: "ruby",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  yml: "yaml",
  yaml: "yaml",
  sql: "sql",
  swift: "swift",
  html: "html",
  htm: "html",
  css: "css",
  txt: "plain_text",
};

export const sniffFormat = (filename: string): PasteFormat | null => {
  const base = filename.split(/[/\\]/u).pop() ?? filename;
  const dot = base.lastIndexOf(".");
  if (dot <= 0) return null;
  const ext = base.slice(dot + 1).toLowerCase();
  return EXT_FORMAT[ext] ?? "code";
};

export const isTextFile = (file: File): boolean => {
  if (file.type.startsWith("text/")) return true;
  if (file.type === "application/json") return true;
  if (file.type === "application/javascript") return true;
  if (!file.type) return true;
  return false;
};
