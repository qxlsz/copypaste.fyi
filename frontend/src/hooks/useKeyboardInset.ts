import { useEffect } from "react";

/** Keep the dock above the software keyboard via --keyboard-inset. */
export function useKeyboardInset() {
  useEffect(() => {
    const viewport = window.visualViewport;
    if (!viewport) return;

    const sync = () => {
      const inset = Math.max(0, window.innerHeight - viewport.height - viewport.offsetTop);
      document.documentElement.style.setProperty("--keyboard-inset", `${Math.round(inset)}px`);
    };

    sync();
    viewport.addEventListener("resize", sync);
    viewport.addEventListener("scroll", sync);
    return () => {
      viewport.removeEventListener("resize", sync);
      viewport.removeEventListener("scroll", sync);
      document.documentElement.style.removeProperty("--keyboard-inset");
    };
  }, []);
}
