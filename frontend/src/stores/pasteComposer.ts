import { create } from "zustand";

import type { EncryptionAlgorithm, PasteFormat } from "../api/types";

export const RETENTION_OPTIONS: Array<{ label: string; value: number }> = [
  { label: "No expiry", value: 0 },
  { label: "30 minutes", value: 30 },
  { label: "1 hour", value: 60 },
  { label: "1 day", value: 60 * 24 },
  { label: "7 days", value: 60 * 24 * 7 },
  { label: "30 days", value: 60 * 24 * 30 },
];

type ToastMessage = {
  title: string;
  description?: string;
  tone: "success" | "error" | "info";
};

interface PasteComposerState {
  content: string;
  format: PasteFormat;
  encryption: EncryptionAlgorithm;
  encryptionKey: string;
  burnAfterReading: boolean;
  selectedRetention: number;
  customRetention: string;
  shareUrl: string | null;
  isCopying: boolean;
  lastToast?: ToastMessage;
  setContent: (value: string) => void;
  setFormat: (value: PasteFormat) => void;
  setEncryption: (value: EncryptionAlgorithm) => void;
  setEncryptionKey: (value: string) => void;
  setBurnAfterReading: (value: boolean) => void;
  setSelectedRetention: (value: number) => void;
  setCustomRetention: (value: string) => void;
  setShareUrl: (value: string | null) => void;
  setCopying: (value: boolean) => void;
  recordToast: (toast: ToastMessage) => void;
  reset: () => void;
  getEffectiveRetention: () => number;
  requiresKey: () => boolean;
}

const initialState = {
  content: "",
  format: "plain_text" as PasteFormat,
  encryption: "none" as EncryptionAlgorithm,
  encryptionKey: "",
  burnAfterReading: false,
  selectedRetention: RETENTION_OPTIONS[0].value,
  customRetention: "",
  shareUrl: null as string | null,
  isCopying: false,
  lastToast: undefined as ToastMessage | undefined,
};

export const usePasteComposerStore = create<PasteComposerState>((set, get) => ({
  ...initialState,
  setContent: (value) => set({ content: value }),
  setFormat: (value) => set({ format: value }),
  setEncryption: (value) => set({ encryption: value }),
  setEncryptionKey: (value) => set({ encryptionKey: value }),
  setBurnAfterReading: (value) => set({ burnAfterReading: value }),
  setSelectedRetention: (value) => set({ selectedRetention: value }),
  setCustomRetention: (value) => set({ customRetention: value }),
  setShareUrl: (value) => set({ shareUrl: value }),
  setCopying: (value) => set({ isCopying: value }),
  recordToast: (toast) => set({ lastToast: toast }),
  reset: () => set(initialState),
  getEffectiveRetention: () => {
    const { selectedRetention, customRetention } = get();
    if (selectedRetention >= 0) {
      return selectedRetention;
    }
    const minutes = Number.parseInt(customRetention, 10);
    return Number.isFinite(minutes) && minutes >= 0 ? minutes : 0;
  },
  requiresKey: () => get().encryption !== "none",
}));
