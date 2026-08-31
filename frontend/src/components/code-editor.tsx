import { useMemo, useRef, type DragEvent, type KeyboardEvent } from "react";
import { cn } from "@/lib/utils";

type Props = {
  id?: string;
  value: string;
  onChange?: (value: string) => void;
  readOnly?: boolean;
  placeholder?: string;
  hint?: string;
  className?: string;
  onFiles?: (files: FileList) => void;
  dragging?: boolean;
  onDraggingChange?: (dragging: boolean) => void;
};

const MIN_GUTTER_LINES = 48;

function insertTab(value: string, start: number, end: number, shift: boolean) {
  if (shift) {
    const lineStart = value.lastIndexOf("\n", Math.max(0, start - 1)) + 1;
    const prefix = value.slice(lineStart, lineStart + 1);
    if (prefix === "\t") {
      return {
        next: value.slice(0, lineStart) + value.slice(lineStart + 1),
        cursor: Math.max(lineStart, start - 1),
      };
    }
    if (value.slice(lineStart, lineStart + 2) === "  ") {
      return {
        next: value.slice(0, lineStart) + value.slice(lineStart + 2),
        cursor: Math.max(lineStart, start - 2),
      };
    }
    return { next: value, cursor: start };
  }
  return {
    next: `${value.slice(0, start)}\t${value.slice(end)}`,
    cursor: start + 1,
  };
}

export function CodeEditor({
  id = "content",
  value,
  onChange,
  readOnly,
  placeholder,
  hint,
  className,
  onFiles,
  dragging = false,
  onDraggingChange,
}: Props) {
  const areaRef = useRef<HTMLTextAreaElement>(null);
  const gutterRef = useRef<HTMLDivElement>(null);
  const lineCount = useMemo(
    () => Math.max(value.split("\n").length, MIN_GUTTER_LINES),
    [value],
  );
  const gutter = useMemo(
    () => Array.from({ length: lineCount }, (_, index) => index + 1).join("\n"),
    [lineCount],
  );

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Tab" || event.metaKey || event.ctrlKey || event.altKey) return;
    if (!onChange || readOnly) return;
    event.preventDefault();
    const area = event.currentTarget;
    const start = area.selectionStart;
    const end = area.selectionEnd;
    const { next, cursor } = insertTab(value, start, end, event.shiftKey);
    if (next === value) return;
    onChange(next);
    requestAnimationFrame(() => {
      area.selectionStart = cursor;
      area.selectionEnd = cursor;
    });
  };

  const setDrag = (next: boolean) => onDraggingChange?.(next);

  const onDragOver = (event: DragEvent<HTMLDivElement>) => {
    if (!onFiles) return;
    event.preventDefault();
    setDrag(true);
  };

  const onDrop = (event: DragEvent<HTMLDivElement>) => {
    if (!onFiles) return;
    event.preventDefault();
    setDrag(false);
    if (event.dataTransfer.files.length) onFiles(event.dataTransfer.files);
  };

  const syncScroll = () => {
    if (gutterRef.current && areaRef.current) {
      gutterRef.current.scrollTop = areaRef.current.scrollTop;
    }
  };

  return (
    <div
      className={cn(
        "relative grid min-h-52 flex-1 grid-cols-1 overflow-hidden bg-background sm:grid-cols-[auto_minmax(0,1fr)]",
        className,
      )}
      onDragEnter={onDragOver}
      onDragOver={onDragOver}
      onDragLeave={(event) => {
        if (event.currentTarget.contains(event.relatedTarget as Node)) return;
        setDrag(false);
      }}
      onDrop={onDrop}
    >
      <div
        ref={gutterRef}
        aria-hidden="true"
        className="hidden select-none overflow-hidden border-r border-border bg-gutter px-3 py-5 text-right font-mono text-2xs leading-7 text-muted-foreground/80 tabular-nums whitespace-pre sm:block"
      >
        {gutter}
      </div>
      <div className="relative min-h-0 min-w-0">
        <label className="sr-only" htmlFor={id}>
          Paste content
        </label>
        <textarea
          ref={areaRef}
          id={id}
          value={value}
          readOnly={readOnly}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          enterKeyHint="enter"
          placeholder={placeholder}
          onScroll={syncScroll}
          onKeyDown={onKeyDown}
          onChange={(event) => onChange?.(event.target.value)}
          className="h-full w-full resize-none overflow-auto bg-transparent px-4 py-4 font-mono text-base leading-7 text-foreground outline-none whitespace-pre-wrap break-words sm:px-5 sm:py-5 sm:whitespace-pre sm:break-normal"
        />
        {!value && hint && (
          <p className="pointer-events-none absolute left-4 top-[calc(1rem+1.75rem)] hidden max-w-[min(24rem,calc(100%-2rem))] font-mono text-xs leading-6 text-muted-foreground/70 sm:left-5 sm:top-[calc(1.25rem+1.75rem)] sm:block">
            {hint}
          </p>
        )}
      </div>
      {dragging && (
        <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center bg-background/80 font-mono text-sm text-foreground">
          Drop a text file
        </div>
      )}
    </div>
  );
}
