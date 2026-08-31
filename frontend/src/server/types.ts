export interface PasteViewResponse {
  id: string;
  format:
    | "plain_text"
    | "markdown"
    | "code"
    | "json"
    | "javascript"
    | "typescript"
    | "python"
    | "rust"
    | "go"
    | "cpp"
    | "kotlin"
    | "java"
    | "csharp"
    | "php"
    | "ruby"
    | "bash"
    | "yaml"
    | "sql"
    | "swift"
    | "html"
    | "css";
  content: string;
  createdAt: number;
  expiresAt?: number | null;
  burnAfterReading: boolean;
  encryption: {
    algorithm:
      | "none"
      | "aes256_gcm"
      | "chacha20_poly1305"
      | "xchacha20_poly1305";
    requiresKey: boolean;
  };
  torAccessOnly: boolean;
  timeLock?: {
    notBefore?: number | null;
    notAfter?: number | null;
  } | null;
}
