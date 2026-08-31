import { createFileRoute, Link } from "@tanstack/react-router";
import { useState } from "react";
import { Check, Copy } from "lucide-react";
import { cn, copyText } from "@/lib/utils";

export const Route = createFileRoute("/install")({
  component: InstallPage,
});

function Snippet({
  id,
  label,
  value,
  wide = false,
}: {
  id: string;
  label: string;
  value: string;
  wide?: boolean;
}) {
  const [copied, setCopied] = useState(false);
  return (
    <div
      className={cn(
        "space-y-1.5 border border-border bg-background p-3",
        wide && "sm:col-span-2",
      )}
    >
      <div className="flex items-center justify-between gap-2">
        <label className="text-xs font-medium text-muted-foreground" htmlFor={id}>
          {label}
        </label>
        <button
          type="button"
          className="inline-flex size-11 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground sm:size-8"
          aria-label={`Copy ${label}`}
          onClick={async () => {
            if (await copyText(value)) setCopied(true);
          }}
        >
          {copied ? <Check className="size-3.5 text-success" /> : <Copy className="size-3.5" />}
        </button>
      </div>
      <pre id={id} className="overflow-x-auto whitespace-pre font-mono text-xs leading-6">
        {value}
      </pre>
    </div>
  );
}

function InstallPage() {
  const origin = typeof window === "undefined" ? "" : window.location.origin;
  return (
    <article className="space-y-8 text-sm leading-relaxed">
      <header className="space-y-2">
        <p className="font-mono text-xs lowercase text-muted-foreground">copypaste.v1</p>
        <h1 className="text-2xl font-medium tracking-tight">CLI, packages, agents</h1>
        <p className="text-muted-foreground">
          One protocol. Browser composer, Node CLI (client and server), packages for Homebrew,
          Debian, Nix, Snap, Docker, and npx. Architecture-independent. No listing. 1 MiB.
        </p>
      </header>

      <section className="space-y-3">
        <h2 className="text-base font-medium tracking-tight">Talk to this instance</h2>
        <div className="grid gap-3 sm:grid-cols-2">
          <Snippet
            id="this-host"
            label="send here"
            value={`echo 'from an agent' | copypaste send --host ${origin} --ttl 1h --json`}
            wide
          />
          <Snippet
            id="serve"
            label="org self-host"
            value="copypaste serve --bind 127.0.0.1 --token $COPYPASTE_WRITE_TOKEN"
            wide
          />
        </div>
      </section>

      <section className="space-y-3">
        <h2 className="text-base font-medium tracking-tight">Install</h2>
        <div className="grid gap-3 sm:grid-cols-2">
          <Snippet id="brew" label="Homebrew" value="brew install --HEAD qxlsz/tap/copypaste" />
          <Snippet id="debian" label="Debian / Ubuntu" value="sudo apt install copypaste" />
          <Snippet id="nix" label="Nix" value="nix run .#copypaste -- version" />
          <Snippet id="snap" label="Snap" value="sudo snap install copypaste" />
          <Snippet
            id="curl"
            label="from source"
            value="curl -fsSL https://github.com/qxlsz/copypaste.fyi/raw/main/install.sh | sh"
            wide
          />
          <Snippet
            id="docker"
            label="container"
            value="docker run --rm -p 8787:8787 ghcr.io/qxlsz/copypaste serve --bind 0.0.0.0"
            wide
          />
        </div>
        <p className="text-xs text-muted-foreground">
          Runtime is Node.js 22+. The binary is architecture-independent JavaScript.{" "}
          <code className="font-mono text-foreground">copypaste serve</code> stores pastes in{" "}
          <code className="font-mono text-foreground">~/.copypaste/pastes.sqlite</code>.
        </p>
      </section>

      <section className="space-y-2">
        <h2 className="text-base font-medium tracking-tight">Discovery</h2>
        <p className="text-muted-foreground">
          Agents should read{" "}
          <a href="/api/v1" className="underline decoration-border underline-offset-4 hover:decoration-foreground">
            /api/v1
          </a>
          ,{" "}
          <a href="/llms.txt" className="underline decoration-border underline-offset-4 hover:decoration-foreground">
            /llms.txt
          </a>
          ,{" "}
          <a href="/openapi.yaml" className="underline decoration-border underline-offset-4 hover:decoration-foreground">
            /openapi.yaml
          </a>
          , and{" "}
          <a href="/tools.json" className="underline decoration-border underline-offset-4 hover:decoration-foreground">
            /tools.json
          </a>
          . Do not scrape this HTML.
        </p>
      </section>

      <section className="space-y-2">
        <h2 className="text-base font-medium tracking-tight">Acceptable use</h2>
        <p className="text-muted-foreground">
          Not a backup, file host, or anonymity network. No malware, credential dumps, or bulk PII.
          Burn-after-reading is best-effort. Operators should set{" "}
          <code className="font-mono text-foreground">COPYPASTE_WRITE_TOKEN</code> before exposing an
          instance.
        </p>
      </section>

      <p>
        <Link to="/" className="underline decoration-border underline-offset-4 hover:underline">
          Back to the composer
        </Link>
      </p>
    </article>
  );
}
