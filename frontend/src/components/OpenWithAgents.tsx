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
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm">
      <span className="text-muted-foreground">Open with</span>
      {OPEN_AGENTS.map((agent) => (
        <a
          key={agent.id}
          href={agent.href(prompt)}
          target="_blank"
          rel="noopener noreferrer"
          className="text-text underline-offset-2 hover:underline"
        >
          {agent.label}
        </a>
      ))}
      <button
        type="button"
        onClick={() => void handleAddToGrok()}
        className="text-text underline-offset-2 hover:underline"
      >
        Add to Grok
      </button>
    </div>
  );
};
