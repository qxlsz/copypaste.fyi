import { useEffect } from "react";

type Options = {
  shortcut: string;
  handler: () => void;
  enabled?: boolean;
};

function matches(event: KeyboardEvent, shortcut: string): boolean {
  const parts = shortcut.toLowerCase().split("+");
  const key = parts[parts.length - 1];
  const needMeta = parts.includes("meta") || parts.includes("mod");
  const needCtrl = parts.includes("ctrl") || parts.includes("mod");
  const needShift = parts.includes("shift");
  const eventKey = event.key.toLowerCase() === "escape" ? "esc" : event.key.toLowerCase();
  if (eventKey !== key && event.code.toLowerCase() !== `key${key}`) return false;
  if (needMeta && !(event.metaKey || event.ctrlKey)) return false;
  if (parts.includes("meta") && !event.metaKey && !event.ctrlKey) return false;
  if (parts.includes("ctrl") && !event.ctrlKey && !event.metaKey) return false;
  if (!needMeta && !needCtrl && (event.metaKey || event.ctrlKey)) return false;
  if (needShift !== event.shiftKey) return false;
  return true;
}

export function useHotkeys({ shortcut, handler, enabled = true }: Options) {
  useEffect(() => {
    if (!enabled) return;
    const onKey = (event: KeyboardEvent) => {
      if (!matches(event, shortcut)) return;
      const target = event.target;
      if (target instanceof HTMLElement) {
        const typing =
          target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT" ||
          target.isContentEditable;
        const isMod = shortcut.toLowerCase().includes("mod") || shortcut.toLowerCase().includes("meta") || shortcut.toLowerCase().includes("ctrl");
        if (typing && !isMod && shortcut.toLowerCase() !== "esc") return;
      }
      event.preventDefault();
      handler();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shortcut, handler, enabled]);
}
