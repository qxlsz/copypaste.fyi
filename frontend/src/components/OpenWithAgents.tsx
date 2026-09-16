import { toast } from "sonner";

import { ChatGptMark, ClaudeMark, CodexMark, GrokMark } from "./AgentMarks";
import {
  GROK_BOT_ADD_PROMPT,
  GROK_BOT_SKILL,
  OPEN_AGENTS,
  grokBotHref,
  openPrompt,
} from "../lib/openAgents";

const logoButton =
  "inline-flex size-11 items-center justify-center rounded-md text-text transition hover:bg-border focus-visible:outline-none sm:size-10";

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
    <div className="flex flex-wrap items-center gap-1">
      {OPEN_AGENTS.map((agent) => {
        const Mark =
          agent.id === "grok"
            ? GrokMark
            : agent.id === "codex"
              ? CodexMark
              : agent.id === "chatgpt"
                ? ChatGptMark
                : ClaudeMark;
        return (
          <a
            key={agent.id}
            href={agent.href(prompt)}
            target="_blank"
            rel="noopener noreferrer"
            className={logoButton}
            aria-label={agent.label}
            title={`Open in ${agent.label}`}
          >
            <Mark />
          </a>
        );
      })}
      <button
        type="button"
        onClick={() => void handleAddToGrok()}
        className={logoButton}
        aria-label="Add to Grok"
        title="Add to Grok"
      >
        <span className="relative inline-flex">
          <GrokMark />
          <span className="absolute -right-1 -top-1 text-[10px] leading-none">+</span>
        </span>
      </button>
    </div>
  );
};
