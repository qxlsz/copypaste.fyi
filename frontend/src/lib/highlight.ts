const KEYWORDS: Record<string, string[]> = {
  javascript: [
    "break", "case", "catch", "class", "const", "continue", "debugger", "default",
    "delete", "do", "else", "export", "extends", "false", "finally", "for",
    "function", "if", "import", "in", "instanceof", "let", "new", "null",
    "return", "super", "switch", "this", "throw", "true", "try", "typeof",
    "undefined", "var", "void", "while", "with", "yield", "async", "await",
  ],
  typescript: [
    "as", "break", "case", "catch", "class", "const", "continue", "debugger",
    "declare", "default", "delete", "do", "else", "enum", "export", "extends",
    "false", "finally", "for", "function", "if", "implements", "import", "in",
    "instanceof", "interface", "keyof", "let", "new", "null", "private",
    "protected", "public", "readonly", "return", "static", "super", "switch",
    "this", "throw", "true", "try", "type", "typeof", "undefined", "var",
    "void", "while", "with", "yield", "async", "await",
  ],
  python: [
    "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "False", "finally", "for", "from",
    "global", "if", "import", "in", "is", "lambda", "None", "nonlocal", "not",
    "or", "pass", "raise", "return", "True", "try", "while", "with", "yield",
  ],
  rust: [
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
    "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self",
    "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while",
  ],
  go: [
    "break", "case", "chan", "const", "continue", "default", "defer", "else",
    "fallthrough", "for", "func", "go", "goto", "if", "import", "interface",
    "map", "package", "range", "return", "select", "struct", "switch", "type",
    "var", "true", "false", "nil",
  ],
  sql: [
    "select", "from", "where", "and", "or", "insert", "into", "values", "update",
    "set", "delete", "create", "table", "index", "join", "left", "right",
    "inner", "on", "as", "order", "by", "group", "limit", "offset", "not",
    "null", "primary", "key", "unique", "default",
  ],
};

KEYWORDS.java = [...KEYWORDS.javascript, "package", "public", "private", "protected", "static", "final", "void", "int", "new"];
KEYWORDS.kotlin = [...KEYWORDS.java, "fun", "val", "var", "when", "object", "companion", "data"];
KEYWORDS.csharp = [...KEYWORDS.java, "namespace", "using", "var", "async", "await", "get", "set"];
KEYWORDS.cpp = ["auto", "bool", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do", "else", "enum", "false", "for", "if", "inline", "namespace", "new", "nullptr", "private", "public", "return", "sizeof", "static", "struct", "switch", "template", "this", "throw", "true", "try", "typedef", "typename", "using", "virtual", "void", "while"];
KEYWORDS.php = ["echo", "function", "class", "public", "private", "protected", "return", "if", "else", "elseif", "foreach", "for", "while", "true", "false", "null", "new", "use", "namespace"];
KEYWORDS.ruby = ["def", "end", "class", "module", "if", "elsif", "else", "unless", "do", "while", "until", "for", "in", "true", "false", "nil", "return", "yield", "self", "require"];
KEYWORDS.swift = ["func", "let", "var", "if", "else", "guard", "switch", "case", "for", "in", "while", "return", "true", "false", "nil", "struct", "class", "enum", "protocol", "import"];
KEYWORDS.bash = ["if", "then", "else", "fi", "for", "in", "do", "done", "while", "case", "esac", "function", "return", "echo", "export", "local"];
KEYWORDS.css = ["important", "from", "to", "and", "or", "not", "only"];
KEYWORDS.html = ["html", "head", "body", "script", "style", "div", "span"];
KEYWORDS.yaml = ["true", "false", "null", "yes", "no"];

type Kind = "plain" | "keyword" | "string" | "comment" | "number";

export type Token = { kind: Kind; text: string };

const MAX_HIGHLIGHT_CHARS = 4000;
const CASE_INSENSITIVE = new Set(["sql", "yaml"]);

function keywordsFor(format: string): Set<string> {
  const list = KEYWORDS[format] ?? (format === "code" ? KEYWORDS.javascript : []);
  if (CASE_INSENSITIVE.has(format)) {
    return new Set(list.map((word) => word.toLowerCase()));
  }
  return new Set(list);
}

export function tokenizeLine(line: string, format: string): Token[] {
  if (format === "plain_text" || format === "markdown") {
    return [{ kind: "plain", text: line }];
  }
  if (line.length > MAX_HIGHLIGHT_CHARS) {
    return [{ kind: "plain", text: line }];
  }
  if (format === "json") {
    return tokenizeGeneric(line, new Set(["true", "false", "null"]), false);
  }
  return tokenizeGeneric(line, keywordsFor(format), CASE_INSENSITIVE.has(format));
}

function tokenizeGeneric(line: string, keywords: Set<string>, insensitive: boolean): Token[] {
  const tokens: Token[] = [];
  let i = 0;
  while (i < line.length) {
    const rest = line.slice(i);
    if (rest.startsWith("//") || rest.startsWith("#")) {
      tokens.push({ kind: "comment", text: rest });
      break;
    }
    if (rest.startsWith("/*")) {
      const end = rest.indexOf("*/", 2);
      const take = end === -1 ? rest.length : end + 2;
      tokens.push({ kind: "comment", text: rest.slice(0, take) });
      i += take;
      continue;
    }
    const ch = line[i];
    if (ch === '"' || ch === "'" || ch === "`") {
      let j = i + 1;
      while (j < line.length && line[j] !== ch) {
        if (line[j] === "\\") j += 2;
        else j += 1;
      }
      tokens.push({ kind: "string", text: line.slice(i, Math.min(j + 1, line.length)) });
      i = Math.min(j + 1, line.length);
      continue;
    }
    if (/[0-9]/.test(ch)) {
      let j = i + 1;
      while (j < line.length && /[0-9_.xXa-fA-F]/.test(line[j])) j += 1;
      tokens.push({ kind: "number", text: line.slice(i, j) });
      i = j;
      continue;
    }
    if (/[A-Za-z_$]/.test(ch)) {
      let j = i + 1;
      while (j < line.length && /[A-Za-z0-9_$]/.test(line[j])) j += 1;
      const word = line.slice(i, j);
      const lookup = insensitive ? word.toLowerCase() : word;
      tokens.push({ kind: keywords.has(lookup) ? "keyword" : "plain", text: word });
      i = j;
      continue;
    }
    tokens.push({ kind: "plain", text: ch });
    i += 1;
  }
  return tokens;
}

export function tokenClass(kind: Kind): string {
  switch (kind) {
    case "keyword":
      return "text-accent";
    case "string":
      return "text-success";
    case "comment":
      return "text-muted-foreground";
    case "number":
      return "text-foreground/80";
    default:
      return "text-foreground";
  }
}
