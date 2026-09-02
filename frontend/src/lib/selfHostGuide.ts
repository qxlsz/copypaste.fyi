export type HostGoal = "public" | "local" | "locked";
export type HostMachine = "apple" | "windows" | "ubuntu" | "fedora" | "docker" | "grok" | "cursor";

export const HOST_GOALS: { id: HostGoal; label: string; hint: string }[] = [
  { id: "public", label: "Just send text", hint: "Use copypaste.fyi. No server." },
  { id: "local", label: "Host on this computer", hint: "Browser at 127.0.0.1:8000." },
  { id: "locked", label: "Host and lock writes", hint: "Team box with a write token." },
];

export const HOST_MACHINES: { id: HostMachine; label: string }[] = [
  { id: "apple", label: "Apple" },
  { id: "windows", label: "Windows" },
  { id: "ubuntu", label: "Ubuntu / Debian" },
  { id: "fedora", label: "Fedora" },
  { id: "grok", label: "Grok / Grokbot VM" },
  { id: "cursor", label: "Cursor cloud agent" },
  { id: "docker", label: "I have Docker" },
];

export interface HostRecipe {
  title: string;
  follow: string;
  commands: string;
}

const serve = "ROCKET_ADDRESS=127.0.0.1 COPYPASTE_FORCE_MEMORY=true copypaste serve";
const lock = `export COPYPASTE_REQUIRE_WRITE_AUTH=true
export COPYPASTE_AUTH_TOKEN='replace-with-43-to-128-base64url-chars'
${serve}`;
const sendLocal = 'copypaste send --host http://127.0.0.1:8000 "notes from this box"';
const sendPublic = 'copypaste send --host https://www.copypaste.fyi "notes"';

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

export const hostRecipe = (goal: HostGoal, machine: HostMachine): HostRecipe => {
  const { follow, install } = installFor(machine);

  if (goal === "public") {
    if (machine === "docker" || machine === "grok" || machine === "cursor") {
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
    return {
      title: goal === "locked" ? "Docker host, locked writes" : "Docker host on this computer",
      follow: "Follow Docker. Open http://127.0.0.1:8000 after compose is up.",
      commands: `${install}${extra}
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
      title: goal === "locked" ? `${who}, locked writes` : `${who}, local server`,
      follow: `Follow ${who}. Run agent-setup, then --serve. Do not use the Apple brew path.`,
      commands: `${install}${lockBit}
./scripts/agent-setup.sh --serve
${sendLocal}`,
    };
  }

  if (goal === "locked") {
    return {
      title: "Your computer, locked writes",
      follow: `Follow ${follow}, then the lock block.`,
      commands: `${install}
${lock}
# http://127.0.0.1:8000
${sendLocal}`,
    };
  }

  const service =
    machine === "apple"
      ? "\nbrew services start copypaste   # optional, instead of serve"
      : machine === "ubuntu" || machine === "fedora"
        ? "\n# optional: sudo cp contrib/systemd/copypaste.service /etc/systemd/system && sudo systemctl enable --now copypaste"
        : "";

  return {
    title: "Your computer, open writes",
    follow: `Follow ${follow}, then serve.`,
    commands: `${install}
${serve}${service}
# http://127.0.0.1:8000
${sendLocal}`,
  };
};
