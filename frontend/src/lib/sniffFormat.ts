import type { PasteFormat } from "../api/types";

const FENCE =
  /^```(?:json|js|javascript|ts|tsx|py|python|rs|rust|go|bash|sh|yaml|yml|sql|html|css|md)?/u;

export const sniffFormatFromText = (text: string): PasteFormat | null => {
  const raw = text.trim();
  if (raw.length < 8) return null;

  const first = raw.split(/\r?\n/u, 1)[0] ?? "";

  if (first.startsWith("#!") && /bash|zsh|sh/u.test(first)) return "bash";
  if (first.startsWith("#!") && /python/u.test(first)) return "python";
  if (first.startsWith("#!") && /node/u.test(first)) return "javascript";

  if (FENCE.test(first) || /^#{1,6}\s+\S/u.test(first) || /\[[^\]]+\]\([^)]+\)/u.test(raw)) {
    if (/^```json/u.test(first)) return "json";
    if (/^```(?:ts|tsx|typescript)/u.test(first)) return "typescript";
    if (/^```(?:js|javascript)/u.test(first)) return "javascript";
    if (/^```(?:py|python)/u.test(first)) return "python";
    if (/^```(?:rs|rust)/u.test(first)) return "rust";
    if (/^```go/u.test(first)) return "go";
    if (/^```(?:ya?ml)/u.test(first)) return "yaml";
    if (/^```sql/u.test(first)) return "sql";
    if (/^```html/u.test(first)) return "html";
    if (/^```css/u.test(first)) return "css";
    if (/^```(?:sh|bash)/u.test(first)) return "bash";
    return "markdown";
  }

  if (first.startsWith("{") || first.startsWith("[")) {
    try {
      JSON.parse(raw);
      return "json";
    } catch {
      /* not json */
    }
  }

  if (/^(\s*fn\s+\w+|\s*pub\s+(fn|struct|enum)|use\s+[a-z_]+::)/mu.test(raw)) return "rust";
  if (/^(\s*def\s+\w+\s*\(|\s*async\s+def\s+|from\s+\w+\s+import\s+|import\s+\w+)/mu.test(raw)) {
    return "python";
  }
  if (/^(\s*package\s+\w+|\s*func\s+\w+\s*\()/mu.test(raw)) return "go";
  if (/^(\s*interface\s+\w+|\s*type\s+\w+\s*=|\s*export\s+(const|function|type))/mu.test(raw)) {
    return "typescript";
  }
  if (/^(\s*function\s+\w+|\s*const\s+\w+\s*=\s*\(|\s*console\.log\()/mu.test(raw)) {
    return "javascript";
  }
  if (/^(\s*SELECT\s+.+\s+FROM\s+|\s*CREATE\s+TABLE\s+)/imu.test(raw)) return "sql";
  if (/^(\s*[\w-]+\s*:\s*.+\n){2,}/u.test(raw) && !raw.includes("{")) return "yaml";
  if (/^\s*</u.test(raw) && /<\/[a-z]+>/iu.test(raw)) return "html";
  if (/^\s*[.#a-z][\w-]*\s*\{/u.test(raw)) return "css";

  return null;
};
