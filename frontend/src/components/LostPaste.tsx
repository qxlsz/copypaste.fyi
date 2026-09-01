import { Link } from "react-router-dom";

import { BrandMark } from "./BrandMark";
import { quipFor } from "../lib/lostQuips";

export const LostPaste = ({ seed }: { seed?: string }) => (
  <section
    className="flex min-h-0 flex-1 flex-col items-center justify-center px-6 py-16"
    aria-label="Paste not found"
  >
    <div className="w-full max-w-sm space-y-5">
      <BrandMark className="size-8 text-text" />
      <h1 className="text-2xl font-medium tracking-tight text-text">{quipFor(seed)}</h1>
      <p className="text-sm leading-relaxed text-muted-foreground">
        Are you lost, or hunting a hidden secret? Missing, burned, and expired links look the same
        on purpose. There is no public listing to poke at.
      </p>
      <Link
        to="/"
        className="inline-flex h-12 items-center rounded-md bg-accent px-4 text-sm font-medium text-accent-foreground sm:h-11"
      >
        New paste
      </Link>
    </div>
  </section>
);
