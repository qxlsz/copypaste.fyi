import clsx from "clsx";

/**
 * Two sheets — copy sitting on paste.
 * Ink is currentColor so the same mark flips with light/dark.
 */
export const BrandMark = ({ className }: { className?: string }) => (
  <svg viewBox="0 0 32 32" fill="none" className={clsx("size-7", className)} aria-hidden="true">
    <rect
      x="2.125"
      y="2.125"
      width="16.75"
      height="16.75"
      rx="3.75"
      stroke="currentColor"
      strokeWidth="2.25"
    />
    <rect x="13.25" y="13.25" width="16.75" height="16.75" rx="3.75" fill="currentColor" />
  </svg>
);
