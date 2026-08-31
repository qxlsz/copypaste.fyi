import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";

const bin = fileURLToPath(new URL("./copypaste.mjs", import.meta.url));

function run(args, { input = "", env = {}, timeout = 8_000 } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [bin, ...args], {
      env: { ...process.env, ...env },
      stdio: ["pipe", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (c) => stdout.push(c));
    child.stderr.on("data", (c) => stderr.push(c));
    child.on("error", reject);
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error(`timeout: ${args.join(" ")}\n${Buffer.concat(stderr)}`));
    }, timeout);
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({
        code,
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
      });
    });
    if (input) child.stdin.end(input);
    else child.stdin.end();
  });
}

async function waitFor(host, attempts = 50) {
  for (let i = 0; i < attempts; i += 1) {
    try {
      const res = await fetch(`${host}/health`);
      if (res.ok) return;
    } catch {
      /* not up */
    }
    await delay(50);
  }
  throw new Error(`server did not start at ${host}`);
}

function startServe(args, env = {}) {
  return spawn(process.execPath, [bin, "serve", ...args], {
    env: { ...process.env, ...env },
    stdio: ["ignore", "ignore", "pipe"],
  });
}

test("version, spec, tools, doctor speak copypaste.v1", async () => {
  const version = await run(["version"]);
  assert.equal(version.code, 0);
  assert.match(version.stdout, /copypaste\.v1/);
  const spec = await run(["spec"]);
  assert.equal(spec.code, 0);
  const json = JSON.parse(spec.stdout);
  assert.equal(json.protocol, "copypaste.v1");
  assert.equal(json.maxBytes, 1_048_576);
  assert.equal(json.ethics.listing, false);
  assert.match(json.endpoints.tools, /\/tools\.json$/);
  const tools = await run(["tools"]);
  assert.equal(tools.code, 0);
  const schema = JSON.parse(tools.stdout);
  assert.deepEqual(
    schema.tools.map((t) => t.name),
    ["copypaste_send", "copypaste_get", "copypaste_raw"],
  );
  const doctor = await run(["doctor"]);
  assert.equal(doctor.code, 0, doctor.stderr);
  const health = JSON.parse(doctor.stdout);
  assert.equal(health.ok, true);
  assert.equal(health.sqlite, true);
});

test("local serve round-trips send/get/raw, encryption, burn, and agent discovery", async () => {
  const dir = await mkdtemp(join(tmpdir(), "copypaste-"));
  const port = 18000 + Math.floor(Math.random() * 1000);
  const host = `http://127.0.0.1:${port}`;
  const server = startServe(["--port", String(port), "--bind", "127.0.0.1", "--data-dir", dir, "--origin", host]);
  try {
    await waitFor(host);
    const sent = await run(["put", "--host", host, "--ttl", "10m", "--json"], { input: "hello agent\n" });
    assert.equal(sent.code, 0, sent.stderr);
    const created = JSON.parse(sent.stdout);
    assert.equal(created.id.length, 24);
    assert.equal(created.url, `${host}/p/${created.id}`);

    const got = await run(["fetch", created.id, "--origin", host, "--json"]);
    assert.equal(got.code, 0, got.stderr);
    assert.equal(JSON.parse(got.stdout).paste.content, "hello agent\n");

    const raw = await run(["raw", created.id, "--host", host]);
    assert.equal(raw.code, 0, raw.stderr);
    assert.match(raw.stdout, /hello agent/);

    const locked = await run(["send", "--host", host, "--encrypt", "--json"], { input: "secret-note" });
    assert.equal(locked.code, 0, locked.stderr);
    const box = JSON.parse(locked.stdout);
    assert.ok(box.key);
    const opened = await run(["get", box.id, "--host", host, "--key", box.key, "--json"]);
    assert.equal(opened.code, 0, opened.stderr);
    assert.equal(JSON.parse(opened.stdout).paste.content, "secret-note");

    const burned = await run(["send", "--host", host, "--burn", "--ttl", "1h", "--json"], { input: "once-only" });
    assert.equal(burned.code, 0, burned.stderr);
    const burnId = JSON.parse(burned.stdout).id;
    const first = await run(["get", burnId, "--host", host, "--json"]);
    assert.equal(first.code, 0, first.stderr);
    assert.equal(JSON.parse(first.stdout).paste.content, "once-only");
    const second = await run(["get", burnId, "--host", host, "--json"]);
    assert.notEqual(second.code, 0);

    const discovery = await fetch(`${host}/api/v1`);
    assert.equal(discovery.status, 200);
    const doc = await discovery.json();
    assert.equal(doc.protocol, "copypaste.v1");
    assert.equal(doc.endpoints.tools, `${host}/tools.json`);
    const tools = await fetch(`${host}/tools.json`);
    assert.equal(tools.status, 200);
    const llms = await fetch(`${host}/llms.txt`);
    assert.equal(llms.status, 200);
    assert.match(await llms.text(), /copypaste/);
  } finally {
    server.kill("SIGTERM");
    await rm(dir, { recursive: true, force: true });
  }
});

test("write token, binary reject, and default ttl", async () => {
  const dir = await mkdtemp(join(tmpdir(), "copypaste-"));
  const port = 19000 + Math.floor(Math.random() * 1000);
  const host = `http://127.0.0.1:${port}`;
  const server = startServe(
    ["--port", String(port), "--bind", "127.0.0.1", "--data-dir", dir, "--token", "s3cret"],
  );
  try {
    await waitFor(host);
    const denied = await run(["send", "--host", host, "--json"], { input: "nope" });
    assert.notEqual(denied.code, 0);
    const allowed = await run(["send", "--host", host, "--token", "s3cret", "--json"], { input: "ok-body" });
    assert.equal(allowed.code, 0, allowed.stderr);
    const created = JSON.parse(allowed.stdout);
    const meta = await fetch(`${host}/api/v1/pastes/${created.id}`);
    const body = await meta.json();
    assert.ok(body.paste.expiresAt, "default TTL should set expiresAt");

    const binary = await run(["send", "--host", host, "--token", "s3cret", "--json"], {
      input: "\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000\u0000payload",
    });
    assert.notEqual(binary.code, 0);
  } finally {
    server.kill("SIGTERM");
    await rm(dir, { recursive: true, force: true });
  }
});
