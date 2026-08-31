import { getSql } from "@/lib/db";
import { blockedIds, looksBinary } from "@/lib/abuse";
import {
  MAX_PASTE_BYTES,
  MAX_STORED_BYTES,
  PASTE_FORMATS,
  PASTE_ID_PATTERN,
} from "@/lib/formats";
import { generatePasteId, isUniqueViolation } from "@/lib/paste-id";
import { DEFAULT_API_TTL_MINUTES } from "@/lib/protocol";

const FormatSet = new Set(PASTE_FORMATS.map((item) => item.value));

export type PastePayload = {
  id: string;
  content: string;
  format: string;
  encrypted: boolean;
  algorithm: string | null;
  salt: string | null;
  nonce: string | null;
  burnAfterReading: boolean;
  createdAt: string;
  expiresAt: string | null;
  viewCount: number;
};

export type PasteReadResult =
  | { status: "ok"; paste: PastePayload }
  | { status: "not_found" }
  | { status: "expired" }
  | { status: "burned" }
  | { status: "blocked" };

export type CreatePasteInput = {
  content: string;
  format: string;
  encrypted: boolean;
  algorithm: string | null;
  salt: string | null;
  nonce: string | null;
  burnAfterReading: boolean;
  retentionMinutes?: number | null;
};

type PasteRow = {
  id: string;
  content: string;
  format: string;
  encrypted: boolean | string | number;
  algorithm: string | null;
  salt: string | null;
  nonce: string | null;
  burn_after_reading: boolean | string | number;
  burned: boolean | string | number;
  created_at: string | Date;
  expires_at: string | Date | null;
  view_count: number | string;
};

function asBool(value: boolean | string | number): boolean {
  return value === true || value === "t" || value === "true" || value === 1;
}

function asIso(value: string | Date | null | undefined): string | null {
  if (!value) return null;
  if (value instanceof Date) return value.toISOString();
  return String(value);
}

function toPayload(row: PasteRow, content = row.content): PastePayload {
  return {
    id: row.id,
    content,
    format: row.format,
    encrypted: asBool(row.encrypted),
    algorithm: row.algorithm,
    salt: row.salt,
    nonce: row.nonce,
    burnAfterReading: asBool(row.burn_after_reading),
    createdAt: asIso(row.created_at) ?? new Date().toISOString(),
    expiresAt: asIso(row.expires_at),
    viewCount: Number(row.view_count) || 0,
  };
}

async function sweepExpired() {
  const sql = await getSql();
  const now = new Date().toISOString();
  await sql`
    delete from pastes
    where id in (
      select id from pastes
      where expires_at is not null and expires_at <= ${now}
      limit 25
    )
  `;
}

export function validateCreateInput(data: CreatePasteInput, source: "ui" | "api") {
  if (!data.content) return "Content is required.";
  const bytes = new TextEncoder().encode(data.content);
  if (!data.encrypted && looksBinary(bytes)) {
    return "Binary uploads are not accepted. Paste text.";
  }
  if (bytes.byteLength > MAX_STORED_BYTES) return "Paste exceeds the 1 MiB limit.";
  if (!data.encrypted && bytes.byteLength > MAX_PASTE_BYTES) {
    return "Paste exceeds the 1 MiB limit.";
  }
  if (!FormatSet.has(data.format as (typeof PASTE_FORMATS)[number]["value"])) {
    return "Unknown format.";
  }
  if (data.encrypted && (!data.salt || !data.nonce || !data.algorithm)) {
    return "Encrypted pastes require algorithm, salt, and nonce.";
  }
  if (data.retentionMinutes != null) {
    if (!Number.isInteger(data.retentionMinutes) || data.retentionMinutes < 0 || data.retentionMinutes > 43_200) {
      return "Retention is out of range.";
    }
  } else if (source === "api") {
    data.retentionMinutes = DEFAULT_API_TTL_MINUTES;
  }
  return null;
}

export async function insertPaste(data: CreatePasteInput, source: "ui" | "api" = "ui") {
  const error = validateCreateInput(data, source);
  if (error) throw new Error(error);

  const sql = await getSql();
  await sweepExpired().catch(() => undefined);

  const minutes = data.retentionMinutes ?? (source === "api" ? DEFAULT_API_TTL_MINUTES : 0);
  const expiresAt =
    minutes > 0 ? new Date(Date.now() + minutes * 60_000).toISOString() : null;

  for (let attempt = 0; attempt < 4; attempt += 1) {
    const id = generatePasteId();
    if (blockedIds().has(id)) continue;
    try {
      await sql`
        insert into pastes (
          id, content, format, encrypted, algorithm, salt, nonce,
          burn_after_reading, retention_minutes, expires_at
        ) values (
          ${id},
          ${data.content},
          ${data.format},
          ${data.encrypted},
          ${data.algorithm},
          ${data.salt},
          ${data.nonce},
          ${data.burnAfterReading},
          ${minutes || null},
          ${expiresAt}
        )
      `;
      return {
        id,
        path: `/p/${id}`,
        shareableUrl: `/p/${id}`,
        raw: `/api/v1/pastes/${id}/raw`,
        expiresAt,
      };
    } catch (caught) {
      if (isUniqueViolation(caught) && attempt < 3) continue;
      throw caught;
    }
  }

  throw new Error("Unable to allocate a paste id.");
}

export async function fetchPaste(id: string): Promise<PasteReadResult> {
  if (!PASTE_ID_PATTERN.test(id)) return { status: "not_found" };
  if (blockedIds().has(id)) return { status: "blocked" };

  const sql = await getSql();
  const now = new Date().toISOString();

  const updated = await sql<PasteRow>`
    update pastes
    set
      view_count = view_count + 1,
      burned = case when burn_after_reading then true else burned end
    where id = ${id}
      and burned = false
      and (expires_at is null or expires_at > ${now})
    returning
      id, content, format, encrypted, algorithm, salt, nonce,
      burn_after_reading, burned, created_at, expires_at, view_count
  `;

  const row = updated[0];
  if (row) {
    if (asBool(row.burn_after_reading)) {
      await sql`update pastes set content = '' where id = ${id}`;
    }
    return { status: "ok", paste: toPayload(row, row.content) };
  }

  const existing = await sql<PasteRow>`
    select
      id, content, format, encrypted, algorithm, salt, nonce,
      burn_after_reading, burned, created_at, expires_at, view_count
    from pastes
    where id = ${id}
    limit 1
  `;
  const found = existing[0];
  if (!found) return { status: "not_found" };
  if (asBool(found.burned)) return { status: "burned" };
  const expiresAt = found.expires_at ? new Date(found.expires_at) : null;
  if (expiresAt && expiresAt.getTime() <= Date.now()) {
    await sql`delete from pastes where id = ${id}`;
    return { status: "expired" };
  }
  return { status: "not_found" };
}
