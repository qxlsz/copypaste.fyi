Output docs/demo.gif

Set FontSize 18
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Scene 1: Create a burn-after-reading paste
Type "# Create a burn-after-reading paste"
Sleep 500ms
Enter

Type `PASTE_URL=$(curl -s -X POST http://localhost:8000/api/pastes \`
Sleep 200ms
Enter
Type `  -H 'Content-Type: application/json' \`
Sleep 200ms
Enter
Type `  -d '{"content":"db-password: hunter2","burn_after_reading":true}' \`
Sleep 200ms
Enter
Type `  | python3 -c 'import sys,json; print(json.load(sys.stdin)["shareableUrl"])')`
Sleep 1s
Enter
Sleep 2s

Type `echo "Created: http://localhost:8000$PASTE_URL"`
Enter
Sleep 1s

# Scene 2: First view — paste is delivered
Type "# First view delivers the content"
Sleep 500ms
Enter

Type `curl -s "http://localhost:8000${PASTE_URL}/raw"`
Enter
Sleep 2s

# Scene 3: Second view — paste is gone (burn-after-reading worked)
Type "# Second view: paste is destroyed"
Sleep 500ms
Enter

Type `curl -s -o /dev/null -w "%{http_code}" "http://localhost:8000${PASTE_URL}/raw"`
Enter
Sleep 2s
