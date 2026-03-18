import { useState } from "react";
import { motion } from "framer-motion";
import { Key } from "lucide-react";

interface Props {
  onSubmit: (key: string) => void;
  error?: string | null;
}

export function DecryptForm({ onSubmit, error }: Props) {
  const [key, setKey] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!key.trim()) return;
    setLoading(true);
    onSubmit(key.trim());
    setTimeout(() => setLoading(false), 2000);
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex flex-col items-center gap-4 max-w-sm w-full"
    >
      <div className="flex items-center gap-2 text-[#a855f7]">
        <Key size={20} />
        <span className="text-sm font-medium">This paste is encrypted</span>
      </div>
      <form onSubmit={handleSubmit} className="flex w-full gap-2">
        <input
          type="text"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder="Enter decryption key"
          className="flex-1 bg-[#141414] border border-[#262626] rounded-md px-3 py-2
                     text-sm text-[#fafafa] placeholder-[#a1a1a1] focus:border-[#a855f7]
                     focus:outline-none transition-colors"
          autoFocus
        />
        <button
          type="submit"
          disabled={loading || !key.trim()}
          className="px-4 py-2 bg-[#a855f7] hover:bg-[#9333ea] rounded-md text-sm
                     disabled:opacity-50 transition-colors text-white"
        >
          Decrypt
        </button>
      </form>
      {error && <p className="text-xs text-[#ef4444]">{error}</p>}
    </motion.div>
  );
}
