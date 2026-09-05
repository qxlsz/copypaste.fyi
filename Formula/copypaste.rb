# Official tap: brew install qxlsz/copypaste/copypaste
# Stable URLs are GitHub Release tarballs written by scripts/bump-homebrew.sh
# on a v* tag. head: still compiles from main.
class Copypaste < Formula
  desc "Pastebin CLI and self-hostable server - type, get link, share"
  homepage "https://www.copypaste.fyi"
  license "MIT"
  version "0.2.0"
  head "https://github.com/qxlsz/copypaste.fyi.git", branch: "main"

  on_macos do
    on_arm do
      url "https://github.com/qxlsz/copypaste.fyi/releases/download/v0.2.0/copypaste-darwin-arm64.tar.gz"
      sha256 "afbae1b88c79197452c5df46373e60769a9f4dcb228e569ff524fcddff67c18f"
    end
    on_intel do
      url "https://github.com/qxlsz/copypaste.fyi/releases/download/v0.2.0/copypaste-darwin-x64.tar.gz"
      sha256 "66965ba7508db5c092473fa866561e2dae220b9c1964758846ff691ec072cd9e"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/qxlsz/copypaste.fyi/releases/download/v0.2.0/copypaste-linux-arm64.tar.gz"
      sha256 "7e422382bb7ff737fa14af6c8c3a8841d1ae1ed639b9a019dc23db2c70bab07d"
    end
    on_intel do
      url "https://github.com/qxlsz/copypaste.fyi/releases/download/v0.2.0/copypaste-linux-amd64.tar.gz"
      sha256 "974fa66ae5b4f00b0937c762d6d3372d700aad78124d5d3fd910c1fdbda043bf"
    end
  end

  depends_on "rust" => :build if build.head?

  def install
    if build.head?
      system "cargo", "install", *std_cargo_args
    else
      bin.install "copypaste"
    end
  end

  def caveats
    <<~EOS
      Public site:  copypaste send --host https://www.copypaste.fyi "notes"
      Local server: copypaste serve
                    brew services start copypaste

      Closed instance: set COPYPASTE_REQUIRE_WRITE_AUTH=true and
      COPYPASTE_AUTH_TOKEN in the environment. Tokens never go on argv.
    EOS
  end

  service do
    run [opt_bin/"copypaste", "serve"]
    keep_alive true
    working_dir var
    log_path var/"log/copypaste.log"
    error_log_path var/"log/copypaste.log"
    environment_variables ROCKET_ADDRESS: "127.0.0.1", COPYPASTE_FORCE_MEMORY: "true"
  end

  test do
    assert_match "paste", shell_output("#{bin/\"copypaste\"} --help")
  end
end
