import { useEffect, useState } from "react";
import { Shield, Lock, Eye, Globe, Zap, X } from "lucide-react";

interface JourneyStep {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  detail: string;
  detected: boolean;
}

export const PrivacyJourney = () => {
  const [isExpanded, setIsExpanded] = useState(false);
  const [steps, setSteps] = useState<JourneyStep[]>([]);

  useEffect(() => {
    const detectPrivacyFeatures = async () => {
      const journeySteps: JourneyStep[] = [];

      // Check for HTTPS
      const isHttps = window.location.protocol === "https:";
      journeySteps.push({
        icon: Lock,
        label: "Encrypted Connection",
        detail: isHttps
          ? "TLS/SSL encryption active"
          : "Unencrypted connection",
        detected: isHttps,
      });

      // Check for Tor (onion address)
      const isTor = window.location.hostname.endsWith(".onion");
      journeySteps.push({
        icon: Globe,
        label: "Tor Network",
        detail: isTor ? "Accessing via Tor onion service" : "Direct connection",
        detected: isTor,
      });

      // Check for Do Not Track
      const dnt =
        navigator.doNotTrack === "1" ||
        (window as Window & { doNotTrack?: string }).doNotTrack === "1";
      journeySteps.push({
        icon: Eye,
        label: "Do Not Track",
        detail: dnt ? "DNT header enabled" : "DNT not set",
        detected: dnt,
      });

      // Check for Private/Incognito mode (heuristic)
      let isPrivateMode = false;
      try {
        // Test for private mode using storage quota
        if ("storage" in navigator && "estimate" in navigator.storage) {
          const { quota } = await navigator.storage.estimate();
          isPrivateMode = (quota || 0) < 120000000; // Less than 120MB suggests private mode
        }
      } catch {
        // Some browsers block this in private mode
        isPrivateMode = true;
      }
      journeySteps.push({
        icon: Zap,
        label: "Private Browsing",
        detail: isPrivateMode
          ? "Likely in private/incognito mode"
          : "Normal browsing mode",
        detected: isPrivateMode,
      });

      setSteps(journeySteps);
    };

    detectPrivacyFeatures();
  }, []);

  const privacyScore = steps.filter((s) => s.detected).length;
  const totalSteps = steps.length;

  return (
    <div className="fixed bottom-4 left-4 z-50 sm:bottom-6 sm:left-6">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        aria-expanded={isExpanded}
        aria-label="Privacy journey details"
        className="inline-flex items-center gap-1.5 rounded-full border border-border bg-surface px-2.5 py-1 font-mono text-[11px] text-muted-foreground transition hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background"
      >
        <Shield className="h-3 w-3" aria-hidden="true" />
        privacy {privacyScore}/{totalSteps}
      </button>

      {isExpanded && (
        <div className="absolute bottom-10 left-0 w-[calc(100vw-2rem)] max-w-sm rounded-lg border border-border bg-surface p-4 sm:w-80">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="text-sm font-semibold tracking-tight text-text">
              Your privacy journey
            </h3>
            <button
              onClick={() => setIsExpanded(false)}
              className="inline-flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
              aria-label="Close"
            >
              <X className="h-3.5 w-3.5" aria-hidden="true" />
            </button>
          </div>

          <p className="mb-3 text-xs text-muted-foreground">
            We detected {privacyScore} privacy measure
            {privacyScore !== 1 ? "s" : ""} protecting your connection
          </p>

          <div className="space-y-1">
            {steps.map((step, index) => {
              const Icon = step.icon;
              return (
                <div
                  key={index}
                  className="flex items-start gap-3 rounded-md p-2"
                >
                  <Icon
                    className={`mt-0.5 h-4 w-4 flex-shrink-0 ${
                      step.detected ? "text-success" : "text-muted-foreground"
                    }`}
                  />
                  <div className="min-w-0 flex-1">
                    <p
                      className={`text-xs font-medium ${
                        step.detected ? "text-text" : "text-muted-foreground"
                      }`}
                    >
                      {step.label}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {step.detail}
                    </p>
                  </div>
                  {step.detected && (
                    <span className="font-mono text-xs text-success">✓</span>
                  )}
                </div>
              );
            })}

            <div className="flex items-start gap-3 rounded-md p-2">
              <Shield className="mt-0.5 h-4 w-4 flex-shrink-0 text-muted-foreground" />
              <div className="min-w-0 flex-1">
                <p className="text-xs font-medium text-muted-foreground">
                  VPN/Proxy
                </p>
                <p className="text-xs text-muted-foreground">
                  Not assessed — no IP intelligence service is contacted
                </p>
              </div>
            </div>
          </div>

          <div className="mt-3 rounded-md border border-border p-3 text-xs text-muted-foreground">
            <p className="font-medium text-text">Privacy first</p>
            <p className="mt-1">
              Paste encryption runs on the server. Keys transit over TLS, are
              used in memory, and are not stored.
            </p>
          </div>

          <a
            href="https://how-did-i-get-here.net/"
            target="_blank"
            rel="noopener noreferrer"
            className="mt-3 block text-center text-xs text-muted-foreground transition hover:text-text"
          >
            Inspired by how-did-i-get-here.net ↗
          </a>
        </div>
      )}
    </div>
  );
};
