const EXPIRE_OPTIONS = [
  { value: "60", label: "1h" },
  { value: "1440", label: "24h" },
  { value: "10080", label: "7d" },
  { value: "43200", label: "30d" },
  { value: "0", label: "never" },
];

interface Props {
  value: string;
  onChange: (v: string) => void;
}

export function ExpireSelect({ value, onChange }: Props) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="bg-[#141414] border border-[#262626] rounded-md px-3 py-1.5 text-sm
                 text-[#a1a1a1] focus:outline-none focus:border-[#3b82f6] cursor-pointer
                 dark:bg-[#141414] dark:border-[#262626] dark:text-[#a1a1a1]"
    >
      {EXPIRE_OPTIONS.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}
