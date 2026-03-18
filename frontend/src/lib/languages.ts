export interface Language {
  value: string;
  label: string;
  /** Monaco language id (if different from value) */
  monacoId?: string;
}

export const LANGUAGES: Language[] = [
  { value: "plain_text", label: "text", monacoId: "plaintext" },
  { value: "rust", label: "rust" },
  { value: "javascript", label: "js" },
  { value: "typescript", label: "ts" },
  { value: "python", label: "python" },
  { value: "go", label: "go" },
  { value: "bash", label: "bash" },
  { value: "json", label: "json" },
  { value: "yaml", label: "yaml" },
  { value: "markdown", label: "md" },
  { value: "sql", label: "sql" },
  { value: "cpp", label: "c++" },
  { value: "kotlin", label: "kotlin" },
  { value: "java", label: "java" },
  { value: "csharp", label: "c#" },
  { value: "php", label: "php" },
  { value: "ruby", label: "ruby" },
  { value: "swift", label: "swift" },
  { value: "html", label: "html" },
  { value: "css", label: "css" },
  { value: "code", label: "code" },
];

/** Get the Monaco language id for a paste format */
export function toMonacoLanguage(format: string): string {
  const lang = LANGUAGES.find((l) => l.value === format);
  return lang?.monacoId ?? lang?.value ?? "plaintext";
}
