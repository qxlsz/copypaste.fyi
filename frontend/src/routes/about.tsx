import { createFileRoute, Link } from "@tanstack/react-router";

export const Route = createFileRoute("/about")({
  component: AboutPage,
});

function AboutPage() {
  return (
    <article className="space-y-8 text-sm leading-relaxed">
      <header className="space-y-2">
        <p className="font-mono text-xs lowercase text-muted-foreground">copypaste.v1</p>
        <h1 className="text-2xl font-medium tracking-tight">Ephemeral storage with a hard edge</h1>
        <p className="text-muted-foreground">
          A paste layer for humans and agents. The browser composer, the HTTP API, and the CLI all
          speak the same protocol. Encryption happens on the client. There is no public listing.
        </p>
      </header>

      <section className="space-y-2">
        <h2 className="text-base font-medium tracking-tight">Security boundary</h2>
        <p className="text-muted-foreground">
          AES-256-GCM runs locally via the Web Crypto API. Keys are derived with PBKDF2-SHA-256
          (210,000 iterations) and a random 16-byte salt. Generated keys live in the URL fragment
          after <code className="font-mono text-foreground">#</code>, so they are not sent to the
          server. A CLI <code className="font-mono text-foreground">--key</code> or{" "}
          <code className="font-mono text-foreground">--key-file</code> stays in that process.
        </p>
      </section>

      <section className="space-y-2">
        <h2 className="text-base font-medium tracking-tight">Limits, on purpose</h2>
        <ul className="list-disc space-y-1 pl-5 text-muted-foreground">
          <li>Maximum paste size is 1 MiB. Binary uploads are rejected.</li>
          <li>API creates default to a 24-hour TTL unless you set one.</li>
          <li>Identifiers are 24-character CSPRNG strings. There is no directory.</li>
          <li>Burn-after-reading is best-effort, not a legal control.</li>
          <li>Optional write admission via COPYPASTE_WRITE_TOKEN.</li>
        </ul>
      </section>

      <section className="space-y-2">
        <h2 className="text-base font-medium tracking-tight">Self-host</h2>
        <p className="text-muted-foreground">
          Organizations run{" "}
          <code className="font-mono text-foreground">copypaste serve</code> with native SQLite under{" "}
          <code className="font-mono text-foreground">~/.copypaste</code>. Same protocol as this
          site. Set a write token before exposing it.
        </p>
      </section>

      <section className="space-y-2">
        <h2 className="text-base font-medium tracking-tight">Acceptable use</h2>
        <p className="text-muted-foreground">
          Not a backup, file host, or anonymity network. Do not distribute malware, dump
          credentials, or warehouse personal data here. Operators should set a write token and a
          conservative default TTL before exposing an instance.
        </p>
      </section>

      <section className="space-y-2">
        <h2 className="text-base font-medium tracking-tight">Agents</h2>
        <p className="text-muted-foreground">
          Read{" "}
          <a href="/llms.txt" className="underline decoration-border underline-offset-4 hover:decoration-foreground">
            /llms.txt
          </a>
          ,{" "}
          <a href="/tools.json" className="underline decoration-border underline-offset-4 hover:decoration-foreground">
            /tools.json
          </a>
          , and{" "}
          <a href="/api/v1" className="underline decoration-border underline-offset-4 hover:decoration-foreground">
            /api/v1
          </a>
          . Install the CLI from the{" "}
          <Link to="/install" className="underline decoration-border underline-offset-4 hover:decoration-foreground">
            packages page
          </Link>
          .
        </p>
      </section>

      <p>
        <a
          href="https://github.com/qxlsz/copypaste.fyi"
          target="_blank"
          rel="noopener noreferrer"
          className="underline decoration-border underline-offset-4 hover:underline"
        >
          github.com/qxlsz/copypaste.fyi
        </a>
      </p>
    </article>
  );
}
