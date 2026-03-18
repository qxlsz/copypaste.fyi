import { Flame } from "lucide-react";

interface Props {
  enabled: boolean;
  onChange: (v: boolean) => void;
}

export function BurnToggle({ enabled, onChange }: Props) {
  return (
    <button
      onClick={() => onChange(!enabled)}
      className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors border group
        ${
          enabled
            ? "bg-[#ef4444]/10 text-[#ef4444] border-[#ef4444]/30"
            : "bg-[#141414] text-[#a1a1a1] hover:text-[#fafafa] border-[#262626]"
        }`}
    >
      <Flame
        size={14}
        className={enabled ? "animate-pulse" : "group-hover:animate-pulse"}
      />
      Burn
    </button>
  );
}
