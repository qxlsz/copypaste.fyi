export const PASTE_FORMATS = [
  { label: "Plain text", value: "plain_text" },
  { label: "Markdown", value: "markdown" },
  { label: "Generic code", value: "code" },
  { label: "JSON", value: "json" },
  { label: "JavaScript", value: "javascript" },
  { label: "TypeScript", value: "typescript" },
  { label: "Python", value: "python" },
  { label: "Rust", value: "rust" },
  { label: "Go", value: "go" },
  { label: "C++", value: "cpp" },
  { label: "Kotlin", value: "kotlin" },
  { label: "Java", value: "java" },
  { label: "C#", value: "csharp" },
  { label: "PHP", value: "php" },
  { label: "Ruby", value: "ruby" },
  { label: "Bash", value: "bash" },
  { label: "YAML", value: "yaml" },
  { label: "SQL", value: "sql" },
  { label: "Swift", value: "swift" },
  { label: "HTML", value: "html" },
  { label: "CSS", value: "css" },
] as const;

export type PasteFormat = (typeof PASTE_FORMATS)[number]["value"];

export const RETENTION_OPTIONS = [
  { label: "∞", value: 0, title: "Keep until forgotten" },
  { label: "1m", value: 1, title: "1 minute" },
  { label: "10m", value: 10, title: "10 minutes" },
  { label: "1h", value: 60, title: "1 hour" },
  { label: "3h", value: 180, title: "3 hours" },
  { label: "1d", value: 1440, title: "1 day" },
  { label: "7d", value: 10080, title: "7 days" },
  { label: "30d", value: 43200, title: "30 days" },
] as const;

export const MAX_PASTE_BYTES = 1_048_576;
/** Ciphertext is plaintext + 16-byte GCM tag, then base64url (~4/3). */
export const MAX_STORED_BYTES = Math.ceil((MAX_PASTE_BYTES + 16) * (4 / 3)) + 32;
export const PASTE_ID_PATTERN = /^[A-Za-z0-9]{24}$/;

const EXTENSIONS: Record<string, PasteFormat> = {
  txt: "plain_text",
  md: "markdown",
  markdown: "markdown",
  json: "json",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "javascript",
  ts: "typescript",
  tsx: "typescript",
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
};

const FORMAT_EXTENSION: Record<string, string> = {
  plain_text: "txt",
  markdown: "md",
  code: "txt",
  json: "json",
  javascript: "js",
  typescript: "ts",
  python: "py",
  rust: "rs",
  go: "go",
  cpp: "cpp",
  kotlin: "kt",
  java: "java",
  csharp: "cs",
  php: "php",
  ruby: "rb",
  bash: "sh",
  yaml: "yml",
  sql: "sql",
  swift: "swift",
  html: "html",
  css: "css",
};

export function isPasteFormat(value: string): value is PasteFormat {
  return PASTE_FORMATS.some((option) => option.value === value);
}

export function formatLabel(value: string): string {
  return PASTE_FORMATS.find((option) => option.value === value)?.label ?? value;
}

export function formatExtension(value: string): string {
  return FORMAT_EXTENSION[value] ?? "txt";
}

export function formatFromFilename(name: string): PasteFormat | null {
  const base = name.split(/[/\\]/).pop() ?? name;
  const dot = base.lastIndexOf(".");
  if (dot <= 0) return null;
  const ext = base.slice(dot + 1).toLowerCase();
  return EXTENSIONS[ext] ?? null;
}

export function retentionLabel(minutes: number | null | undefined): string {
  if (!minutes) return "until forgotten";
  const match = RETENTION_OPTIONS.find((option) => option.value === minutes);
  if (match) return match.title;
  if (minutes < 60) return `${minutes}m`;
  if (minutes < 1440) return `${Math.round(minutes / 60)}h`;
  return `${Math.round(minutes / 1440)}d`;
}

export function byteLength(text: string): number {
  return new TextEncoder().encode(text).byteLength;
}

export function detectFormat(content: string): PasteFormat | null {
  const text = content.trim();
  if (text.length < 8) return null;

  if (
    (text.startsWith("{") && text.endsWith("}")) ||
    (text.startsWith("[") && text.endsWith("]"))
  ) {
    try {
      JSON.parse(text);
      return "json";
    } catch {
      /* not json */
    }
  }

  const head = text.slice(0, 2400);

  if (/^#!/.test(head) && /\b(bash|sh|zsh|env bash)\b/.test(head.slice(0, 80))) {
    return "bash";
  }
  if (/^<!DOCTYPE html/i.test(head) || /^<html[\s>]/i.test(head)) return "html";
  if (/^(SELECT|WITH|INSERT|UPDATE|DELETE|CREATE|ALTER)\s/i.test(head)) return "sql";
  if (/\b(fn |let mut |impl |pub struct |pub enum )\b/.test(head)) return "rust";
  if (/\bpackage\s+\w+/.test(head) && /\bfunc\s+\w+\(/.test(head)) return "go";
  if (/\bdef\s+\w+\(/.test(head) || /\bfrom\s+\w+\s+import\b/.test(head) || head.includes("if __name__")) {
    return "python";
  }
  if (
    (/\binterface\s+\w+/.test(head) || /\btype\s+\w+\s*=/.test(head) || /:\s*(string|number|boolean|void)\b/.test(head)) &&
    /\b(import |export |const |function )/.test(head)
  ) {
    return "typescript";
  }
  if (/\b(import .+ from ['"]|export default |const \w+ = |function \w+\()/.test(head)) {
    return "javascript";
  }
  if (/^#{1,6} .+/m.test(head) && (/\[.+\]\(https?:\/\//.test(head) || /^## /m.test(head))) {
    return "markdown";
  }
  if (
    !head.includes("{") &&
    /^[\w-]+:\s.+/m.test(head) &&
    head.split("\n").filter((line) => /^[\w-]+:\s/.test(line)).length >= 3
  ) {
    return "yaml";
  }
  return null;
}
