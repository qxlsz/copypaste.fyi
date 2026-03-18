# Autonomous Issue-to-Merge Agent Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a fully autonomous GitHub Actions pipeline that picks up every new issue, implements a fix via Claude Code, reviews it with an independent agent, and auto-merges after CI passes.

**Architecture:** Four chained GitHub Actions workflows (`dispatcher` → `implement` → `review` → `merge`) connected via `repository_dispatch` events using GitHub App tokens. A JSON-based FIFO queue on a dedicated `agent-queue` branch ensures sequential processing. Two GitHub App identities separate the implementer from the reviewer.

**Tech Stack:** GitHub Actions, GitHub Apps, Claude Code Action (`anthropic/claude-code-action`), `gh` CLI, `jq`, bash scripting

**Spec:** `docs/superpowers/specs/2026-03-17-agent-pipeline-design.md`

---

## File Structure

```
.github/
  workflows/
    agent-dispatcher.yml    # Workflow 0: queue + dispatch
    agent-implement.yml     # Workflow 1: triage + implement + create PR
    agent-review.yml        # Workflow 2: independent code review
    agent-merge.yml         # Workflow 3: gate verification + merge
    agent-recovery.yml      # Workflow 4: hourly stuck-queue recovery
  scripts/
    agent-queue.sh          # Shared queue management functions
```

All queue operations (read, write, pop, dispatch-next) are centralized in `agent-queue.sh` to avoid duplication across 5 workflows.

---

### Task 1: Queue Management Script

**Files:**
- Create: `.github/scripts/agent-queue.sh`

This is the shared foundation used by all workflows. It manages the JSON queue file on the `agent-queue` branch.

- [ ] **Step 1: Create the queue management script**

```bash
#!/usr/bin/env bash
# .github/scripts/agent-queue.sh
# Shared queue management for the agent pipeline.
# Requires: gh CLI authenticated, GH_REPO set (owner/repo format)
# All functions operate on the 'agent-queue' branch, file 'queue.json'

set -euo pipefail

QUEUE_BRANCH="agent-queue"
QUEUE_FILE="queue.json"

# Ensure the agent-queue branch and queue.json exist.
# Creates them if missing (first-time setup).
ensure_queue_branch() {
  if ! gh api "repos/${GH_REPO}/git/ref/heads/${QUEUE_BRANCH}" &>/dev/null; then
    echo "Creating ${QUEUE_BRANCH} branch with empty queue..."
    # Get the SHA of the default branch
    local default_sha
    default_sha=$(gh api "repos/${GH_REPO}/git/ref/heads/main" --jq '.object.sha')
    # Create the branch
    gh api "repos/${GH_REPO}/git/refs" \
      -f "ref=refs/heads/${QUEUE_BRANCH}" \
      -f "sha=${default_sha}"
    # Create initial queue file
    local content
    content=$(echo '{"queue":[],"processing":null,"started_at":null}' | base64)
    gh api "repos/${GH_REPO}/contents/${QUEUE_FILE}" \
      -X PUT \
      -f "message=Initialize agent queue" \
      -f "content=${content}" \
      -f "branch=${QUEUE_BRANCH}"
  fi
}

# Read the current queue JSON. Outputs to stdout.
read_queue() {
  gh api "repos/${GH_REPO}/contents/${QUEUE_FILE}?ref=${QUEUE_BRANCH}" \
    --jq '.content' | base64 -d
}

# Get the SHA of the queue file (needed for updates).
get_queue_sha() {
  gh api "repos/${GH_REPO}/contents/${QUEUE_FILE}?ref=${QUEUE_BRANCH}" \
    --jq '.sha'
}

# Write updated queue JSON. Args: $1 = new JSON content, $2 = commit message
write_queue() {
  local new_content="$1"
  local message="$2"
  local sha
  sha=$(get_queue_sha)
  local encoded
  encoded=$(echo "${new_content}" | base64)
  gh api "repos/${GH_REPO}/contents/${QUEUE_FILE}" \
    -X PUT \
    -f "message=${message}" \
    -f "content=${encoded}" \
    -f "sha=${sha}" \
    -f "branch=${QUEUE_BRANCH}"
}

# Add an issue to the queue. Args: $1 = issue number
enqueue_issue() {
  local issue_number="$1"
  local queue_json
  queue_json=$(read_queue)
  local now
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  local updated
  updated=$(echo "${queue_json}" | jq --arg num "$issue_number" --arg ts "$now" \
    '.queue += [{"issue_number": ($num | tonumber), "queued_at": $ts}]')
  write_queue "${updated}" "Queue issue #${issue_number}"
}

# Check if the pipeline is currently processing an issue.
# Returns 0 if idle, 1 if busy.
is_idle() {
  local queue_json
  queue_json=$(read_queue)
  local processing
  processing=$(echo "${queue_json}" | jq -r '.processing')
  [ "$processing" = "null" ]
}

# Start processing the next issue in the queue.
# Sets processing field and removes from queue.
# Outputs the issue number to stdout, or empty if queue is empty.
start_next() {
  local queue_json
  queue_json=$(read_queue)
  local next_issue
  next_issue=$(echo "${queue_json}" | jq -r '.queue[0].issue_number // empty')
  if [ -z "$next_issue" ]; then
    echo ""
    return
  fi
  local now
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  local updated
  updated=$(echo "${queue_json}" | jq --arg ts "$now" \
    '.processing = .queue[0].issue_number | .started_at = $ts | .queue = .queue[1:]')
  write_queue "${updated}" "Start processing issue #${next_issue}"
  echo "$next_issue"
}

# Clear the processing field (issue done or failed).
# Does NOT dispatch next — call dispatch_next separately.
clear_processing() {
  local queue_json
  queue_json=$(read_queue)
  local updated
  updated=$(echo "${queue_json}" | jq '.processing = null | .started_at = null')
  write_queue "${updated}" "Clear processing"
}

# Dispatch the repository_dispatch event for agent-implement.
# Args: $1 = issue number, $2 = attempt (default 1), $3 = review feedback (default "")
dispatch_implement() {
  local issue_number="$1"
  local attempt="${2:-1}"
  local review_feedback="${3:-}"
  gh api "repos/${GH_REPO}/dispatches" \
    -f "event_type=agent-implement" \
    -f "client_payload[issue_number]=${issue_number}" \
    -f "client_payload[attempt]=${attempt}" \
    -f "client_payload[review_feedback]=${review_feedback}"
}

# Dispatch agent-review event.
# Args: $1 = issue number, $2 = PR number, $3 = attempt
dispatch_review() {
  local issue_number="$1"
  local pr_number="$2"
  local attempt="$3"
  gh api "repos/${GH_REPO}/dispatches" \
    -f "event_type=agent-review" \
    -f "client_payload[issue_number]=${issue_number}" \
    -f "client_payload[pr_number]=${pr_number}" \
    -f "client_payload[attempt]=${attempt}"
}

# Dispatch agent-merge event.
# Args: $1 = issue number, $2 = PR number
dispatch_merge() {
  local issue_number="$1"
  local pr_number="$2"
  gh api "repos/${GH_REPO}/dispatches" \
    -f "event_type=agent-merge" \
    -f "client_payload[issue_number]=${issue_number}" \
    -f "client_payload[pr_number]=${pr_number}"
}

# Pop queue on failure: clear processing, label issue, dispatch next.
# Args: $1 = issue number, $2 = failure reason
handle_failure() {
  local issue_number="$1"
  local reason="$2"
  # Label the issue
  gh issue edit "$issue_number" --repo "$GH_REPO" --add-label "agent-stuck" 2>/dev/null || true
  # Comment on the issue
  gh issue comment "$issue_number" --repo "$GH_REPO" \
    --body "The agent pipeline failed for this issue.

**Reason:** ${reason}

This issue has been labeled \`agent-stuck\` for manual review." || true
  # Clear processing and dispatch next
  clear_processing
  dispatch_next_if_queued
}

# If there are items in the queue, start the next one.
dispatch_next_if_queued() {
  local next
  next=$(start_next)
  if [ -n "$next" ]; then
    dispatch_implement "$next" 1 ""
  fi
}
```

- [ ] **Step 2: Make the script executable**

Run: `chmod +x .github/scripts/agent-queue.sh`

- [ ] **Step 3: Verify script syntax**

Run: `bash -n .github/scripts/agent-queue.sh`
Expected: No output (no syntax errors)

- [ ] **Step 4: Commit**

```bash
git add .github/scripts/agent-queue.sh
git commit -m "feat: add shared agent queue management script"
```

---

### Task 2: Agent Dispatcher Workflow

**Files:**
- Create: `.github/workflows/agent-dispatcher.yml`

Triggers on new/reopened issues. Adds the issue to the FIFO queue. If no issue is currently being processed, dispatches it immediately.

- [ ] **Step 1: Create the dispatcher workflow**

```yaml
# .github/workflows/agent-dispatcher.yml
name: "Agent: Dispatcher"

on:
  issues:
    types: [opened, reopened]

concurrency:
  group: agent-dispatcher
  cancel-in-progress: false

permissions:
  contents: write
  issues: write
  actions: write

jobs:
  dispatch:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Generate App token
        id: app-token
        uses: actions/create-github-app-token@v1
        with:
          app-id: ${{ secrets.AGENT_IMPL_APP_ID }}
          private-key: ${{ secrets.AGENT_IMPL_PRIVATE_KEY }}

      - name: Check if issue is from agent bot
        id: check-author
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
          ISSUE_AUTHOR: ${{ github.event.issue.user.login }}
        run: |
          # Get the bot usernames for both apps
          IMPL_BOT=$(gh api /app --jq '.slug')
          # Skip if the issue was created by either agent bot
          if [[ "$ISSUE_AUTHOR" == "${IMPL_BOT}[bot]" ]]; then
            echo "skip=true" >> "$GITHUB_OUTPUT"
            echo "Skipping issue from agent bot: $ISSUE_AUTHOR"
          else
            echo "skip=false" >> "$GITHUB_OUTPUT"
          fi

      - name: Ensure queue branch exists
        if: steps.check-author.outputs.skip == 'false'
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
          GH_REPO: ${{ github.repository }}
        run: |
          source .github/scripts/agent-queue.sh
          ensure_queue_branch

      - name: Enqueue and dispatch
        if: steps.check-author.outputs.skip == 'false'
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
          GH_REPO: ${{ github.repository }}
          ISSUE_NUMBER: ${{ github.event.issue.number }}
        run: |
          source .github/scripts/agent-queue.sh
          echo "Enqueuing issue #${ISSUE_NUMBER}..."
          enqueue_issue "$ISSUE_NUMBER"

          if is_idle; then
            echo "Pipeline is idle. Starting issue #${ISSUE_NUMBER}..."
            next=$(start_next)
            if [ -n "$next" ]; then
              dispatch_implement "$next" 1 ""
              echo "Dispatched agent-implement for issue #${next}"
            fi
          else
            echo "Pipeline is busy. Issue #${ISSUE_NUMBER} is queued."
          fi
```

- [ ] **Step 2: Validate workflow YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/agent-dispatcher.yml'))"`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/agent-dispatcher.yml
git commit -m "feat: add agent dispatcher workflow with FIFO queue"
```

---

### Task 3: Agent Implement Workflow

**Files:**
- Create: `.github/workflows/agent-implement.yml`

The core workflow: triages the issue, creates a branch, runs Claude Code to implement, validates locally, creates a PR, dispatches review.

- [ ] **Step 1: Create the implement workflow**

```yaml
# .github/workflows/agent-implement.yml
name: "Agent: Implement"

on:
  repository_dispatch:
    types: [agent-implement]

permissions:
  contents: write
  issues: write
  pull-requests: write
  actions: write

jobs:
  implement:
    runs-on: ubuntu-latest
    timeout-minutes: 45
    env:
      GH_REPO: ${{ github.repository }}
      ISSUE_NUMBER: ${{ github.event.client_payload.issue_number }}
      ATTEMPT: ${{ github.event.client_payload.attempt || '1' }}
      REVIEW_FEEDBACK: ${{ github.event.client_payload.review_feedback || '' }}

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Generate impl App token
        id: app-token
        uses: actions/create-github-app-token@v1
        with:
          app-id: ${{ secrets.AGENT_IMPL_APP_ID }}
          private-key: ${{ secrets.AGENT_IMPL_PRIVATE_KEY }}

      - name: Fetch issue details
        id: issue
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          ISSUE_JSON=$(gh issue view "$ISSUE_NUMBER" --repo "$GH_REPO" --json title,body,labels)
          TITLE=$(echo "$ISSUE_JSON" | jq -r '.title')
          BODY=$(echo "$ISSUE_JSON" | jq -r '.body')
          LABELS=$(echo "$ISSUE_JSON" | jq -r '[.labels[].name] | join(",")')
          echo "title=$TITLE" >> "$GITHUB_OUTPUT"
          # Use temp files for multi-line values
          echo "$BODY" > /tmp/issue_body.txt
          echo "$LABELS" >> "$GITHUB_OUTPUT"

      - name: Triage issue
        id: triage
        uses: anthropic/claude-code-action@beta
        with:
          anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}
          prompt: |
            You are a triage agent. Read this GitHub issue and classify it.

            **Issue #${{ env.ISSUE_NUMBER }}: ${{ steps.issue.outputs.title }}**

            Body:
            $(cat /tmp/issue_body.txt)

            Classify this issue as one of:
            - ACTIONABLE: A bug report with enough detail to fix, a feature request with clear scope, or a refactoring task
            - NOT_ACTIONABLE: A question, discussion, incomplete report without enough detail to act on, or a meta-issue

            Respond with ONLY one line:
            ACTIONABLE: <brief reason>
            or
            NOT_ACTIONABLE: <brief reason why and what info is needed>
          max_turns: 1
          timeout_minutes: 2

      - name: Handle non-actionable issue
        if: contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          source .github/scripts/agent-queue.sh
          REASON=$(echo "${{ steps.triage.outputs.result }}" | sed 's/NOT_ACTIONABLE: //')
          gh issue comment "$ISSUE_NUMBER" --repo "$GH_REPO" \
            --body "I reviewed this issue but can't act on it yet.

          **Reason:** ${REASON}

          Once the issue has more detail, reopen it and I'll take another look."
          clear_processing
          dispatch_next_if_queued
          echo "Issue is not actionable. Skipping."
          exit 0

      - name: Create/reset branch
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          BRANCH="agent/issue-${ISSUE_NUMBER}"
          git checkout -B "$BRANCH" origin/main
          git push --force origin "$BRANCH"
          echo "BRANCH=$BRANCH" >> "$GITHUB_ENV"

      - name: Determine strategy
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        run: |
          case "$ATTEMPT" in
            1) STRATEGY="straightforward" ;;
            2) STRATEGY="deeper-analysis" ;;
            3) STRATEGY="decompose" ;;
            *) STRATEGY="straightforward" ;;
          esac
          echo "STRATEGY=$STRATEGY" >> "$GITHUB_ENV"

      - name: Build implementation prompt
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        run: |
          cat > /tmp/implement_prompt.txt << 'PROMPT_EOF'
          You are an autonomous coding agent working on the copypaste.fyi project.

          ## Your Task

          Implement a fix/feature for this GitHub issue:

          **Issue #ISSUE_NUM: ISSUE_TITLE**

          ISSUE_BODY_PLACEHOLDER

          ## Attempt Info

          This is attempt ATTEMPT_NUM/3 using strategy: STRATEGY_NAME

          FEEDBACK_SECTION

          ## Strategy Directives

          STRATEGY_DIRECTIVE

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
          PROMPT_EOF

          # Fill in placeholders
          ISSUE_BODY=$(cat /tmp/issue_body.txt)
          TITLE="${{ steps.issue.outputs.title }}"

          sed -i "s|ISSUE_NUM|${ISSUE_NUMBER}|g" /tmp/implement_prompt.txt
          sed -i "s|ISSUE_TITLE|${TITLE}|g" /tmp/implement_prompt.txt
          sed -i "s|ISSUE_BODY_PLACEHOLDER|${ISSUE_BODY}|g" /tmp/implement_prompt.txt
          sed -i "s|ATTEMPT_NUM|${ATTEMPT}|g" /tmp/implement_prompt.txt
          sed -i "s|STRATEGY_NAME|${STRATEGY}|g" /tmp/implement_prompt.txt

          # Add review feedback if retrying
          if [ -n "$REVIEW_FEEDBACK" ]; then
            sed -i "s|FEEDBACK_SECTION|## Previous Review Feedback\n\n${REVIEW_FEEDBACK}|g" /tmp/implement_prompt.txt
          else
            sed -i "s|FEEDBACK_SECTION||g" /tmp/implement_prompt.txt
          fi

          # Add strategy-specific directives
          case "$STRATEGY" in
            straightforward)
              sed -i "s|STRATEGY_DIRECTIVE|Read the issue, understand what's needed, implement the most direct fix.|g" /tmp/implement_prompt.txt
              ;;
            deeper-analysis)
              sed -i "s|STRATEGY_DIRECTIVE|The straightforward approach failed. Read more of the codebase extensively. Study related files and tests. Try a fundamentally different approach informed by the previous failure and review feedback.|g" /tmp/implement_prompt.txt
              ;;
            decompose)
              sed -i "s|STRATEGY_DIRECTIVE|Previous approaches failed. Break the problem into the smallest possible change that still addresses the core issue. Tackle only the simplest viable fix. If the issue is too complex for a single PR, implement what you can and document what remains.|g" /tmp/implement_prompt.txt
              ;;
          esac

      - name: Run Claude Code
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        id: claude
        uses: anthropic/claude-code-action@beta
        with:
          anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}
          prompt_file: /tmp/implement_prompt.txt
          max_turns: 30
          timeout_minutes: 35

      - name: Install Rust toolchain
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: stable
          components: clippy, rustfmt

      - name: Install cargo-nextest
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-nextest

      - name: Setup Node.js
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: frontend/package-lock.json

      - name: Install frontend deps
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        working-directory: frontend
        run: npm ci

      - name: Validate — cargo fmt
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        run: cargo fmt --all -- --check

      - name: Validate — cargo clippy
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Validate — cargo nextest
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        run: cargo nextest run --workspace --all-features

      - name: Validate — frontend tests
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        working-directory: frontend
        run: npm test -- --run

      - name: Validate — frontend lint
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        working-directory: frontend
        run: npm run lint

      - name: Push branch
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          git add -A
          git diff --cached --quiet && echo "No changes to commit" || \
            git commit -m "agent: implement fix for issue #${ISSUE_NUMBER} (attempt ${ATTEMPT})"
          git push --force origin "$BRANCH"

      - name: Create or update PR
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        id: pr
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          TITLE="${{ steps.issue.outputs.title }}"
          EXISTING_PR=$(gh pr list --repo "$GH_REPO" --head "$BRANCH" --json number --jq '.[0].number // empty')

          PR_BODY="## Closes #${ISSUE_NUMBER}

          ## What changed
          $(echo "${{ steps.claude.outputs.result }}" | head -50)

          ## Approach
          Attempt ${ATTEMPT}/3 — Strategy: ${STRATEGY}

          ## Testing
          All local validation passed: fmt, clippy, nextest, frontend tests, frontend lint."

          if [ -n "$EXISTING_PR" ]; then
            gh pr edit "$EXISTING_PR" --repo "$GH_REPO" \
              --body "$PR_BODY"
            echo "pr_number=$EXISTING_PR" >> "$GITHUB_OUTPUT"
            echo "Updated existing PR #${EXISTING_PR}"
          else
            PR_URL=$(gh pr create --repo "$GH_REPO" \
              --head "$BRANCH" \
              --base main \
              --title "fix: ${TITLE}" \
              --body "$PR_BODY")
            PR_NUM=$(echo "$PR_URL" | grep -oE '[0-9]+$')
            echo "pr_number=$PR_NUM" >> "$GITHUB_OUTPUT"
            echo "Created PR #${PR_NUM}"
          fi

      - name: Dispatch review
        if: "!contains(steps.triage.outputs.result, 'NOT_ACTIONABLE')"
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          source .github/scripts/agent-queue.sh
          dispatch_review "$ISSUE_NUMBER" "${{ steps.pr.outputs.pr_number }}" "$ATTEMPT"
          echo "Dispatched agent-review for PR #${{ steps.pr.outputs.pr_number }}"

      - name: Handle failure
        if: failure()
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          source .github/scripts/agent-queue.sh
          handle_failure "$ISSUE_NUMBER" "Implementation workflow failed on attempt ${ATTEMPT}. Check the workflow logs: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}"
```

- [ ] **Step 2: Validate workflow YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/agent-implement.yml'))"`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/agent-implement.yml
git commit -m "feat: add agent implement workflow with triage and escalation"
```

---

### Task 4: Agent Review Workflow

**Files:**
- Create: `.github/workflows/agent-review.yml`

Independent review using a separate GitHub App identity. Approves or requests changes, dispatching merge or retry accordingly.

- [ ] **Step 1: Create the review workflow**

```yaml
# .github/workflows/agent-review.yml
name: "Agent: Review"

on:
  repository_dispatch:
    types: [agent-review]

permissions:
  contents: write
  issues: write
  pull-requests: write
  actions: write

jobs:
  review:
    runs-on: ubuntu-latest
    timeout-minutes: 20
    env:
      GH_REPO: ${{ github.repository }}
      ISSUE_NUMBER: ${{ github.event.client_payload.issue_number }}
      PR_NUMBER: ${{ github.event.client_payload.pr_number }}
      ATTEMPT: ${{ github.event.client_payload.attempt || '1' }}

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Generate reviewer App token
        id: reviewer-token
        uses: actions/create-github-app-token@v1
        with:
          app-id: ${{ secrets.AGENT_REVIEWER_APP_ID }}
          private-key: ${{ secrets.AGENT_REVIEWER_PRIVATE_KEY }}

      - name: Generate impl App token (for dispatch/queue)
        id: impl-token
        uses: actions/create-github-app-token@v1
        with:
          app-id: ${{ secrets.AGENT_IMPL_APP_ID }}
          private-key: ${{ secrets.AGENT_IMPL_PRIVATE_KEY }}

      - name: Fetch PR diff and issue
        env:
          GH_TOKEN: ${{ steps.reviewer-token.outputs.token }}
        run: |
          gh pr diff "$PR_NUMBER" --repo "$GH_REPO" > /tmp/pr_diff.txt
          gh issue view "$ISSUE_NUMBER" --repo "$GH_REPO" --json title,body \
            --jq '"# Issue #" + (.number // "" | tostring) + ": " + .title + "\n\n" + .body' > /tmp/issue_details.txt

      - name: Run Claude Code review
        id: review
        uses: anthropic/claude-code-action@beta
        with:
          anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}
          max_turns: 15
          timeout_minutes: 15
          prompt: |
            You are an independent code reviewer for the copypaste.fyi project. You are a DIFFERENT agent from the one that wrote this code — review it critically.

            ## The Issue

            $(cat /tmp/issue_details.txt)

            ## The PR Diff

            ```diff
            $(cat /tmp/pr_diff.txt)
            ```

            ## Review Checklist — ALL must pass

            1. **Correctness**: Does the code actually solve what the issue asks for?
            2. **Security**: No injection vulnerabilities, unsafe unwraps, secret leaks, OWASP top 10 violations
            3. **Existing patterns**: Follows codebase conventions (check CLAUDE.md and agents.md)
            4. **Test coverage**: New/changed behavior has tests that would catch regressions
            5. **No scope creep**: Only changes what's needed — no unnecessary refactoring
            6. **Build confidence**: Changes look consistent with passing CI

            ## What NOT to review
            - Style/formatting (cargo fmt and clippy handle this)
            - "Nice to have" improvements
            - The approach itself, unless it's fundamentally broken

            ## Your response

            Respond with EXACTLY this format:

            DECISION: APPROVE
            SUMMARY: <brief summary of what the code does right>

            OR

            DECISION: REQUEST_CHANGES
            SUMMARY: <brief summary of issues>
            FEEDBACK:
            - <specific, actionable issue 1>
            - <specific, actionable issue 2>
            ...

      - name: Parse review decision
        id: parse
        run: |
          RESULT="${{ steps.review.outputs.result }}"
          if echo "$RESULT" | grep -q "DECISION: APPROVE"; then
            echo "decision=approve" >> "$GITHUB_OUTPUT"
          else
            echo "decision=request_changes" >> "$GITHUB_OUTPUT"
            # Extract feedback section
            FEEDBACK=$(echo "$RESULT" | sed -n '/^FEEDBACK:/,$ p' | tail -n +2)
            echo "$FEEDBACK" > /tmp/review_feedback.txt
          fi

      - name: Submit approving review
        if: steps.parse.outputs.decision == 'approve'
        env:
          GH_TOKEN: ${{ steps.reviewer-token.outputs.token }}
        run: |
          SUMMARY=$(echo "${{ steps.review.outputs.result }}" | sed -n 's/^SUMMARY: //p')
          gh api "repos/${GH_REPO}/pulls/${PR_NUMBER}/reviews" \
            -f event="APPROVE" \
            -f body="## Agent Review: Approved

          ${SUMMARY}

          All review checks passed: correctness, security, patterns, test coverage, scope."

      - name: Dispatch merge
        if: steps.parse.outputs.decision == 'approve'
        env:
          GH_TOKEN: ${{ steps.impl-token.outputs.token }}
        run: |
          source .github/scripts/agent-queue.sh
          dispatch_merge "$ISSUE_NUMBER" "$PR_NUMBER"
          echo "Dispatched agent-merge for PR #${PR_NUMBER}"

      - name: Submit changes-requested review
        if: steps.parse.outputs.decision == 'request_changes'
        env:
          GH_TOKEN: ${{ steps.reviewer-token.outputs.token }}
        run: |
          FEEDBACK=$(cat /tmp/review_feedback.txt)
          gh api "repos/${GH_REPO}/pulls/${PR_NUMBER}/reviews" \
            -f event="REQUEST_CHANGES" \
            -f body="## Agent Review: Changes Requested

          ${FEEDBACK}"

      - name: Dispatch retry or give up
        if: steps.parse.outputs.decision == 'request_changes'
        env:
          GH_TOKEN: ${{ steps.impl-token.outputs.token }}
        run: |
          source .github/scripts/agent-queue.sh
          FEEDBACK=$(cat /tmp/review_feedback.txt)
          NEXT_ATTEMPT=$((ATTEMPT + 1))

          if [ "$NEXT_ATTEMPT" -le 3 ]; then
            echo "Dispatching retry attempt ${NEXT_ATTEMPT}..."
            dispatch_implement "$ISSUE_NUMBER" "$NEXT_ATTEMPT" "$FEEDBACK"
          else
            echo "All 3 attempts exhausted. Marking as stuck."
            handle_failure "$ISSUE_NUMBER" "All 3 implementation attempts failed code review. The agent could not produce code that passes review for this issue."
          fi

      - name: Handle unexpected failure
        if: failure()
        env:
          GH_TOKEN: ${{ steps.impl-token.outputs.token }}
        run: |
          source .github/scripts/agent-queue.sh
          handle_failure "$ISSUE_NUMBER" "Review workflow failed unexpectedly. Check logs: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}"
```

- [ ] **Step 2: Validate workflow YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/agent-review.yml'))"`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/agent-review.yml
git commit -m "feat: add agent review workflow with independent reviewer identity"
```

---

### Task 5: Agent Merge Workflow

**Files:**
- Create: `.github/workflows/agent-merge.yml`

Waits for CI, verifies all gates, squash merges, cleans up, dispatches next issue.

- [ ] **Step 1: Create the merge workflow**

```yaml
# .github/workflows/agent-merge.yml
name: "Agent: Merge"

on:
  repository_dispatch:
    types: [agent-merge]

permissions:
  contents: write
  issues: write
  pull-requests: write
  actions: write

jobs:
  merge:
    runs-on: ubuntu-latest
    timeout-minutes: 20
    env:
      GH_REPO: ${{ github.repository }}
      ISSUE_NUMBER: ${{ github.event.client_payload.issue_number }}
      PR_NUMBER: ${{ github.event.client_payload.pr_number }}

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Generate impl App token
        id: app-token
        uses: actions/create-github-app-token@v1
        with:
          app-id: ${{ secrets.AGENT_IMPL_APP_ID }}
          private-key: ${{ secrets.AGENT_IMPL_PRIVATE_KEY }}

      - name: Wait for CI checks to complete
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          echo "Waiting for CI checks on PR #${PR_NUMBER}..."
          # Wait up to 15 minutes for checks to complete
          for i in $(seq 1 15); do
            CHECKS_JSON=$(gh pr checks "$PR_NUMBER" --repo "$GH_REPO" --json name,state,conclusion 2>/dev/null || echo "[]")

            if [ "$CHECKS_JSON" = "[]" ]; then
              echo "No checks found yet. Waiting... (attempt $i/15)"
              sleep 60
              continue
            fi

            # Check if all checks are complete (no 'pending' or 'queued' states)
            PENDING=$(echo "$CHECKS_JSON" | jq '[.[] | select(.state != "completed")] | length')
            if [ "$PENDING" -gt 0 ]; then
              echo "Still $PENDING checks running. Waiting... (attempt $i/15)"
              sleep 60
              continue
            fi

            echo "All checks complete."
            echo "$CHECKS_JSON" > /tmp/checks.json
            break
          done

          if [ ! -f /tmp/checks.json ]; then
            echo "ERROR: CI checks did not complete within 15 minutes."
            exit 1
          fi

      - name: Verify ALL CI checks passed (HARD GATE)
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          CHECKS_JSON=$(cat /tmp/checks.json)

          echo "=== CI Check Results ==="
          echo "$CHECKS_JSON" | jq -r '.[] | "\(.name): \(.conclusion)"'
          echo "========================"

          # HARD GATE: Every check must have conclusion "success"
          FAILED=$(echo "$CHECKS_JSON" | jq '[.[] | select(.conclusion != "success")] | length')
          if [ "$FAILED" -gt 0 ]; then
            echo "ERROR: $FAILED check(s) did not pass:"
            echo "$CHECKS_JSON" | jq -r '.[] | select(.conclusion != "success") | "  FAIL: \(.name) — \(.conclusion)"'
            exit 1
          fi

          # Verify required checks are present (not just that nothing failed)
          REQUIRED_CHECKS=("Check formatting" "Lint with clippy" "Run test suite (nextest)" "Coverage (llvm-cov)" "Frontend unit tests" "Frontend lint")
          ACTUAL_NAMES=$(echo "$CHECKS_JSON" | jq -r '.[].name')

          for check in "${REQUIRED_CHECKS[@]}"; do
            if ! echo "$ACTUAL_NAMES" | grep -qF "$check"; then
              echo "ERROR: Required check '${check}' is missing from CI results."
              echo "This indicates a CI configuration issue. Do not merge."
              exit 1
            fi
          done

          echo "ALL CI checks passed. All required checks present."

      - name: Verify approving review exists (HARD GATE)
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          REVIEWS=$(gh api "repos/${GH_REPO}/pulls/${PR_NUMBER}/reviews" --jq '.')

          # Check for at least one APPROVED review
          APPROVED=$(echo "$REVIEWS" | jq '[.[] | select(.state == "APPROVED")] | length')
          if [ "$APPROVED" -eq 0 ]; then
            echo "ERROR: No approving review found on PR #${PR_NUMBER}."
            exit 1
          fi

          # Check no CHANGES_REQUESTED reviews are outstanding
          CHANGES_REQUESTED=$(echo "$REVIEWS" | jq '[.[] | select(.state == "CHANGES_REQUESTED")] | length')
          if [ "$CHANGES_REQUESTED" -gt 0 ]; then
            echo "ERROR: There are outstanding change requests on PR #${PR_NUMBER}."
            exit 1
          fi

          echo "Review gate passed: ${APPROVED} approving review(s), no outstanding change requests."

      - name: Determine commit type
        id: commit-type
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          ISSUE_JSON=$(gh issue view "$ISSUE_NUMBER" --repo "$GH_REPO" --json labels,title)
          LABELS=$(echo "$ISSUE_JSON" | jq -r '[.labels[].name] | join(",")')
          TITLE=$(echo "$ISSUE_JSON" | jq -r '.title')

          # Determine conventional commit type from labels
          if echo "$LABELS" | grep -qiE "bug|fix"; then
            TYPE="fix"
          elif echo "$LABELS" | grep -qiE "feature|enhancement"; then
            TYPE="feat"
          elif echo "$LABELS" | grep -qiE "docs|documentation"; then
            TYPE="docs"
          elif echo "$LABELS" | grep -qiE "refactor"; then
            TYPE="refactor"
          else
            # Default to fix
            TYPE="fix"
          fi

          echo "type=$TYPE" >> "$GITHUB_OUTPUT"
          echo "title=$TITLE" >> "$GITHUB_OUTPUT"
          echo "Commit type: $TYPE"

      - name: Squash merge
        id: merge
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          TYPE="${{ steps.commit-type.outputs.type }}"
          TITLE="${{ steps.commit-type.outputs.title }}"
          COMMIT_MSG="${TYPE}: resolve #${ISSUE_NUMBER} — ${TITLE}"

          echo "Merging PR #${PR_NUMBER} with message: ${COMMIT_MSG}"
          gh pr merge "$PR_NUMBER" --repo "$GH_REPO" \
            --squash \
            --subject "$COMMIT_MSG" \
            --delete-branch

      - name: Close issue with comment
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          gh issue close "$ISSUE_NUMBER" --repo "$GH_REPO" \
            --comment "Resolved in PR #${PR_NUMBER}. Merged to main."

      - name: Dispatch next issue
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          source .github/scripts/agent-queue.sh
          clear_processing
          dispatch_next_if_queued
          echo "Queue advanced. Pipeline ready for next issue."

      - name: Handle merge conflict (retry with rebase)
        if: failure() && steps.merge.outcome == 'failure'
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          echo "Merge failed. Attempting rebase onto main..."
          BRANCH=$(gh pr view "$PR_NUMBER" --repo "$GH_REPO" --json headRefName --jq '.headRefName')
          git fetch origin main "$BRANCH"
          git checkout "$BRANCH"
          if git rebase origin/main; then
            git push --force origin "$BRANCH"
            echo "Rebase succeeded. Re-attempting merge..."
            gh pr merge "$PR_NUMBER" --repo "$GH_REPO" \
              --squash \
              --subject "${{ steps.commit-type.outputs.type }}: resolve #${ISSUE_NUMBER} — ${{ steps.commit-type.outputs.title }}" \
              --delete-branch
            gh issue close "$ISSUE_NUMBER" --repo "$GH_REPO" \
              --comment "Resolved in PR #${PR_NUMBER}. Merged to main (after rebase)."
            source .github/scripts/agent-queue.sh
            clear_processing
            dispatch_next_if_queued
          else
            echo "Rebase failed. Marking as stuck."
            source .github/scripts/agent-queue.sh
            handle_failure "$ISSUE_NUMBER" "Merge failed due to conflicts and automatic rebase also failed. Manual intervention required."
          fi

      - name: Handle unexpected failure
        if: failure() && steps.merge.outcome != 'failure'
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
        run: |
          source .github/scripts/agent-queue.sh
          handle_failure "$ISSUE_NUMBER" "Merge workflow failed unexpectedly. Check logs: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}"
```

- [ ] **Step 2: Validate workflow YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/agent-merge.yml'))"`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/agent-merge.yml
git commit -m "feat: add agent merge workflow with hard CI gate verification"
```

---

### Task 6: Agent Recovery Workflow

**Files:**
- Create: `.github/workflows/agent-recovery.yml`

Hourly cron job that detects stuck queues and recovers automatically.

- [ ] **Step 1: Create the recovery workflow**

```yaml
# .github/workflows/agent-recovery.yml
name: "Agent: Recovery"

on:
  schedule:
    - cron: '0 * * * *'  # every hour
  workflow_dispatch:       # manual trigger for testing

permissions:
  contents: write
  issues: write
  actions: write

jobs:
  recover:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Generate impl App token
        id: app-token
        uses: actions/create-github-app-token@v1
        with:
          app-id: ${{ secrets.AGENT_IMPL_APP_ID }}
          private-key: ${{ secrets.AGENT_IMPL_PRIVATE_KEY }}

      - name: Check for stuck queue
        env:
          GH_TOKEN: ${{ steps.app-token.outputs.token }}
          GH_REPO: ${{ github.repository }}
        run: |
          source .github/scripts/agent-queue.sh

          # Check if queue branch exists
          if ! gh api "repos/${GH_REPO}/git/ref/heads/agent-queue" &>/dev/null; then
            echo "No agent-queue branch. Nothing to recover."
            exit 0
          fi

          QUEUE_JSON=$(read_queue)
          PROCESSING=$(echo "$QUEUE_JSON" | jq -r '.processing // empty')
          STARTED_AT=$(echo "$QUEUE_JSON" | jq -r '.started_at // empty')

          if [ -z "$PROCESSING" ]; then
            # Check if there are queued items with no processing
            QUEUED=$(echo "$QUEUE_JSON" | jq '.queue | length')
            if [ "$QUEUED" -gt 0 ]; then
              echo "Found $QUEUED queued items with no active processing. Dispatching next..."
              dispatch_next_if_queued
            else
              echo "Queue is empty and idle. Nothing to recover."
            fi
            exit 0
          fi

          echo "Currently processing issue #${PROCESSING}, started at ${STARTED_AT}"

          # Check if started_at is older than 90 minutes
          if [ -n "$STARTED_AT" ]; then
            STARTED_EPOCH=$(date -d "$STARTED_AT" +%s 2>/dev/null || date -jf "%Y-%m-%dT%H:%M:%SZ" "$STARTED_AT" +%s 2>/dev/null || echo "0")
            NOW_EPOCH=$(date +%s)
            AGE_MINUTES=$(( (NOW_EPOCH - STARTED_EPOCH) / 60 ))

            if [ "$AGE_MINUTES" -ge 90 ]; then
              echo "Issue #${PROCESSING} has been processing for ${AGE_MINUTES} minutes. This is stuck."
              handle_failure "$PROCESSING" "Pipeline stuck: processing exceeded 90-minute timeout. Recovered by hourly recovery job."
            else
              echo "Issue #${PROCESSING} has been processing for ${AGE_MINUTES} minutes. Still within timeout."
            fi
          else
            echo "No started_at timestamp. Clearing stuck state."
            handle_failure "$PROCESSING" "Pipeline stuck: no timestamp found. Recovered by hourly recovery job."
          fi
```

- [ ] **Step 2: Validate workflow YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/agent-recovery.yml'))"`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/agent-recovery.yml
git commit -m "feat: add agent recovery workflow for stuck queue detection"
```

---

### Task 7: Create the `agent-stuck` Label

**Files:** None (GitHub API call)

- [ ] **Step 1: Create the label via gh CLI**

Run:
```bash
gh label create "agent-stuck" --repo <owner/repo> --color "d73a4a" --description "Agent pipeline could not resolve this issue" --force
```
Expected: Label created (or already exists)

- [ ] **Step 2: Verify label exists**

Run: `gh label list --repo <owner/repo> | grep agent-stuck`
Expected: Shows the `agent-stuck` label

- [ ] **Step 3: Commit** (no file changes — label is in GitHub, not the repo)

---

### Task 8: Initialize the `agent-queue` Branch

**Files:** None (done via git)

- [ ] **Step 1: Create the orphan branch with initial queue file**

```bash
git checkout --orphan agent-queue
git rm -rf .
echo '{"queue":[],"processing":null,"started_at":null}' > queue.json
git add queue.json
git commit -m "Initialize agent queue"
git push origin agent-queue
git checkout main
```

- [ ] **Step 2: Verify the branch exists**

Run: `git branch -r | grep agent-queue`
Expected: `origin/agent-queue`

---

### Task 9: Integration Test — End to End

**Files:** None (manual verification)

This task verifies the full pipeline works before considering it done.

- [ ] **Step 1: Verify all workflow files are committed and pushed**

Run: `git log --oneline -10`
Expected: Commits for all 5 workflows + queue script

Run: `ls .github/workflows/agent-*.yml`
Expected:
```
.github/workflows/agent-dispatcher.yml
.github/workflows/agent-implement.yml
.github/workflows/agent-merge.yml
.github/workflows/agent-recovery.yml
.github/workflows/agent-review.yml
```

Run: `ls .github/scripts/agent-queue.sh`
Expected: File exists and is executable

- [ ] **Step 2: Verify GitHub repo settings**

Check manually:
1. Settings → Actions → General → "Allow GitHub Actions to create and approve pull requests" is ON
2. Settings → General → "Allow auto-merge" is ON
3. Secrets are configured: `ANTHROPIC_API_KEY`, `AGENT_IMPL_APP_ID`, `AGENT_IMPL_PRIVATE_KEY`, `AGENT_REVIEWER_APP_ID`, `AGENT_REVIEWER_PRIVATE_KEY`

- [ ] **Step 3: Create a test issue**

Run:
```bash
gh issue create --repo <owner/repo> \
  --title "Test: Add a comment to the health endpoint handler" \
  --body "Add a code comment to the health endpoint handler in src/server/handlers.rs explaining what the endpoint does. This is a test of the agent pipeline."
```

- [ ] **Step 4: Monitor the pipeline**

Watch: `gh run list --repo <owner/repo> --workflow "Agent: Dispatcher" --limit 5`

Then: `gh run list --repo <owner/repo> --workflow "Agent: Implement" --limit 5`

Then: `gh run list --repo <owner/repo> --workflow "Agent: Review" --limit 5`

Then: `gh run list --repo <owner/repo> --workflow "Agent: Merge" --limit 5`

Expected: All workflows succeed in sequence. A PR is created, reviewed, and merged. The test issue is closed.

- [ ] **Step 5: Verify the merge**

Run: `gh pr list --repo <owner/repo> --state merged --limit 5`
Expected: The test PR appears as merged

Run: `gh issue view <test-issue-number> --repo <owner/repo> --json state`
Expected: `"state": "CLOSED"`
