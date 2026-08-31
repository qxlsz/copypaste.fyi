import { createServerFn } from "@tanstack/react-start";
import { z } from "zod";
import { allowRequest } from "@/lib/abuse";
import {
  MAX_STORED_BYTES,
  PASTE_FORMATS,
  PASTE_ID_PATTERN,
} from "@/lib/formats";
import {
  fetchPaste,
  insertPaste,
  type PastePayload,
  type PasteReadResult as StoreReadResult,
} from "@/lib/paste-store";

export type { PastePayload };
export type PasteReadResult = Exclude<StoreReadResult, { status: "blocked" }>;

const FormatSchema = z.enum(
  PASTE_FORMATS.map((item) => item.value) as [string, ...string[]],
);

const CreatePasteSchema = z.object({
  content: z.string().min(1).max(MAX_STORED_BYTES),
  format: FormatSchema,
  encrypted: z.boolean(),
  algorithm: z.string().nullable(),
  salt: z.string().nullable(),
  nonce: z.string().nullable(),
  burnAfterReading: z.boolean(),
  retentionMinutes: z.number().int().min(0).max(43_200),
});

const IdSchema = z.object({
  id: z.string().regex(PASTE_ID_PATTERN),
});

export const createPaste = createServerFn({ method: "POST" })
  .validator((input: unknown) => CreatePasteSchema.parse(input))
  .handler(async ({ data }) => {
    const allowed = allowRequest("ui", "create");
    if (!allowed.ok) throw new Error("Too many pastes from this session. Wait and retry.");
    return insertPaste(data, "ui");
  });

export const readPaste = createServerFn({ method: "POST" })
  .validator((input: unknown) => IdSchema.parse(input))
  .handler(async ({ data }): Promise<PasteReadResult> => {
    const allowed = allowRequest("ui", "read");
    if (!allowed.ok) return { status: "not_found" };
    const result = await fetchPaste(data.id);
    if (result.status === "blocked") return { status: "not_found" };
    return result;
  });
