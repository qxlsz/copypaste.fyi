import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Copy, Check } from "lucide-react";

interface Props {
  text: string;
  label?: string;
  size?: "sm" | "md";
}

export function CopyButton({ text, label, size = "md" }: Props) {
  const [copied, setCopied] = useState(false);

  const handleClick = async () => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <motion.button
      onClick={handleClick}
      whileTap={{ scale: 0.95 }}
      className={`flex items-center gap-1.5 rounded transition-colors
        ${size === "sm" ? "px-2 py-1 text-xs" : "px-3 py-1.5 text-sm"}
        ${
          copied
            ? "bg-[#22c55e]/10 text-[#22c55e] border border-[#22c55e]/30"
            : "bg-[#141414] text-[#a1a1a1] hover:text-[#fafafa] border border-[#262626]"
        }`}
    >
      <AnimatePresence mode="wait">
        {copied ? (
          <motion.span
            key="check"
            initial={{ scale: 0 }}
            animate={{ scale: 1 }}
            className="flex items-center gap-1"
          >
            <Check size={14} /> {label ? "Copied!" : ""}
          </motion.span>
        ) : (
          <motion.span
            key="copy"
            initial={{ scale: 0 }}
            animate={{ scale: 1 }}
            className="flex items-center gap-1"
          >
            <Copy size={14} /> {label}
          </motion.span>
        )}
      </AnimatePresence>
    </motion.button>
  );
}
