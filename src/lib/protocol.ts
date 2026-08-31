export const PROTOCOL = "copypaste.v1" as const;
export const PROTOCOL_VERSION = "1.0.0";
export const DEFAULT_API_TTL_MINUTES = 1_440;
export const MAX_RETENTION_MINUTES = 43_200;
export const RATE_CREATE_LIMIT = 40;
export const RATE_READ_LIMIT = 200;
export const RATE_WINDOW_MS = 10 * 60_000;

export function discoveryDocument(origin = "") {
  const host = origin || "$HOST";
  return {
    name: "copypaste",
    protocol: PROTOCOL,
    version: PROTOCOL_VERSION,
    description:
      "Ephemeral paste layer for humans and agents. Client-side AES-256-GCM. No directory listing.",
    maxBytes: 1_048_576,
    defaultTtlMinutes: DEFAULT_API_TTL_MINUTES,
    maxTtlMinutes: MAX_RETENTION_MINUTES,
    features: [
      "encryption.aes256_gcm",
      "burn_after_reading",
      "ttl",
      "raw",
      "no_listing",
    ],
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
      serve: `copypaste serve --bind 127.0.0.1 --token $COPYPASTE_WRITE_TOKEN`,
    },
  };
}

export const PROTOCOL_HEADERS = {
  "x-copypaste-protocol": PROTOCOL,
  "cache-control": "no-store",
} as const;
