import { motion } from "framer-motion";
import { Lock, LockOpen } from "lucide-react";

interface Props {
  enabled: boolean;
  onChange: (v: boolean) => void;
}

export function EncryptToggle({ enabled, onChange }: Props) {
  return (
    <button
      onClick={() => onChange(!enabled)}
      className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors border
        ${
          enabled
            ? "bg-[#a855f7]/10 text-[#a855f7] border-[#a855f7]/30"
            : "bg-[#141414] text-[#a1a1a1] hover:text-[#fafafa] border-[#262626]"
        }`}
    >
      <motion.div
        animate={{ rotate: enabled ? 0 : -15 }}
        transition={{ type: "spring", stiffness: 300, damping: 20 }}
      >
        {enabled ? <Lock size={14} /> : <LockOpen size={14} />}
      </motion.div>
      Encrypt
    </button>
  );
}
