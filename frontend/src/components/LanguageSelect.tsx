import type { PasteFormat } from "../api/types";

const LANGUAGES: { value: PasteFormat; label: string }[] = [
  { value: "plain_text", label: "text" },
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
];

interface Props {
  value: PasteFormat;
  onChange: (v: PasteFormat) => void;
}

export function LanguageSelect({ value, onChange }: Props) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as PasteFormat)}
      className="bg-[#141414] border border-[#262626] rounded-md px-3 py-1.5 text-sm
                 text-[#a1a1a1] focus:outline-none focus:border-[#3b82f6] cursor-pointer"
    >
      {LANGUAGES.map((l) => (
        <option key={l.value} value={l.value}>
          {l.label}
        </option>
      ))}
    </select>
  );
}
