import { cn } from "@/lib/utils";

export function BrandMark({ className }: { className?: string }) {
  return (
    <span className={cn("relative block size-4", className)} aria-hidden="true">
      <span className="absolute left-0 top-0 size-2.5 border border-current" />
      <span className="absolute bottom-0 right-0 size-2.5 bg-current" />
    </span>
  );
}
