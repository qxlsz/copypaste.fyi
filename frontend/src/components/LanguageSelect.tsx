import { LANGUAGES } from "../lib/languages";

interface Props {
  value: string;
  onChange: (v: string) => void;
}

export function LanguageSelect({ value, onChange }: Props) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="bg-[#141414] border border-[#262626] rounded-md px-3 py-1.5 text-sm
                 text-[#a1a1a1] focus:outline-none focus:border-[#3b82f6] cursor-pointer
                 dark:bg-[#141414] dark:border-[#262626] dark:text-[#a1a1a1]"
    >
      {LANGUAGES.map((l) => (
        <option key={l.value} value={l.value}>
          {l.label}
        </option>
      ))}
    </select>
  );
}
