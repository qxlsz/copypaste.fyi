import assert from "node:assert/strict";
import { test } from "node:test";
import {
  byteLength,
  detectFormat,
  formatExtension,
  formatFromFilename,
  MAX_STORED_BYTES,
  MAX_PASTE_BYTES,
} from "./formats.ts";

test("stored size ceiling covers 1 MiB of AES-GCM ciphertext", () => {
  const encoded = Math.ceil((MAX_PASTE_BYTES + 16) * (4 / 3));
  assert.ok(MAX_STORED_BYTES >= encoded);
});

test("byteLength counts utf-8 bytes not code units", () => {
  assert.equal(byteLength("é"), 2);
});

test("detectFormat recognizes common heads", () => {
  assert.equal(detectFormat('{"ok": true, "n": 1}'), "json");
  assert.equal(detectFormat("SELECT id FROM pastes WHERE id = 1"), "sql");
  assert.equal(detectFormat("def greet(name):\n    return name"), "python");
  assert.equal(
    detectFormat("fn main() {\n    let mut x = 1;\n    println!(\"{x}\");\n}"),
    "rust",
  );
  assert.equal(
    detectFormat("package main\nfunc main() {\n  println(\"hi\")\n}"),
    "go",
  );
  assert.equal(
    detectFormat("import x from './x'\nexport default function App() { return x }"),
    "javascript",
  );
  assert.equal(
    detectFormat("import x from './x'\nexport const n: number = 1"),
    "typescript",
  );
  assert.equal(detectFormat("short"), null);
});

test("formatFromFilename maps extensions", () => {
  assert.equal(formatFromFilename("notes.md"), "markdown");
  assert.equal(formatFromFilename("/tmp/app.tsx"), "typescript");
  assert.equal(formatFromFilename("noext"), null);
  assert.equal(formatExtension("rust"), "rs");
});
