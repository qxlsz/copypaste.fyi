class Copypaste < Formula
  desc "Ephemeral encrypted paste layer for humans and AI agents"
  homepage "https://github.com/qxlsz/copypaste.fyi"
  license "Apache-2.0"
  head "https://github.com/qxlsz/copypaste.fyi.git", branch: "main"

  depends_on "node"

  def install
    bin.install "cli/copypaste.mjs" => "copypaste"
    man1.install "packaging/man/copypaste.1" if File.exist?("packaging/man/copypaste.1")
    doc.install "docs/AGENTS.md" if File.exist?("docs/AGENTS.md")
    doc.install "docs/SECURITY.md" if File.exist?("docs/SECURITY.md")
    doc.install "ACCEPTABLE_USE.md" if File.exist?("ACCEPTABLE_USE.md")
  end

  test do
    output = shell_output("#{bin}/copypaste version")
    assert_match "copypaste.v1", output
    assert_match "1.0.0", output
    spec = shell_output("#{bin}/copypaste spec")
    assert_match "no_listing", spec
  end
end
