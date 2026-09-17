# iTerm2, Ghostty, cmux

Right-click copy in these terminals should land on *your* copypaste server, not only the local clipboard.

The command is:

```bash
export COPYPASTE_HOST=http://127.0.0.1:8000
copypaste clip
```

It reads the current clipboard, `POST`s it to that host, and puts the `/p/{id}` URL back on the clipboard. `clip` reads `COPYPASTE_HOST` when `--host` is omitted. `--host` still wins.

Public site: `export COPYPASTE_HOST=https://www.copypaste.fyi` then `copypaste clip`, or pass `--host` once.

Closed box: `clip` has no `--auth-token-file`. Set `COPYPASTE_AUTH_TOKEN` in the environment as well.

Install snippets: `./contrib/terminals/install.sh`

## iTerm2

1. Select text. That copies to the clipboard if you use iTerm2's default select-to-copy, or use Cmd+C.
2. Pointer: **Preferences → Pointer → Bindings → +**
   - Gesture: Right button
   - Action: Run Command in Background
   - Command:

```bash
/usr/local/bin/copypaste clip --host "${COPYPASTE_HOST:-http://127.0.0.1:8000}"
```

Or, on selection without a prior copy, bind:

```bash
printf %s '\\(selection)' | copypaste send --stdin --host "${COPYPASTE_HOST:-http://127.0.0.1:8000}" | pbcopy
```

`\\(selection)` is iTerm2's selected text. After the command, the clipboard holds the share URL. Paste as usual.

macOS Service (also shows in iTerm2's right-click menu):

```bash
./contrib/macos/install-quick-action.sh
```

Services → **Send to copypaste**. Set `COPYPASTE_HOST` if the server is local.

## Ghostty

Ghostty has no "run this binary on right-click". It does have select-to-copy and the macOS Services menu.

`~/.config/ghostty/config`:

```
copy_on_select = true
config-file = ?copypaste
```

Then copy [contrib/terminals/ghostty.config](../contrib/terminals/ghostty.config) to `~/.config/ghostty/copypaste`.

After a selection copy, run:

```bash
export COPYPASTE_HOST=http://127.0.0.1:8000
copypaste clip
```

Or use the same macOS Service as iTerm2. Right-click selected text → Services → Send to copypaste.

## cmux

cmux reads Ghostty's config for terminal keybinds. Same file as above.

cmux-specific workspace shortcuts stay in cmux Settings. Do not put those in the Ghostty file.

Right-click selected text on macOS still offers Services → Send to copypaste.

## Check it

```bash
export COPYPASTE_HOST=http://127.0.0.1:8000
echo "from the terminal" | pbcopy   # or wl-copy
copypaste clip
pbpaste
```

You should see `http://127.0.0.1:8000/p/...`.
