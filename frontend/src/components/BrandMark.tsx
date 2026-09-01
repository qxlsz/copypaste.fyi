import clsx from "clsx";

/** Black-and-white mark. Tile follows text color; glyphs follow the page paper. */
export const BrandMark = ({ className }: { className?: string }) => (
  <svg
    viewBox="0 0 64 64"
    className={clsx("size-8", className)}
    aria-hidden="true"
  >
    <rect width="64" height="64" rx="14" fill="currentColor" />
    <path
      d="M17 22 L29 32 L17 42"
      fill="none"
      stroke="var(--color-background)"
      strokeWidth="5.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <rect
      x="35"
      y="36.5"
      width="13"
      height="8"
      rx="2"
      fill="var(--color-background)"
    />
  </svg>
);
