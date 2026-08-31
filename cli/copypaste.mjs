#!/usr/bin/env node
/**
 * copypaste — ephemeral paste layer for humans and agents.
 * Protocol: copypaste.v1   Runtime: Node.js >= 22
 *
 *   echo 'hello' | copypaste send --ttl 1h
 *   echo 'hello' | copypaste put --json
 *   copypaste get <id>
 *   copypaste serve --port 8787
 */
import { createServer } from "node:http";
import { chmodSync, mkdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { timingSafeEqual } from "node:crypto";

const PROTOCOL = "copypaste.v1";
const VERSION = "1.0.0";
const MAX_BYTES = 1_048_576;
const MAX_STORED = Math.ceil((MAX_BYTES + 16) * (4 / 3)) + 32;
const DEFAULT_TTL = 1_440;
const MAX_TTL = 43_200;
const ID_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const REJECT_ABOVE = ID_ALPHABET.length * 4;
const FORMATS = new Set([
  "plain_text",
  "markdown",
  "json",
  "html",
  "css",
  "javascript",
  "typescript",
  "python",
  "rust",
  "go",
  "c",
  "cpp",
  "java",
  "shell",
  "sql",
  "yaml",
  "toml",
  "xml",
  "diff",
]);
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function usage(code = 0) {
  const text = `copypaste ${VERSION} — ${PROTOCOL}

Usage:
  copypaste send [file]     Create a paste from a file or stdin (alias: put)
  copypaste get <id>        Fetch a paste as JSON (alias: fetch)
  copypaste raw <id>        Fetch plaintext to stdout (alias: cat)
  copypaste health          Probe a host
  copypaste serve           Run a local ephemeral store (API-only)
  copypaste spec            Print the protocol document
  copypaste tools           Print function-calling tool schema
  copypaste doctor          Check Node, sqlite, and this binary
  copypaste version         Print version

Piped stdin with no command defaults to send.

Send flags:
  --host URL          Server origin (env COPYPASTE_HOST / COPYPASTE_ORIGIN)
  --origin URL        Alias of --host
  --ttl 1h            Retention: 1m, 10m, 1h, 3h, 1d, 7d, 30d, or minutes
  --burn              Delete after first successful read
  --encrypt           AES-256-GCM in this process; key stays off the wire
  --key SECRET        Encryption secret (or COPYPASTE_KEY)
  --key-file PATH     Encryption secret from a file
  --format NAME       plain_text, markdown, json, rust, python, ...
  --token TOKEN       Write admission token (or COPYPASTE_WRITE_TOKEN)
  --token-file PATH   Write token from a file
  --json              Machine-readable stdout
  --stdin             Read body from stdin even if a file is given

Get / raw flags:
  --host URL  --key SECRET  --key-file PATH  --json  --token TOKEN

Serve flags:
  --port 8787         Listen port
  --bind 127.0.0.1    Bind address (use 0.0.0.0 to expose)
  --origin URL        Public origin used in returned URLs
  --data-dir PATH     SQLite directory (env COPYPASTE_DATA_DIR, default ~/.copypaste)
  --token TOKEN       Require write token
  --ttl 1d            Default TTL for creates without one

Ethics: no listing, no binaries, 1 MiB cap, default TTL 24h, optional write token.
Do not use this as malware, credential-dump, or PII warehousing infrastructure.
`;
  process.stdout.write(text);
  process.exit(code);
}

function args(argv) {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === "--") {
      out._.push(...argv.slice(i + 1));
      break;
    }
    if (token.startsWith("--")) {
      const key = token.slice(2);
      const next = argv[i + 1];
      if (next == null || next.startsWith("--")) out[key] = true;
      else {
        out[key] = next;
        i += 1;
      }
    } else out._.push(token);
  }
  return out;
}

function parseTtl(value) {
  if (value == null || value === true) return DEFAULT_TTL;
  const map = { "1m": 1, "10m": 10, "1h": 60, "3h": 180, "1d": 1440, "7d": 10080, "30d": 43200 };
  if (map[String(value)]) return map[String(value)];
  const n = Number(value);
  if (!Number.isFinite(n) || n < 0 || n > MAX_TTL) throw new Error("invalid --ttl");
  return Math.floor(n);
}

function defaultHost() {
  return process.env.COPYPASTE_HOST || process.env.COPYPASTE_ORIGIN || "http://127.0.0.1:8080";
}

function hostOf(flags) {
  const raw = flags.origin || flags.host || defaultHost();
  return String(raw === true ? defaultHost() : raw).replace(/\/$/, "");
}

function warnInsecure(host) {
  try {
    const url = new URL(host);
    const local = url.hostname === "127.0.0.1" || url.hostname === "localhost" || url.hostname === "::1";
    if (url.protocol === "http:" && !local) {
      process.stderr.write("copypaste: warning: non-TLS remote host\n");
    }
  } catch {
    /* ignore */
  }
}

function readSecret(flags, flag, fileFlag, envName) {
  const file = flags[fileFlag];
  if (file && file !== true) return readFileSync(file, "utf8").trim();
  const direct = flags[flag];
  if (direct && direct !== true) return String(direct);
  return process.env[envName] || "";
}

function generateId() {
  const out = [];
  const bytes = new Uint8Array(32);
  while (out.length < 24) {
    crypto.getRandomValues(bytes);
    for (const byte of bytes) {
      if (byte >= REJECT_ABOVE) continue;
      out.push(ID_ALPHABET[byte % ID_ALPHABET.length]);
      if (out.length === 24) break;
    }
  }
  return out.join("");
}

function bytesToB64url(bytes) {
  const chunk = 0x8000;
  let binary = "";
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function b64urlToBytes(value) {
  const padded = value.length % 4 === 0 ? value : value + "=".repeat(4 - (value.length % 4));
  const binary = atob(padded.replace(/-/g, "+").replace(/_/g, "/"));
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

async function deriveKey(secret, salt) {
  const material = await crypto.subtle.importKey("raw", encoder.encode(secret), "PBKDF2", false, ["deriveKey"]);
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", salt, iterations: 210_000, hash: "SHA-256" },
    material,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

async function encryptPaste(plaintext, secret) {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveKey(secret, salt);
  const ciphertext = await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce }, key, encoder.encode(plaintext));
  return {
    content: bytesToB64url(new Uint8Array(ciphertext)),
    salt: bytesToB64url(salt),
    nonce: bytesToB64url(nonce),
    algorithm: "aes256_gcm",
  };
}

async function decryptPaste(payload, secret) {
  const key = await deriveKey(secret, b64urlToBytes(payload.salt));
  const plain = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: b64urlToBytes(payload.nonce) },
    key,
    b64urlToBytes(payload.content),
  );
  return decoder.decode(plain);
}

function generateKey() {
  return bytesToB64url(crypto.getRandomValues(new Uint8Array(32)));
}

function looksBinary(bytes) {
  const sample = bytes.subarray(0, Math.min(bytes.length, 1024));
  let nulls = 0;
  for (const value of sample) if (value === 0) nulls += 1;
  return nulls > 8;
}

function blockedIds() {
  const raw = process.env.COPYPASTE_BLOCKED_PASTE_IDS || "";
  return new Set(
    raw
      .split(",")
      .map((id) => id.trim())
      .filter(Boolean),
  );
}

function toolsDocument() {
  return {
    protocol: PROTOCOL,
    version: VERSION,
    description: "Function-calling tools for ephemeral text pastes. No listing. Decrypt only on the client.",
    constraints: {
      maxBytes: MAX_BYTES,
      defaultTtlMinutes: DEFAULT_TTL,
      maxTtlMinutes: MAX_TTL,
      binaryUploads: false,
      listing: false,
    },
    tools: [
      {
        name: "copypaste_send",
        description: "Create an ephemeral paste. Returns id, url, raw, expiresAt. Never send encryption keys in the body.",
        method: "POST",
        path: "/api/v1/pastes",
        parameters: {
          type: "object",
          required: ["content"],
          properties: {
            content: { type: "string", description: "UTF-8 paste body, max 1 MiB" },
            format: { type: "string", default: "plain_text" },
            retentionMinutes: { type: "integer", minimum: 1, maximum: MAX_TTL, default: DEFAULT_TTL },
            burnAfterReading: { type: "boolean", default: false },
            encrypted: { type: "boolean", default: false },
            algorithm: { type: "string", nullable: true },
            salt: { type: "string", nullable: true },
            nonce: { type: "string", nullable: true },
          },
        },
      },
      {
        name: "copypaste_get",
        description: "Fetch a paste as JSON. A burn-after-reading paste is consumed on success.",
        method: "GET",
        path: "/api/v1/pastes/{id}",
      },
      {
        name: "copypaste_raw",
        description: "Fetch unencrypted plaintext. Encrypted pastes return 409 — use copypaste_get and decrypt locally.",
        method: "GET",
        path: "/api/v1/pastes/{id}/raw",
      },
    ],
  };
}

function discoveryDocument(origin = "") {
  const host = origin || "$HOST";
  return {
    name: "copypaste",
    protocol: PROTOCOL,
    version: VERSION,
    description: "Ephemeral paste layer for humans and agents. Client-side AES-256-GCM. No directory listing.",
    maxBytes: MAX_BYTES,
    defaultTtlMinutes: DEFAULT_TTL,
    maxTtlMinutes: MAX_TTL,
    features: ["encryption.aes256_gcm", "burn_after_reading", "ttl", "raw", "no_listing"],
    ethics: {
      listing: false,
      binaryUploads: false,
      defaultTtl: true,
      writeAdmission: "optional bearer / X-Write-Token / X-CopyPaste-Write-Token",
    },
    endpoints: {
      spec: `${origin}/api/v1`,
      health: `${origin}/health`,
      create: `${origin}/api/v1/pastes`,
      get: `${origin}/api/v1/pastes/{id}`,
      raw: `${origin}/api/v1/pastes/{id}/raw`,
      openapi: `${origin}/openapi.yaml`,
      llms: `${origin}/llms.txt`,
      tools: `${origin}/tools.json`,
    },
    agent: {
      send: `echo BODY | copypaste send --host ${host} --ttl 1h --json`,
      put: `echo BODY | copypaste put --host ${host} --json`,
      get: `copypaste get PASTE_ID --host ${host} --json`,
    },
  };
}

function llmsText(origin = "$ORIGIN") {
  return `# copypaste

> Ephemeral paste layer for humans and AI agents. Protocol ${PROTOCOL}. Client-side AES-256-GCM. No directory listing.

Prefer the HTTP API or the copypaste CLI. Do not scrape HTML.

## Use it

\`\`\`bash
echo 'body' | copypaste send --host ${origin} --ttl 1h --json
copypaste get $ID --host ${origin} --json
\`\`\`

## Protocol

- GET /api/v1 — discovery document
- GET /health — liveness
- POST /api/v1/pastes — create
- GET /api/v1/pastes/{id} — JSON fetch (consumes burn-after-reading)
- GET /api/v1/pastes/{id}/raw — text/plain for unencrypted pastes
- GET /tools.json — function-calling schema
- GET /llms.txt — this document
- GET /openapi.yaml — OpenAPI 3
- Optional write admission: Authorization: Bearer, X-Write-Token, or X-CopyPaste-Write-Token

Default TTL is 24 hours. Maximum size is 1 MiB. No listing. No binaries.
Decrypt only on the client. Do not store malware, credential dumps, or bulk PII.
`;
}

function openApiYaml() {
  return `openapi: 3.0.3
info:
  title: copypaste.v1
  version: "${VERSION}"
  description: Ephemeral paste layer. No listing. Client-side encryption.
paths:
  /health:
    get:
      summary: Liveness
  /api/v1:
    get:
      summary: Discovery document
  /api/v1/pastes:
    post:
      summary: Create a paste
  /api/v1/pastes/{id}:
    get:
      summary: Fetch paste JSON
  /api/v1/pastes/{id}/raw:
    get:
      summary: Fetch unencrypted plaintext
`;
}

async function readBody(flags) {
  const file = flags._[1];
  if (file && flags.stdin !== true) {
    const buf = readFileSync(file);
    if (looksBinary(buf)) throw new Error("binary files are not accepted");
    return decoder.decode(buf);
  }
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  const buf = Buffer.concat(chunks.map((c) => Buffer.from(c)));
  if (looksBinary(buf)) throw new Error("binary stdin is not accepted");
  return decoder.decode(buf);
}

function headers(flags, jsonBody = true) {
  const out = {
    "x-copypaste-protocol": PROTOCOL,
    accept: "application/json",
    "user-agent": `copypaste/${VERSION}`,
  };
  if (jsonBody) out["content-type"] = "application/json";
  const token = readSecret(flags, "token", "token-file", "COPYPASTE_WRITE_TOKEN");
  if (token) {
    out.authorization = `Bearer ${token}`;
    out["x-write-token"] = token;
  }
  return out;
}

async function api(method, path, flags, body) {
  const host = hostOf(flags);
  warnInsecure(host);
  const res = await fetch(`${host}${path}`, {
    method,
    headers: headers(flags, body !== undefined),
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  const textBody = await res.text();
  let parsed = textBody;
  try {
    parsed = JSON.parse(textBody);
  } catch {
    /* raw */
  }
  return { res, body: parsed, text: textBody };
}

function emit(flags, value, fallback) {
  if (flags.json) process.stdout.write(`${JSON.stringify(value, null, 2)}\n`);
  else process.stdout.write(`${fallback}\n`);
}

async function cmdSend(flags) {
  const content = await readBody(flags);
  if (!content.trim()) throw new Error("content is required (pipe stdin or pass a file)");
  if (encoder.encode(content).byteLength > MAX_BYTES) throw new Error("paste exceeds 1 MiB");

  let key = readSecret(flags, "key", "key-file", "COPYPASTE_KEY");
  const encrypted = Boolean(flags.encrypt) || Boolean(key);
  let algorithm = null;
  let salt = null;
  let nonce = null;
  let payload = content;
  if (encrypted) {
    if (!key) key = generateKey();
    const box = await encryptPaste(content, key);
    payload = box.content;
    algorithm = box.algorithm;
    salt = box.salt;
    nonce = box.nonce;
  }

  const format = flags.format && flags.format !== true && FORMATS.has(String(flags.format))
    ? String(flags.format)
    : "plain_text";

  const { res, body } = await api("POST", "/api/v1/pastes", flags, {
    content: payload,
    format,
    encrypted,
    algorithm,
    salt,
    nonce,
    burnAfterReading: Boolean(flags.burn),
    retentionMinutes: parseTtl(flags.ttl),
  });
  if (!res.ok) {
    throw new Error(typeof body === "object" ? body.message || body.error || res.statusText : body);
  }
  const url = key ? `${body.url}#${encodeURIComponent(key)}` : body.url;
  emit(flags, { ...body, url, key: key || undefined, protocol: PROTOCOL }, url);
}

async function cmdGet(flags) {
  const id = flags._[1];
  if (!id) throw new Error("usage: copypaste get <id>");
  const { res, body } = await api("GET", `/api/v1/pastes/${id}`, flags);
  if (!res.ok) throw new Error(typeof body === "object" ? body.status || body.error : body);
  const key = readSecret(flags, "key", "key-file", "COPYPASTE_KEY");
  if (body.paste?.encrypted && key) {
    body.paste.content = await decryptPaste(body.paste, key);
    body.paste.decrypted = true;
  }
  emit(flags, body, body.paste?.content ?? JSON.stringify(body));
}

async function cmdRaw(flags) {
  const id = flags._[1];
  if (!id) throw new Error("usage: copypaste raw <id>");
  const host = hostOf(flags);
  warnInsecure(host);
  const res = await fetch(`${host}/api/v1/pastes/${id}/raw`, { headers: headers(flags, false) });
  if (res.status === 409) {
    flags.json = true;
    await cmdGet(flags);
    return;
  }
  const textBody = await res.text();
  if (!res.ok) throw new Error(textBody.trim() || res.statusText);
  process.stdout.write(textBody.endsWith("\n") ? textBody : `${textBody}\n`);
}

async function cmdHealth(flags) {
  const { res, body } = await api("GET", "/health", flags);
  if (!res.ok) throw new Error("unhealthy");
  emit(flags, body, body.ok ? `${body.protocol} ok` : "fail");
}

function openDb(dir, DatabaseSync) {
  mkdirSync(dir, { recursive: true, mode: 0o700 });
  const path = join(dir, "pastes.sqlite");
  const db = new DatabaseSync(path);
  db.exec(`
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = NORMAL;
    PRAGMA busy_timeout = 5000;
    PRAGMA foreign_keys = ON;
    PRAGMA temp_store = MEMORY;
    create table if not exists pastes (
      id text primary key,
      content text not null,
      format text not null,
      encrypted integer not null default 0,
      algorithm text,
      salt text,
      nonce text,
      burn_after_reading integer not null default 0,
      burned integer not null default 0,
      retention_minutes integer,
      expires_at text,
      created_at text not null,
      view_count integer not null default 0
    );
    create index if not exists pastes_expires_at_idx on pastes (expires_at);
  `);
  try {
    chmodSync(path, 0o600);
  } catch {
    /* windows */
  }
  return db;
}

function serveInsert(db, data, defaultTtl, blocked) {
  const bytes = encoder.encode(data.content);
  if (!data.content) throw Object.assign(new Error("content required"), { status: 400 });
  if (!data.encrypted && looksBinary(bytes)) throw Object.assign(new Error("binary rejected"), { status: 400 });
  if (bytes.byteLength > (data.encrypted ? MAX_STORED : MAX_BYTES)) {
    throw Object.assign(new Error("too large"), { status: 413 });
  }
  const minutes = data.retentionMinutes == null ? defaultTtl : Number(data.retentionMinutes);
  if (!Number.isFinite(minutes) || minutes < 0 || minutes > MAX_TTL) {
    throw Object.assign(new Error("invalid ttl"), { status: 400 });
  }
  const now = new Date().toISOString();
  const expiresAt = minutes > 0 ? new Date(Date.now() + minutes * 60_000).toISOString() : null;
  const format = FORMATS.has(data.format) ? data.format : "plain_text";
  let id = "";
  for (let attempt = 0; attempt < 8; attempt += 1) {
    id = generateId();
    if (!blocked.has(id)) break;
  }
  if (blocked.has(id)) throw Object.assign(new Error("unable to allocate id"), { status: 500 });
  db.prepare(
    `insert into pastes (id, content, format, encrypted, algorithm, salt, nonce,
      burn_after_reading, burned, retention_minutes, expires_at, created_at, view_count)
     values (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, 0)`,
  ).run(
    id,
    data.content,
    format,
    data.encrypted ? 1 : 0,
    data.algorithm ?? null,
    data.salt ?? null,
    data.nonce ?? null,
    data.burnAfterReading ? 1 : 0,
    minutes || null,
    expiresAt,
    now,
  );
  return { id, expiresAt };
}

function serveRead(db, id, blocked) {
  if (blocked.has(id)) return { status: "blocked" };
  const now = new Date().toISOString();
  db.exec("begin immediate");
  try {
    const row = db
      .prepare(
        `update pastes
         set view_count = view_count + 1,
             burned = case when burn_after_reading then 1 else burned end
         where id = ?
           and burned = 0
           and (expires_at is null or expires_at > ?)
         returning *`,
      )
      .get(id, now);
    if (row) {
      if (row.burn_after_reading) {
        db.prepare("update pastes set content = '' where id = ?").run(id);
      }
      db.exec("commit");
      return {
        status: "ok",
        paste: {
          id: row.id,
          content: row.content,
          format: row.format,
          encrypted: Boolean(row.encrypted),
          algorithm: row.algorithm,
          salt: row.salt,
          nonce: row.nonce,
          burnAfterReading: Boolean(row.burn_after_reading),
          createdAt: row.created_at,
          expiresAt: row.expires_at,
          viewCount: row.view_count,
        },
      };
    }
    const found = db.prepare("select burned, expires_at from pastes where id = ?").get(id);
    if (found?.expires_at && Date.parse(found.expires_at) <= Date.now()) {
      db.prepare("delete from pastes where id = ?").run(id);
      db.exec("commit");
      return { status: "expired" };
    }
    db.exec("commit");
    if (!found) return { status: "not_found" };
    if (found.burned) return { status: "burned" };
    return { status: "not_found" };
  } catch (error) {
    db.exec("rollback");
    throw error;
  }
}

function sweepExpired(db) {
  db.prepare("delete from pastes where expires_at is not null and expires_at <= ?").run(
    new Date().toISOString(),
  );
}

const CORS = {
  "access-control-allow-origin": "*",
  "access-control-allow-headers":
    "content-type, authorization, x-write-token, x-copypaste-write-token",
  "access-control-allow-methods": "GET, POST, OPTIONS",
};

function sendJson(res, status, obj) {
  const payload = JSON.stringify(obj);
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "x-copypaste-protocol": PROTOCOL,
    "cache-control": "no-store",
    ...CORS,
  });
  res.end(payload);
}

function sendText(res, status, body, type = "text/plain; charset=utf-8") {
  res.writeHead(status, {
    "content-type": type,
    "x-copypaste-protocol": PROTOCOL,
    "cache-control": "no-store",
    ...CORS,
  });
  res.end(body);
}

function checkToken(req, token) {
  if (!token) return true;
  const auth = req.headers.authorization?.replace(/^Bearer\s+/i, "") || "";
  const header = String(req.headers["x-write-token"] || req.headers["x-copypaste-write-token"] || "");
  return safeEqual(auth, token) || safeEqual(header, token);
}

function safeEqual(a, b) {
  const left = Buffer.from(String(a));
  const right = Buffer.from(String(b));
  if (left.length !== right.length) {
    timingSafeEqual(right, right);
    return false;
  }
  return timingSafeEqual(left, right);
}

async function cmdServe(flags) {
  const { DatabaseSync } = await import("node:sqlite");
  const port = Number(flags.port || process.env.PORT || 8787);
  const bind = String(flags.bind || "127.0.0.1");
  const dir = String(flags["data-dir"] || process.env.COPYPASTE_DATA_DIR || join(homedir(), ".copypaste"));
  const token = readSecret(flags, "token", "token-file", "COPYPASTE_WRITE_TOKEN");
  const defaultTtl = parseTtl(flags.ttl);
  const publicOrigin = String(
    flags.origin && flags.origin !== true
      ? flags.origin
      : process.env.COPYPASTE_PUBLIC_ORIGIN || `http://${bind === "0.0.0.0" ? "127.0.0.1" : bind}:${port}`,
  ).replace(/\/$/, "");
  const blocked = blockedIds();
  const db = openDb(dir, DatabaseSync);
  sweepExpired(db);
  const sweeper = setInterval(() => {
    try {
      sweepExpired(db);
    } catch {
      /* ignore */
    }
  }, 5 * 60_000);
  if (typeof sweeper.unref === "function") sweeper.unref();

  const buckets = new Map();
  const allow = (ip, kind) => {
    const now = Date.now();
    const key = ip || "local";
    let row = buckets.get(key);
    if (!row || now - row.start >= 10 * 60_000) {
      row = { start: now, creates: 0, reads: 0 };
      buckets.set(key, row);
    }
    if (kind === "create") {
      if (row.creates >= 40) return false;
      row.creates += 1;
      return true;
    }
    if (row.reads >= 200) return false;
    row.reads += 1;
    return true;
  };

  const server = createServer(async (req, res) => {
    const url = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`);
    if (req.method === "OPTIONS") {
      res.writeHead(204, CORS);
      res.end();
      return;
    }
    try {
      if (req.method === "GET" && (url.pathname === "/health" || url.pathname === "/api/v1/health")) {
        sendJson(res, 200, { ok: true, protocol: PROTOCOL, version: VERSION, mode: "cli-serve" });
        return;
      }
      if (req.method === "GET" && url.pathname === "/api/v1") {
        sendJson(res, 200, { ...discoveryDocument(publicOrigin), mode: "cli-serve" });
        return;
      }
      if (req.method === "GET" && url.pathname === "/tools.json") {
        sendJson(res, 200, toolsDocument());
        return;
      }
      if (req.method === "GET" && url.pathname === "/llms.txt") {
        sendText(res, 200, llmsText(publicOrigin));
        return;
      }
      if (req.method === "GET" && url.pathname === "/openapi.yaml") {
        sendText(res, 200, openApiYaml(), "text/yaml; charset=utf-8");
        return;
      }
      if (req.method === "POST" && url.pathname === "/api/v1/pastes") {
        if (!allow(req.socket.remoteAddress, "create")) {
          sendJson(res, 429, { error: "rate_limited" });
          return;
        }
        if (!checkToken(req, token)) {
          sendJson(res, 401, { error: "write_token_required" });
          return;
        }
        const chunks = [];
        for await (const chunk of req) chunks.push(chunk);
        const raw = Buffer.concat(chunks).toString("utf8");
        let body;
        try {
          body = JSON.parse(raw);
        } catch {
          body = { content: raw };
        }
        const created = serveInsert(
          db,
          {
            content: body.content,
            format: body.format,
            encrypted: Boolean(body.encrypted),
            algorithm: body.algorithm,
            salt: body.salt,
            nonce: body.nonce,
            burnAfterReading: Boolean(body.burnAfterReading ?? body.burn),
            retentionMinutes: body.retentionMinutes ?? body.ttl,
          },
          defaultTtl,
          blocked,
        );
        sweepExpired(db);
        sendJson(res, 201, {
          id: created.id,
          url: `${publicOrigin}/p/${created.id}`,
          raw: `${publicOrigin}/api/v1/pastes/${created.id}/raw`,
          expiresAt: created.expiresAt,
        });
        return;
      }
      const match = url.pathname.match(/^\/api\/v1\/pastes\/([A-Za-z0-9]{24})(\/raw)?$/);
      if (req.method === "GET" && match) {
        if (!allow(req.socket.remoteAddress, "read")) {
          sendJson(res, 429, { error: "rate_limited" });
          return;
        }
        const result = serveRead(db, match[1], blocked);
        if (result.status !== "ok") {
          const status = result.status === "not_found" || result.status === "blocked" ? 404 : 410;
          if (match[2]) {
            sendText(res, status, `${result.status}\n`);
            return;
          }
          sendJson(res, status, result);
          return;
        }
        if (match[2]) {
          if (result.paste.encrypted) {
            sendJson(res, 409, { status: "encrypted", id: result.paste.id });
            return;
          }
          sendText(res, 200, result.paste.content);
          return;
        }
        sendJson(res, 200, result);
        return;
      }
      sendJson(res, 404, { error: "not_found" });
    } catch (error) {
      sendJson(res, error.status || 500, { error: error.message || "fail" });
    }
  });

  server.listen(port, bind, () => {
    process.stderr.write(`copypaste serve ${PROTOCOL} on http://${bind}:${port}  origin=${publicOrigin}  data=${dir}\n`);
  });
}

function cmdSpec() {
  process.stdout.write(`${JSON.stringify(discoveryDocument(""), null, 2)}\n`);
}

function cmdTools() {
  process.stdout.write(`${JSON.stringify(toolsDocument(), null, 2)}\n`);
}

async function cmdDoctor() {
  const major = Number(process.versions.node.split(".")[0]);
  const report = {
    ok: true,
    protocol: PROTOCOL,
    version: VERSION,
    node: process.versions.node,
    nodeOk: major >= 22,
    sqlite: false,
    arch: process.arch,
    platform: process.platform,
  };
  try {
    await import("node:sqlite");
    report.sqlite = true;
  } catch (error) {
    report.ok = false;
    report.sqliteError = error.message;
  }
  if (!report.nodeOk) report.ok = false;
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  if (!report.ok) process.exit(1);
}

const flags = args(process.argv.slice(2));
let command = flags._[0];

if (flags.help || flags.h) usage(0);
if (!command && !process.stdin.isTTY) command = "send";
if (!command) usage(1);

const commands = {
  send: cmdSend,
  put: cmdSend,
  get: cmdGet,
  fetch: cmdGet,
  raw: cmdRaw,
  cat: cmdRaw,
  health: cmdHealth,
  serve: cmdServe,
  spec: cmdSpec,
  tools: cmdTools,
  doctor: cmdDoctor,
  version: () => process.stdout.write(`copypaste ${VERSION} ${PROTOCOL}\n`),
};

try {
  const fn = commands[command];
  if (!fn) usage(1);
  const result = fn(flags);
  if (result && typeof result.then === "function") {
    result.catch((error) => {
      process.stderr.write(`copypaste: ${error.message}\n`);
      process.exit(1);
    });
  }
} catch (error) {
  process.stderr.write(`copypaste: ${error.message}\n`);
  process.exit(1);
}
