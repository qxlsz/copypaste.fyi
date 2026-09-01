import { ChevronDown } from "lucide-react";
import { toast } from "sonner";

import {
  GROK_BOT_ADD_PROMPT,
  GROK_BOT_SKILL,
  OPEN_AGENTS,
  grokBotHref,
  openPrompt,
} from "../lib/openAgents";

export const OpenWithAgents = ({ url }: { url: string }) => {
  const prompt = openPrompt(url);

  const handleAddToGrok = async () => {
    try {
      await navigator.clipboard.writeText(GROK_BOT_SKILL);
      toast.success("Grok Bot skill copied");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      toast.error("Unable to copy Grok Bot skill", { description: message });
      return;
    }
    window.open(grokBotHref(GROK_BOT_ADD_PROMPT), "_blank", "noopener,noreferrer");
  };

  return (
    <details className="rounded-md bg-muted">
      <summary className="flex h-12 list-none items-center justify-between gap-2 px-3 text-sm text-text sm:h-11 [&::-webkit-details-marker]:hidden">
        <span>Open in Grok, Codex, ChatGPT…</span>
        <ChevronDown className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      </summary>
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 px-3 pb-3 pt-1 text-sm">
        {OPEN_AGENTS.map((agent) => (
          <a
            key={agent.id}
            href={agent.href(prompt)}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex min-h-11 items-center text-text underline-offset-2 hover:underline"
          >
            {agent.label}
          </a>
        ))}
        <button
          type="button"
          onClick={() => void handleAddToGrok()}
          className="inline-flex min-h-11 items-center text-text underline-offset-2 hover:underline"
        >
          Add to Grok
        </button>
      </div>
    </details>
  );
};
