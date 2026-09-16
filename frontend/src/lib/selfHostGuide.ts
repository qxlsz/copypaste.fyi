export type HostGoal = "public" | "local" | "locked" | "connector";
export type HostMachine =
  | "apple"
  | "windows"
  | "ubuntu"
  | "fedora"
  | "docker"
  | "grok"
  | "cursor"
  | "aws";
export type HostStore = "memory" | "redis";

export const HOST_GOALS: { id: HostGoal; label: string; hint: string }[] = [
  { id: "public", label: "Just send text", hint: "Use copypaste.fyi. No server." },
  { id: "local", label: "Host on this computer", hint: "Browser at 127.0.0.1:8000." },
  { id: "locked", label: "Host and lock writes", hint: "Team box with a write token." },
  {
    id: "connector",
    label: "Encrypted store + Grok connector",
    hint: "Your server keeps ciphertext. Point Grok at /mcp.",
  },
];

export const HOST_MACHINES: { id: HostMachine; label: string }[] = [
  { id: "apple", label: "Apple" },
  { id: "windows", label: "Windows" },
  { id: "ubuntu", label: "Ubuntu / Debian" },
  { id: "fedora", label: "Fedora" },
  { id: "aws", label: "AWS / any VM" },
  { id: "grok", label: "Grok / Grokbot VM" },
  { id: "cursor", label: "Cursor cloud agent" },
  { id: "docker", label: "I have Docker" },
];

export const HOST_STORES: { id: HostStore; label: string; hint: string }[] = [
  { id: "memory", label: "Memory", hint: "Pastes die when the process dies." },
  {
    id: "redis",
    label: "Upstash Redis",
    hint: "Durable store. Works from AWS, Fly, or a closet. Not S3.",
  },
];

export interface HostRecipe {
  title: string;
  follow: string;
  commands: string;
}

const sendLocal = 'copypaste send --host http://127.0.0.1:8000 "notes from this box"';
const sendPublic = 'copypaste send --host https://www.copypaste.fyi "notes"';

const persistBlock = (store: HostStore, bind: string): string => {
  if (store === "redis") {
    return `export COPYPASTE_PERSISTENCE_BACKEND=redis
export UPSTASH_REDIS_REST_URL='https://your-upstash-endpoint'
export UPSTASH_REDIS_REST_TOKEN='your-upstash-token'
# Do not set COPYPASTE_FORCE_MEMORY. S3 and ElastiCache TCP are not backends.
ROCKET_ADDRESS=${bind} copypaste serve`;
  }
  return `ROCKET_ADDRESS=${bind} COPYPASTE_FORCE_MEMORY=true copypaste serve`;
};

const installFor = (machine: HostMachine): { follow: string; install: string } => {
  switch (machine) {
    case "apple":
      return {
        follow: "Apple + Homebrew",
        install: "brew install qxlsz/copypaste/copypaste",
      };
    case "windows":
      return {
        follow: "Windows PowerShell",
        install: "irm https://www.copypaste.fyi/install.ps1 | iex",
      };
    case "ubuntu":
      return {
        follow: "Ubuntu / Debian (same as a Grok VM)",
        install: `git clone https://github.com/qxlsz/copypaste.fyi.git
cd copypaste.fyi
./scripts/agent-setup.sh`,
      };
    case "fedora":
      return {
        follow: "Fedora",
        install: `curl -fsSL https://www.copypaste.fyi/install.sh | sh
# if that cannot find a release:
sudo dnf install -y gcc pkgconf openssl-devel
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install copypaste`,
      };
    case "aws":
      return {
        follow: "AWS EC2, Lightsail, or any Ubuntu VM",
        install: `sudo apt-get update
sudo apt-get install -y git build-essential pkg-config libssl-dev
git clone https://github.com/qxlsz/copypaste.fyi.git
cd copypaste.fyi
./scripts/agent-setup.sh
# Security group: open 8000 only to you, or put nginx/caddy in front.`,
      };
    case "docker":
      return {
        follow: "Docker, from a git clone",
        install: `git clone https://github.com/qxlsz/copypaste.fyi.git
cd copypaste.fyi
docker compose up --build`,
      };
    case "grok":
      return {
        follow: "Grok / Grokbot VM",
        install: `git clone https://github.com/qxlsz/copypaste.fyi.git
cd copypaste.fyi
./scripts/agent-setup.sh`,
      };
    case "cursor":
      return {
        follow: "Cursor cloud agent",
        install: `git clone https://github.com/qxlsz/copypaste.fyi.git
cd copypaste.fyi
./scripts/agent-setup.sh`,
      };
  }
};

export const hostRecipe = (
  goal: HostGoal,
  machine: HostMachine,
  store: HostStore = "memory",
): HostRecipe => {
  const { follow, install } = installFor(machine);
  const bind = machine === "aws" ? "0.0.0.0" : "127.0.0.1";
  const serve = persistBlock(store, bind);

  if (goal === "public") {
    if (machine === "docker" || machine === "grok" || machine === "cursor" || machine === "aws") {
      return {
        title: "Public site only",
        follow: "You do not need a VM server. curl the public API, or install the CLI.",
        commands: `curl -sS -X POST https://www.copypaste.fyi/api/pastes \\
  -H 'content-type: application/json' \\
  -d '{"content":"notes","format":"plain_text"}'
# or: cargo install copypaste && ${sendPublic}`,
      };
    }
    return {
      title: "Public site only",
      follow: `Follow ${follow}. Do not start a server.`,
      commands: `${install}
${sendPublic}`,
    };
  }

  if (machine === "docker") {
    const extra = goal === "locked" ? `\n# then set COPYPASTE_REQUIRE_WRITE_AUTH=true in .env` : "";
    const storeBit =
      store === "redis"
        ? "\n# set COPYPASTE_PERSISTENCE_BACKEND=redis and the UPSTASH_* vars in .env"
        : "";
    return {
      title: goal === "locked" ? "Docker host, locked writes" : "Docker host on this computer",
      follow: "Follow Docker. Open http://127.0.0.1:8000 after compose is up.",
      commands: `${install}${extra}${storeBit}
${sendLocal}`,
    };
  }

  if (machine === "grok" || machine === "cursor") {
    const who = machine === "grok" ? "Grok / Grokbot VM" : "Cursor cloud agent";
    const lockBit =
      goal === "locked"
        ? "\nexport COPYPASTE_REQUIRE_WRITE_AUTH=true\nexport COPYPASTE_AUTH_TOKEN='replace-with-43-to-128-base64url-chars'"
        : "";
    return {
      title:
        goal === "connector"
          ? `${who}, encrypted store + connector`
          : goal === "locked"
            ? `${who}, locked writes`
            : `${who}, local server`,
      follow:
        goal === "connector"
          ? `Follow ${who}, serve, then add http://127.0.0.1:8000/mcp in grok.com/connectors.`
          : `Follow ${who}. Run agent-setup, then --serve. Do not use the Apple brew path.`,
      commands: `${install}${lockBit}
./scripts/agent-setup.sh --serve
${sendLocal}${
        goal === "connector"
          ? `
# Encrypt on send:
copypaste send --host http://127.0.0.1:8000 --agent "secret for the next model"
# Grok connector: grok.com/connectors → New → Custom → http://127.0.0.1:8000/mcp`
          : ""
      }`,
    };
  }

  const lockBit =
    goal === "locked"
      ? `export COPYPASTE_REQUIRE_WRITE_AUTH=true
export COPYPASTE_AUTH_TOKEN='replace-with-43-to-128-base64url-chars'
`
      : "";

  const service =
    machine === "apple"
      ? "\nbrew services start copypaste   # optional, instead of serve"
      : machine === "ubuntu" || machine === "fedora" || machine === "aws"
        ? "\n# optional: sudo cp contrib/systemd/copypaste.service /etc/systemd/system && sudo systemctl enable --now copypaste"
        : "";

  const where = machine === "aws" ? "AWS or any VM" : "Your computer";
  if (goal === "connector") {
    return {
      title: `${where}, encrypted store + Grok connector`,
      follow: `Follow ${follow}. Serve, encrypt on send, add /mcp as a custom Grok connector.`,
      commands: `${install}
${lockBit}${serve}${service}
# store ciphertext, not S3
copypaste send --host http://127.0.0.1:8000 --agent "secret for the next model"
# grok.com/connectors → New Connector → Custom
# MCP URL: http://${bind === "0.0.0.0" ? "<public-host>" : "127.0.0.1"}:8000/mcp`,
    };
  }
  return {
    title: `${where}, ${goal === "locked" ? "locked writes" : "open writes"}, ${store}`,
    follow: `Follow ${follow}, then serve. Pastes live in ${store === "redis" ? "Upstash Redis" : "process memory"}.`,
    commands: `${install}
${lockBit}${serve}${service}
# http://${bind === "0.0.0.0" ? "<public-ip>" : "127.0.0.1"}:8000
${bind === "0.0.0.0" ? 'copypaste send --host http://127.0.0.1:8000 "notes from this box"' : sendLocal}`,
  };
};
