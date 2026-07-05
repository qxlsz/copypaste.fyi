import { useEffect, useState } from "react";
import { Shield, Lock, Eye, Globe, Server, Zap, X } from "lucide-react";

interface JourneyStep {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  detail: string;
  detected: boolean;
}

type VpnStatus = "unknown" | "checking" | "detected" | "not_detected";

export const PrivacyJourney = () => {
  const [isExpanded, setIsExpanded] = useState(false);
  const [steps, setSteps] = useState<JourneyStep[]>([]);
  const [vpnStatus, setVpnStatus] = useState<VpnStatus>("unknown");

  // Opt-in only: contacting a third-party IP API is itself a privacy leak,
  // so it never runs automatically — only when the user clicks "Check".
  const checkVpn = async () => {
    setVpnStatus("checking");
    let isVpnLikely = false;
    try {
      const response = await fetch("https://ipapi.co/json/", {
        signal: AbortSignal.timeout(3000),
      });
      const data = await response.json();
      isVpnLikely =
        data.org?.toLowerCase().includes("vpn") ||
        data.org?.toLowerCase().includes("proxy") ||
        data.asn?.toString().includes("VPN");
      setVpnStatus(isVpnLikely ? "detected" : "not_detected");
    } catch {
      setVpnStatus("not_detected");
    }
  };

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

      // Client-side encryption
      journeySteps.push({
        icon: Server,
        label: "Client-Side Encryption",
        detail: "Keys never leave your device",
        detected: true, // Always true for this app
      });

      setSteps(journeySteps);
    };

    detectPrivacyFeatures();
  }, []);

  const vpnDetected = vpnStatus === "detected";
  const vpnDetail =
    vpnStatus === "unknown"
      ? "Unknown — click Check (queries ipapi.co)"
      : vpnStatus === "checking"
        ? "Checking…"
        : vpnDetected
          ? "Possible VPN/proxy detected"
          : "Direct IP connection";

  const privacyScore =
    steps.filter((s) => s.detected).length + (vpnDetected ? 1 : 0);
  const totalSteps = steps.length + 1;

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
              <Shield
                className={`mt-0.5 h-4 w-4 flex-shrink-0 ${
                  vpnDetected ? "text-success" : "text-muted-foreground"
                }`}
              />
              <div className="min-w-0 flex-1">
                <p
                  className={`text-xs font-medium ${
                    vpnDetected ? "text-text" : "text-muted-foreground"
                  }`}
                >
                  VPN/Proxy
                </p>
                <p className="text-xs text-muted-foreground">{vpnDetail}</p>
              </div>
              {vpnDetected ? (
                <span className="font-mono text-xs text-success">✓</span>
              ) : (
                <button
                  onClick={checkVpn}
                  disabled={vpnStatus === "checking"}
                  className="rounded border border-border px-2 py-0.5 font-mono text-[10px] text-muted-foreground transition hover:bg-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-60"
                >
                  {vpnStatus === "checking" ? "…" : "Check"}
                </button>
              )}
            </div>
          </div>

          <div className="mt-3 rounded-md border border-border p-3 text-xs text-muted-foreground">
            <p className="font-medium text-text">Privacy first</p>
            <p className="mt-1">
              All encryption happens in your browser. Your keys never touch our
              servers.
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
