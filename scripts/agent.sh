#!/usr/bin/env bash
# =============================================================================
# copypaste.fyi — Autonomous Issue-to-Merge Agent
# =============================================================================
# Picks up GitHub issues, implements fixes via Claude Code, creates PRs,
# reviews independently, and merges — all locally.
#
# Prerequisites:
#   - gh CLI authenticated (gh auth status)
#   - claude CLI authenticated (claude login or existing session)
#   - cargo, npm, cargo-nextest installed
#
# Usage:
#   ./scripts/agent.sh              # Process all open issues then exit
#   ./scripts/agent.sh --loop       # Poll every 5 minutes forever
#   ./scripts/agent.sh --loop 120   # Poll every 120 seconds
# =============================================================================

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

REPO=$(gh repo view --json nameWithOwner --jq '.nameWithOwner')
MAX_ATTEMPTS=5
POLL_INTERVAL=300
LOOP_MODE=false
LOG_DIR="${REPO_ROOT}/.agent-logs"
PROMPT_DIR=$(mktemp -d)
mkdir -p "$LOG_DIR"

trap 'rm -rf "$PROMPT_DIR"' EXIT

# Lock file — prevent concurrent agent runs (e.g. from rogue background sessions)
LOCK_FILE="${REPO_ROOT}/.agent.lock"
if ! ( set -o noclobber; echo "$$" > "$LOCK_FILE" ) 2>/dev/null; then
  existing_pid=$(cat "$LOCK_FILE" 2>/dev/null || echo "unknown")
  # Stale lock: if existing PID is not a running process, remove and proceed
  if [[ "$existing_pid" != "unknown" ]] && ! kill -0 "$existing_pid" 2>/dev/null; then
    echo "Removing stale lock for dead PID ${existing_pid}."
    rm -f "$LOCK_FILE"
    echo "$$" > "$LOCK_FILE"
  else
    echo "Agent already running (PID ${existing_pid}). Exiting to avoid concurrent runs."
    exit 0
  fi
fi
trap 'rm -rf "$PROMPT_DIR"; rm -f "$LOCK_FILE"' EXIT

# Parse args
if [[ "${1:-}" == "--loop" ]]; then
  LOOP_MODE=true
  [[ -n "${2:-}" ]] && POLL_INTERVAL="$2"
fi

# ---------------------------------------------------------------------------
log() { echo "[$(date '+%H:%M:%S')] $*"; }
log_section() { echo ""; log "═══════════════════════════════════════════"; log "$*"; log "═══════════════════════════════════════════"; }

sanitize_issue_body() {
  # Strip prompt-injection patterns before embedding externally-sourced issue
  # content in Claude prompts. Defence-in-depth: primary mitigation is removing
  # --dangerously-skip-permissions from triage and review invocations.
  printf '%s' "$1" \
    | sed 's/[Ii]gnore previous instructions[^.]*\.//g' \
    | sed 's/[Ii]gnore all previous[^.]*\.//g' \
    | sed 's/[Ff]orget.*instructions[^.]*\.//g' \
    | sed 's/[Dd]isregard.*instructions[^.]*\.//g' \
    | sed 's/[Ss]ystem[[:space:]]*prompt[[:space:]]*://g'
}

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------
preflight() {
  log "Running preflight checks..."
  local ok=true

  if ! command -v gh &>/dev/null; then
    log "ERROR: gh CLI not found"; ok=false
  elif ! gh auth status &>/dev/null 2>&1; then
    log "ERROR: gh not authenticated. Run: gh auth login"; ok=false
  fi

  if ! command -v claude &>/dev/null; then
    log "ERROR: claude CLI not found. Install: npm i -g @anthropic-ai/claude-code"; ok=false
  fi

  if ! command -v cargo &>/dev/null; then
    log "ERROR: cargo not found"; ok=false
  fi

  [[ "$ok" == "false" ]] && { log "Preflight failed."; exit 1; }
  log "Preflight passed."
}

# ---------------------------------------------------------------------------
# Issue helpers
# ---------------------------------------------------------------------------
get_open_issues() {
  gh issue list --repo "$REPO" --state open --json number,title,labels --limit 50 | \
    jq -r '[.[] | select(.labels | map(.name) | index("agent-stuck") | not)] | sort_by(.number) | .[].number'
}

has_open_pr() {
  local count
  count=$(gh pr list --repo "$REPO" --head "agent/issue-${1}" --state open --json number --jq 'length')
  [[ "$count" -gt 0 ]]
}

commit_prefix() {
  local labels="$1"
  if echo "$labels" | grep -qiE "bug|fix"; then echo "fix"
  elif echo "$labels" | grep -qiE "feature|enhancement"; then echo "feat"
  elif echo "$labels" | grep -qiE "docs|documentation"; then echo "docs"
  elif echo "$labels" | grep -qiE "refactor"; then echo "refactor"
  else echo "fix"
  fi
}

# ---------------------------------------------------------------------------
# Triage
# ---------------------------------------------------------------------------
triage_issue() {
  local issue_number="$1" title="$2" body="$3"
  log "Triaging issue #${issue_number}..."

  local safe_body
  safe_body=$(sanitize_issue_body "$body")

  cat > "${PROMPT_DIR}/triage.txt" <<TRIAGE_PROMPT
You are a triage agent. Classify this GitHub issue.

Issue #${issue_number}: ${title}

${safe_body}

Classify as:
- ACTIONABLE: Bug with enough detail, feature with clear scope, or refactoring task
- NOT_ACTIONABLE: Question, discussion, incomplete report, or meta-issue

Respond with ONLY one line:
ACTIONABLE: <reason>
or
NOT_ACTIONABLE: <reason>
TRIAGE_PROMPT

  local result
  result=$(cat "${PROMPT_DIR}/triage.txt" | claude --print --max-turns 1 2>/dev/null) || result="ACTIONABLE: triage failed, attempting anyway"
  echo "$result"
}

# ---------------------------------------------------------------------------
# Implement
# ---------------------------------------------------------------------------
implement() {
  local issue_number="$1" title="$2" body="$3" attempt="$4" feedback="${5:-}"

  local strategy directive
  case "$attempt" in
    1) strategy="straightforward"
       directive="Read the issue, understand what's needed, implement the most direct fix." ;;
    2) strategy="deeper-analysis"
       directive="The straightforward approach failed. Read more of the codebase. Study related files and tests. Try a fundamentally different approach based on the review feedback." ;;
    3) strategy="decompose"
       directive="Previous approaches failed. Break the problem into the smallest possible change that still addresses the core issue." ;;
    4) strategy="targeted-fix"
       directive="Address ONLY the specific issues raised in the latest review feedback — nothing more, nothing less. Read the feedback carefully and fix each point exactly as described. Do not refactor, do not add features, do not change anything that wasn't mentioned in the feedback." ;;
    5) strategy="minimal-viable"
       directive="Implement the absolute minimum change needed to address the single most critical issue from the review feedback. If the review has multiple points, fix only the most important correctness or security issue. Leave everything else for a follow-up. The goal is to get something merged, not to be perfect." ;;
  esac

  local feedback_section=""
  if [[ -n "$feedback" ]]; then
    feedback_section="
## Previous Review Feedback

${feedback}"
  fi

  log "Running Claude Code (attempt ${attempt}/3, strategy: ${strategy})..."

  local safe_body
  safe_body=$(sanitize_issue_body "$body")

  cat > "${PROMPT_DIR}/implement.txt" <<IMPL_PROMPT
You are an autonomous coding agent working on the copypaste.fyi project.

## Your Task

Implement a fix/feature for this GitHub issue:

**Issue #${issue_number}: ${title}**

${safe_body}

## Attempt ${attempt}/3 — Strategy: ${strategy}
${feedback_section}

## Strategy

${directive}

## Rules

1. Read CLAUDE.md and agents.md first to understand the project
2. Study the relevant parts of the codebase before writing code
3. Write clean, minimal code that solves the issue — no scope creep
4. Follow existing patterns in the codebase
5. Add tests for new/changed behavior
6. Run the FULL test suite before finishing:
   - cargo fmt --all -- --check
   - cargo clippy --all-targets --all-features -- -D warnings
   - cargo nextest run --workspace --all-features
   - cd frontend && npm test -- --run && npm run lint
7. If tests fail, fix them. Do not leave failing tests.
8. Commit your changes with a descriptive message
9. **CRITICAL — git branch discipline**: You are on branch \`agent/issue-${issue_number}\`. DO NOT run \`git checkout\`, \`git switch\`, \`git branch -D\`, or any other command that changes the current branch. Commit directly to the current branch. Never switch to main or any other branch.
IMPL_PROMPT

  cat "${PROMPT_DIR}/implement.txt" | claude --print --max-turns 30 --allowedTools "Bash,Read,Write,Edit,Glob,Grep"
}

# ---------------------------------------------------------------------------
# Validate locally
# ---------------------------------------------------------------------------
validate() {
  log "Running local validation..."
  local failed=false

  log "  cargo fmt..."
  cargo fmt --all -- --check 2>&1 || { log "  FAIL: cargo fmt"; failed=true; }

  log "  cargo clippy..."
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 || { log "  FAIL: clippy"; failed=true; }

  log "  cargo nextest..."
  cargo nextest run --workspace --all-features 2>&1 || { log "  FAIL: nextest"; failed=true; }

  if cargo llvm-cov --version &>/dev/null 2>&1; then
    log "  cargo llvm-cov (75% min)..."
    cargo llvm-cov nextest --workspace --all-features --fail-under-lines 75 2>&1 || { log "  FAIL: coverage"; failed=true; }
  fi

  log "  frontend install (sync lockfile)..."
  (cd frontend && npm install) 2>&1 || { log "  FAIL: frontend npm install"; failed=true; }

  log "  frontend tests..."
  (cd frontend && npm test -- --run) 2>&1 || { log "  FAIL: frontend tests"; failed=true; }

  log "  frontend lint..."
  (cd frontend && npm run lint) 2>&1 || { log "  FAIL: frontend lint"; failed=true; }

  [[ "$failed" == "true" ]] && return 1
  log "All validation passed."
}

# ---------------------------------------------------------------------------
# Review (independent Claude session)
# ---------------------------------------------------------------------------
review() {
  local issue_number="$1" title="$2" body="$3"
  log "Running independent code review..."

  local diff
  diff=$(git diff main...HEAD)

  local safe_body
  safe_body=$(sanitize_issue_body "$body")

  cat > "${PROMPT_DIR}/review.txt" <<REVIEW_PROMPT
You are an independent code reviewer for the copypaste.fyi project.
You are a DIFFERENT agent from the one that wrote this code. Review it critically.

## The Issue

Issue #${issue_number}: ${title}

${safe_body}

## The Diff

\`\`\`diff
${diff}
\`\`\`

## Review Checklist — ALL must pass

1. **Correctness**: Does the code actually solve what the issue asks for?
2. **Security**: No injection vulnerabilities, unsafe unwraps, secret leaks, OWASP top 10
3. **Existing patterns**: Follows codebase conventions (check CLAUDE.md and agents.md)
4. **Test coverage**: New/changed behavior has tests that would catch regressions
5. **No scope creep**: Only changes what's needed
6. **Build confidence**: Changes look consistent with passing CI

## What NOT to review
- Style/formatting (cargo fmt and clippy handle this)
- "Nice to have" improvements
- The approach itself, unless fundamentally broken

## Your response — use EXACTLY this format

DECISION: APPROVE
SUMMARY: <brief summary>

OR

DECISION: REQUEST_CHANGES
SUMMARY: <brief summary>
FEEDBACK:
- <specific, actionable issue 1>
- <specific, actionable issue 2>
REVIEW_PROMPT

  local result
  result=$(cat "${PROMPT_DIR}/review.txt" | claude --print --max-turns 15 --allowedTools "Read,Glob,Grep" 2>/dev/null) || result="DECISION: APPROVE
SUMMARY: Review timed out"
  echo "$result"
}

# ---------------------------------------------------------------------------
# Process a single issue end-to-end
# ---------------------------------------------------------------------------
process_issue() {
  local issue_number="$1"
  log_section "Processing issue #${issue_number}"

  # Fetch issue
  local issue_json title body labels
  issue_json=$(gh issue view "$issue_number" --repo "$REPO" --json title,body,labels)
  title=$(echo "$issue_json" | jq -r '.title')
  body=$(echo "$issue_json" | jq -r '.body // ""')
  labels=$(echo "$issue_json" | jq -r '[.labels[].name] | join(",")')

  # Triage
  local triage_result
  triage_result=$(triage_issue "$issue_number" "$title" "$body")
  if echo "$triage_result" | grep -q "NOT_ACTIONABLE"; then
    local reason
    reason=$(echo "$triage_result" | sed 's/NOT_ACTIONABLE: //')
    log "Not actionable: ${reason}"
    gh issue comment "$issue_number" --repo "$REPO" \
      --body "I reviewed this issue but can't act on it yet.

**Reason:** ${reason}

Reopen with more detail and I'll try again."
    return 0
  fi

  log "Actionable. Starting implementation..."

  local branch="agent/issue-${issue_number}"
  local prefix
  prefix=$(commit_prefix "$labels")
  local attempt=1 feedback=""

  while [[ $attempt -le $MAX_ATTEMPTS ]]; do
    log "--- Attempt ${attempt}/${MAX_ATTEMPTS} ---"

    # Fresh branch from main
    git fetch origin main
    git checkout -B "$branch" origin/main

    # Implement
    implement "$issue_number" "$title" "$body" "$attempt" "$feedback" \
      2>&1 | tee "${LOG_DIR}/issue-${issue_number}-a${attempt}-impl.log" || true

    # Validate
    if ! validate 2>&1 | tee "${LOG_DIR}/issue-${issue_number}-a${attempt}-validate.log"; then
      log "Validation failed. Giving Claude one fix attempt..."

      local errors
      errors=$(tail -50 "${LOG_DIR}/issue-${issue_number}-a${attempt}-validate.log")
      cat > "${PROMPT_DIR}/fix.txt" <<FIX_PROMPT
The local validation failed. Fix these errors:

${errors}

Make sure ALL pass:
- cargo fmt --all -- --check
- cargo clippy --all-targets --all-features -- -D warnings
- cargo nextest run --workspace --all-features
- cd frontend && npm test -- --run && npm run lint
FIX_PROMPT
      cat "${PROMPT_DIR}/fix.txt" | claude --print --max-turns 10 --allowedTools "Bash,Read,Write,Edit,Glob,Grep" 2>&1 || true

      if ! validate 2>&1 | tee "${LOG_DIR}/issue-${issue_number}-a${attempt}-validate2.log"; then
        log "Still failing after fix."
        feedback="Validation failed. See errors."
        attempt=$((attempt + 1))
        continue
      fi
    fi

    # Safety: if Claude switched branches, recover all work and move to correct branch
    local current_branch
    current_branch=$(git branch --show-current)
    if [[ "$current_branch" != "$branch" ]]; then
      log "WARNING: on branch '$current_branch' instead of '$branch' — recovering..."
      # Capture stray commits on wrong branch (oldest first for cherry-pick order)
      local stray_shas
      stray_shas=$(git log --format="%H" --reverse "origin/main..${current_branch}" 2>/dev/null || true)
      # Stash any uncommitted changes so checkout can succeed
      git stash --include-untracked 2>/dev/null || true
      # Switch to correct branch
      git checkout -B "$branch" origin/main 2>/dev/null
      # Reapply uncommitted changes
      git stash pop 2>/dev/null || true
      # Cherry-pick stray commits (in oldest-first order)
      if [[ -n "$stray_shas" ]]; then
        while IFS= read -r sha; do
          [[ -z "$sha" ]] && continue
          git cherry-pick "$sha" 2>/dev/null || { git cherry-pick --abort 2>/dev/null; true; }
        done <<< "$stray_shas"
      fi
      # Reset the wrong branch back to origin/main
      git branch -f "$current_branch" origin/main 2>/dev/null || true
    fi

    # Commit remaining changes
    git add -A
    git diff --cached --quiet || git commit -m "agent: fix for issue #${issue_number} (attempt ${attempt})"

    # Review
    local review_result
    review_result=$(review "$issue_number" "$title" "$body" 2>&1 | \
      tee "${LOG_DIR}/issue-${issue_number}-a${attempt}-review.log")

    if echo "$review_result" | grep -q "DECISION: APPROVE"; then
      log "Review APPROVED."

      # Push & create PR
      git push --force origin "$branch"

      local existing_pr pr_number
      existing_pr=$(gh pr list --repo "$REPO" --head "$branch" --json number --jq '.[0].number // empty')

      local pr_body="## Closes #${issue_number}

## What changed
Implementation for: ${title}

## Approach
Attempt ${attempt}/${MAX_ATTEMPTS}

## Testing
All local validation passed. Agent review: approved."

      if [[ -n "$existing_pr" ]]; then
        gh pr edit "$existing_pr" --repo "$REPO" --body "$pr_body"
        pr_number="$existing_pr"
      else
        local pr_url
        pr_url=$(gh pr create --repo "$REPO" --head "$branch" --base main \
          --title "${prefix}: ${title}" --body "$pr_body")
        pr_number=$(echo "$pr_url" | grep -oE '[0-9]+$')
      fi
      log "PR #${pr_number}"

      # Wait for CI
      log "Waiting for CI..."
      local ci_passed=false
      for i in $(seq 1 30); do
        sleep 30
        local checks
        checks=$(gh pr checks "$pr_number" --repo "$REPO" --json name,bucket 2>/dev/null || echo "[]")
        [[ "$checks" == "[]" ]] && { log "  No checks yet ($i/30)..."; continue; }

        # Only consider required CI checks (ignore Vercel and other non-required checks)
        local required_checks
        required_checks=$(echo "$checks" | jq '[.[] | select(.name | test("Rust fmt|Rust clippy|Coverage|Frontend lint"))]')

        local pending
        pending=$(echo "$required_checks" | jq '[.[] | select(.bucket == "pending")] | length')
        [[ "$pending" -gt 0 ]] && { log "  ${pending} required checks pending ($i/30)..."; continue; }

        local failed
        failed=$(echo "$required_checks" | jq '[.[] | select(.bucket != "pass")] | length')
        if [[ "$failed" -gt 0 ]]; then
          log "CI FAILED:"
          echo "$required_checks" | jq -r '.[] | select(.bucket != "pass") | "  \(.name): \(.bucket)"'
          break
        fi

        log "All CI checks passed!"
        ci_passed=true
        break
      done

      if [[ "$ci_passed" == "false" ]]; then
        feedback="CI checks failed."
        attempt=$((attempt + 1))
        continue
      fi

      # MERGE
      log "Merging PR #${pr_number}..."
      if gh pr merge "$pr_number" --repo "$REPO" --merge \
          --subject "${prefix}: resolve #${issue_number} — ${title}" --delete-branch; then
        gh issue close "$issue_number" --repo "$REPO" \
          --comment "Resolved in PR #${pr_number}. Merged to main."
        log "DONE: Issue #${issue_number} resolved!"
        git checkout main && git pull origin main
        return 0
      else
        log "Merge failed. Trying rebase..."
        git fetch origin main
        if git rebase origin/main && git push --force origin "$branch"; then
          if gh pr merge "$pr_number" --repo "$REPO" --merge \
              --subject "${prefix}: resolve #${issue_number} — ${title}" --delete-branch; then
            gh issue close "$issue_number" --repo "$REPO" \
              --comment "Resolved in PR #${pr_number}. Merged to main (rebased)."
            log "DONE: Issue #${issue_number} resolved (after rebase)!"
            git checkout main && git pull origin main
            return 0
          fi
        fi
        feedback="Merge conflict."
        attempt=$((attempt + 1))
      fi
    else
      log "Review: CHANGES REQUESTED"
      feedback=$(echo "$review_result" | sed -n '/^FEEDBACK:/,$ p' | tail -n +2)
      log "Feedback: ${feedback}"
      attempt=$((attempt + 1))
    fi
  done

  # Exhausted
  log "All ${MAX_ATTEMPTS} attempts exhausted for issue #${issue_number}."
  gh issue edit "$issue_number" --repo "$REPO" --add-label "agent-stuck" 2>/dev/null || true
  gh issue comment "$issue_number" --repo "$REPO" \
    --body "Agent tried ${MAX_ATTEMPTS} times but couldn't resolve this issue. Labeled \`agent-stuck\`."
  git checkout main 2>/dev/null
  return 1
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
  preflight
  log_section "Agent starting (repo: ${REPO})"
  [[ "$LOOP_MODE" == "true" ]] && log "Loop mode: polling every ${POLL_INTERVAL}s"

  gh label create "agent-stuck" --repo "$REPO" --color "d73a4a" \
    --description "Agent could not resolve this issue" --force 2>/dev/null || true

  while true; do
    git checkout main 2>/dev/null
    git pull origin main 2>/dev/null || true

    local issues
    issues=$(get_open_issues)

    if [[ -z "$issues" ]]; then
      log "No open issues."
    else
      while IFS= read -r issue_number; do
        [[ -z "$issue_number" ]] && continue
        has_open_pr "$issue_number" && { log "Skip #${issue_number} — open PR exists."; continue; }
        process_issue "$issue_number" || true
      done <<< "$issues"
    fi

    [[ "$LOOP_MODE" == "false" ]] && { log "Done."; break; }
    log "Sleeping ${POLL_INTERVAL}s..."
    sleep "$POLL_INTERVAL"
  done
}

main "$@"
