import { useEffect, useState } from "react";
import { Eye, Globe, Lock, Shield, X, Zap } from "lucide-react";
import { cn } from "@/lib/utils";

type Step = {
  label: string;
  detail: string;
  detected: boolean;
  icon: typeof Lock;
};

export function PrivacyJourney({
  variant = "fab",
}: {
  variant?: "fab" | "inline" | "rail";
}) {
  const [open, setOpen] = useState(false);
  const [steps, setSteps] = useState<Step[]>([]);

  useEffect(() => {
    let cancelled = false;
    const detect = async () => {
      const isHttps = window.location.protocol === "https:";
      const isTor = window.location.hostname.endsWith(".onion");
      const dnt =
        navigator.doNotTrack === "1" ||
        (window as Window & { doNotTrack?: string }).doNotTrack === "1";
      let isPrivate = false;
      try {
        if ("storage" in navigator && "estimate" in navigator.storage) {
          const { quota } = await navigator.storage.estimate();
          isPrivate = (quota || 0) < 120_000_000;
        }
      } catch {
        isPrivate = true;
      }
      if (cancelled) return;
      setSteps([
        {
          icon: Lock,
          label: "Encrypted connection",
          detail: isHttps ? "TLS is active on this origin" : "This preview is not on HTTPS",
          detected: isHttps,
        },
        {
          icon: Globe,
          label: "Tor network",
          detail: isTor ? "Onion service host" : "Direct connection",
          detected: isTor,
        },
        {
          icon: Eye,
          label: "Do Not Track",
          detail: dnt ? "DNT is enabled" : "DNT is not set",
          detected: dnt,
        },
        {
          icon: Zap,
          label: "Private browsing",
          detail: isPrivate ? "Likely a private session" : "Normal browsing mode",
          detected: isPrivate,
        },
      ]);
    };
    void detect();
    return () => {
      cancelled = true;
    };
  }, []);

  const score = steps.filter((step) => step.detected).length;
  const inline = variant === "inline";
  const rail = variant === "rail";

  return (
    <div
      className={cn(
        inline ? "relative" : rail ? "relative" : "fixed bottom-4 left-4 z-40 sm:bottom-6 sm:left-6",
      )}
    >
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        aria-label="Privacy journey details"
        title={`Privacy ${score}/${steps.length || 4}`}
        className={cn(
          "inline-flex items-center gap-1.5 font-mono text-xs text-muted-foreground transition-colors duration-150 hover:text-foreground",
          inline && "min-h-11 rounded-md px-1",
          rail && "size-11 justify-center",
          !inline && !rail && "min-h-11 rounded-full border border-border bg-surface px-3 py-1.5 sm:min-h-0",
        )}
      >
        <Shield className="size-4" aria-hidden="true" />
        {inline ? `privacy ${score}/${steps.length || 4}` : rail ? null : `${score}/${steps.length || 4}`}
      </button>

      {open && (
        <div
          className={cn(
            "z-40 w-[min(20rem,calc(100vw-4rem))] border border-border bg-background p-4",
            inline && "absolute bottom-full right-0 mb-2",
            rail && "absolute bottom-0 left-full ml-1",
            !inline && !rail && "absolute bottom-12 left-0",
          )}
        >
          <div className="mb-3 flex items-center justify-between">
            <h3 className="text-sm font-medium tracking-tight">Your privacy journey</h3>
            <button
              type="button"
              onClick={() => setOpen(false)}
              className="inline-flex size-11 items-center justify-center text-muted-foreground transition-colors hover:text-foreground sm:size-8"
              aria-label="Close"
            >
              <X className="size-3.5" />
            </button>
          </div>
          <p className="mb-3 text-xs text-muted-foreground">
            {score} of {steps.length} connection signals look private.
          </p>
          <div className="space-y-1">
            {steps.map((step) => {
              const Icon = step.icon;
              return (
                <div key={step.label} className="flex items-start gap-3 p-2">
                  <Icon
                    className={cn(
                      "mt-0.5 size-4 shrink-0",
                      step.detected ? "text-success" : "text-muted-foreground",
                    )}
                  />
                  <div className="min-w-0 flex-1">
                    <p
                      className={cn(
                        "text-xs font-medium",
                        step.detected ? "text-foreground" : "text-muted-foreground",
                      )}
                    >
                      {step.label}
                    </p>
                    <p className="text-xs text-muted-foreground">{step.detail}</p>
                  </div>
                </div>
              );
            })}
          </div>
          <div className="mt-3 border border-border p-3 text-xs text-muted-foreground">
            <p className="font-medium text-foreground">Client-side encryption</p>
            <p className="mt-1">
              AES-256-GCM runs in the browser. Keys stay in the URL fragment and are never sent to
              the server.
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
