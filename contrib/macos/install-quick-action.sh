#!/usr/bin/env bash
# Installs a macOS Quick Action: select text → right-click → Services → Send to copypaste.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
DEST="$HOME/Library/Services/Send to copypaste.workflow"
SCRIPT="$ROOT/send-selection.sh"

mkdir -p "$DEST/Contents"
chmod +x "$SCRIPT"

cat > "$DEST/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>NSServices</key>
  <array>
    <dict>
      <key>NSMenuItem</key>
      <dict>
        <key>default</key>
        <string>Send to copypaste</string>
      </dict>
      <key>NSMessage</key>
      <string>runWorkflowAsService</string>
      <key>NSSendTypes</key>
      <array>
        <string>public.utf8-plain-text</string>
        <string>NSStringPboardType</string>
      </array>
    </dict>
  </array>
</dict>
</plist>
PLIST

cat > "$DEST/Contents/document.wflow" <<WFLOW
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>AMApplicationBuild</key>
  <string>523</string>
  <key>actions</key>
  <array>
    <dict>
      <key>action</key>
      <dict>
        <key>ActionBundlePath</key>
        <string>/System/Library/Automator/Run Shell Script.action</string>
        <key>ActionName</key>
        <string>Run Shell Script</string>
        <key>ActionParameters</key>
        <dict>
          <key>COMMAND_STRING</key>
          <string>exec "$SCRIPT"</string>
          <key>CheckedForUserDefaultShell</key>
          <true/>
          <key>inputMethod</key>
          <integer>0</integer>
          <key>shell</key>
          <string>/bin/bash</string>
          <key>source</key>
          <string></string>
        </dict>
      </dict>
    </dict>
  </array>
  <key>workflowType</key>
  <string>MagicImport</string>
</dict>
</plist>
WFLOW

echo "Installed: $DEST"
echo "Finder or any app → select text → Services → Send to copypaste"
echo "First run: System Settings → Privacy & Security → allow Automator if asked."
echo "Override host: COPYPASTE_HOST=http://127.0.0.1:8000"
