import { useMemo, useState } from "react";
import { Check, Copy } from "lucide-react";
import { toast } from "sonner";

import {
  HOST_GOALS,
  HOST_MACHINES,
  hostRecipe,
  type HostGoal,
  type HostMachine,
} from "../lib/selfHostGuide";

const chip = (active: boolean) =>
  `rounded-md border px-3 py-2 text-sm ${
    active ? "border-text bg-text text-background" : "border-border text-text hover:border-text"
  }`;

export const SelfHostHelper = () => {
  const [goal, setGoal] = useState<HostGoal>("local");
  const [machine, setMachine] = useState<HostMachine>("grok");
  const [copied, setCopied] = useState(false);
  const recipe = useMemo(() => hostRecipe(goal, machine), [goal, machine]);

  const copyCommands = async () => {
    try {
      await navigator.clipboard.writeText(recipe.commands);
      setCopied(true);
      toast.success("Commands copied");
      window.setTimeout(() => setCopied(false), 1500);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Clipboard unavailable";
      toast.error("Unable to copy commands", { description: message });
    }
  };

  return (
    <div className="space-y-4">
      <p className="text-sm leading-relaxed text-muted-foreground">
        Pick where you are and what you want. Only follow the box that appears.
      </p>
      <div className="space-y-2">
        <p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
          Where
        </p>
        <div className="flex flex-wrap gap-2">
          {HOST_MACHINES.map((item) => (
            <button
              key={item.id}
              type="button"
              className={chip(machine === item.id)}
              onClick={() => setMachine(item.id)}
              aria-pressed={machine === item.id}
            >
              {item.label}
            </button>
          ))}
        </div>
      </div>
      <div className="space-y-2">
        <p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
          Goal
        </p>
        <div className="flex flex-wrap gap-2">
          {HOST_GOALS.map((item) => (
            <button
              key={item.id}
              type="button"
              className={chip(goal === item.id)}
              onClick={() => setGoal(item.id)}
              aria-pressed={goal === item.id}
              title={item.hint}
            >
              {item.label}
            </button>
          ))}
        </div>
      </div>
      <p className="text-sm text-text">
        Follow this: <span className="font-medium">{recipe.follow}</span>
      </p>
      <div className="relative">
        <pre className="overflow-x-auto rounded-md border border-border bg-muted/40 px-3 py-3 pr-12 font-mono text-xs leading-6 text-muted-foreground">
          {recipe.commands}
        </pre>
        <button
          type="button"
          onClick={() => void copyCommands()}
          className="absolute right-2 top-2 inline-flex h-8 w-8 items-center justify-center rounded-md border border-border bg-surface text-muted-foreground transition hover:border-text hover:text-text"
          title="Copy commands"
          aria-label="Copy commands"
        >
          {copied ? (
            <Check className="h-3.5 w-3.5" aria-hidden="true" />
          ) : (
            <Copy className="h-3.5 w-3.5" aria-hidden="true" />
          )}
        </button>
      </div>
    </div>
  );
};
