const AMP = "\u0026amp;";
const LT = "\u0026lt;";
const GT = "\u0026gt;";
const QUOT = "\u0026quot;";

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, AMP)
    .replace(/</g, LT)
    .replace(/>/g, GT)
    .replace(/"/g, QUOT);
}

function inline(value: string): string {
  const escaped = escapeHtml(value);
  return escaped
    .replace(
      /`([^`]+)`/g,
      '<code class="rounded-sm bg-muted px-1 py-0.5 font-mono text-[0.85em]">$1</code>',
    )
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/_([^_]+)_/g, "<em>$1</em>")
    .replace(
      /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
      '<a class="text-accent underline-offset-2 hover:underline" href="$2" rel="noopener noreferrer" target="_blank">$1</a>',
    );
}

export function renderMarkdown(source: string): string {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const html: string[] = [];
  let inCode = false;
  let code: string[] = [];
  let list: string[] = [];

  const flushList = () => {
    if (!list.length) return;
    html.push(`<ul class="my-3 list-disc space-y-1 pl-5">${list.join("")}</ul>`);
    list = [];
  };

  const flushCode = () => {
    if (!inCode) return;
    html.push(
      `<pre class="my-3 overflow-x-auto rounded-md border border-border bg-muted px-3 py-3 font-mono text-sm leading-relaxed"><code>${escapeHtml(code.join("\n"))}</code></pre>`,
    );
    inCode = false;
    code = [];
  };

  for (const line of lines) {
    if (line.startsWith("```")) {
      if (inCode) flushCode();
      else {
        flushList();
        inCode = true;
        code = [];
      }
      continue;
    }
    if (inCode) {
      code.push(line);
      continue;
    }
    if (/^\s*[-*]\s+/.test(line)) {
      list.push(`<li>${inline(line.replace(/^\s*[-*]\s+/, ""))}</li>`);
      continue;
    }
    flushList();
    if (!line.trim()) {
      html.push("");
      continue;
    }
    if (line.startsWith("### ")) {
      html.push(
        `<h3 class="mt-5 mb-2 text-base font-medium tracking-tight">${inline(line.slice(4))}</h3>`,
      );
    } else if (line.startsWith("## ")) {
      html.push(
        `<h2 class="mt-6 mb-2 text-lg font-medium tracking-tight">${inline(line.slice(3))}</h2>`,
      );
    } else if (line.startsWith("# ")) {
      html.push(
        `<h1 class="mt-6 mb-3 text-xl font-medium tracking-tight">${inline(line.slice(2))}</h1>`,
      );
    } else if (line.startsWith("> ")) {
      html.push(
        `<blockquote class="my-3 border-l-2 border-border pl-3 text-muted-foreground">${inline(line.slice(2))}</blockquote>`,
      );
    } else {
      html.push(`<p class="my-2 leading-relaxed text-pretty">${inline(line)}</p>`);
    }
  }
  flushCode();
  flushList();
  return html.join("\n");
}
