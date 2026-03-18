# Autonomous Issue-to-Merge Agent Pipeline

**Date**: 2026-03-17
**Status**: Design approved (revised after spec review)

## Overview

A fully autonomous GitHub Actions pipeline that picks up every new issue, triages it, implements a fix, creates a PR, reviews it with an independent agent identity, and auto-merges — with no human in the loop. Issues are processed sequentially to avoid merge conflicts using a dispatch queue.

## Architecture

Four GitHub Actions workflows form a chain, connected via `repository_dispatch` events using a GitHub App token (not `GITHUB_TOKEN`, which cannot trigger downstream workflows):

| Workflow | Trigger | Responsibility |
|----------|---------|---------------|
| `agent-dispatcher.yml` | `issues: [opened, reopened]` | Queue issues, dispatch one at a time |
| `agent-implement.yml` | `repository_dispatch: agent-implement` | Triage, branch, implement, test, create PR |
| `agent-review.yml` | `repository_dispatch: agent-review` | Independent code review, approve or request changes |
| `agent-merge.yml` | `repository_dispatch: agent-merge` | Verify gates, squash merge, close issue |

```
Issue Created
    │
    ▼
┌──────────────────────────┐
│  agent-dispatcher.yml     │  adds issue to queue file
│                           │  dispatches if no agent running
└──────────┬───────────────┘
           │ repository_dispatch
           ▼
┌──────────────────────────┐
│  agent-implement.yml      │
│                           │
│  1. Triage (Claude        │
│     classifies issue as   │
│     actionable or not)    │
│  2. Create branch         │
│     agent/issue-{num}     │
│  3. Claude Code:          │
│     - Analyze issue       │
│     - Write code          │
│     - Run full test suite │
│  4. Push branch           │
│  5. Create PR             │
│  6. Dispatch review       │
└──────────┬───────────────┘
           │ repository_dispatch
           ▼
┌──────────────────────────┐
│  agent-review.yml         │  uses REVIEWER App identity
│                           │
│  1. Fresh Claude Code     │
│     session               │
│  2. Review checklist:     │
│     - Correctness         │
│     - Security            │
│     - Style/patterns      │
│     - Issue alignment     │
│     - Test coverage       │
│  3. Approve or Request    │
│     Changes               │
└──────────┬───────────────┘
           │
     ┌─────┴─────┐
     │            │
  Approved    Changes
     │        Requested
     │            │
     │            ▼
     │     agent-implement.yml
     │     (retry via dispatch
     │      with review feedback,
     │      escalating strategy,
     │      max 3 attempts)
     │
     ▼
┌──────────────────────────┐
│  agent-merge.yml          │
│                           │
│  1. Wait for CI to pass   │
│     (poll status API)     │
│  2. Verify approving      │
│     review exists         │
│  3. Squash merge          │
│  4. Delete branch         │
│  5. Close issue           │
│  6. Dispatch next issue   │
│     from queue            │
└──────────────────────────┘
```

## GitHub App Setup

Two GitHub Apps are required to ensure workflow chaining works and that the review is a genuine second-party approval:

| App | Purpose | Permissions |
|-----|---------|-------------|
| **copypaste-agent-impl** | Implementation agent identity | Contents: write, Issues: write, Pull requests: write, Actions: write |
| **copypaste-agent-reviewer** | Review agent identity (separate actor) | Contents: write, Issues: write, Pull requests: write, Actions: write |

Each App's private key is stored as a repo secret. Workflows generate short-lived installation tokens at runtime using `actions/create-github-app-token`.

**Why two Apps**: GitHub does not allow the same actor to both author and approve a PR when branch protection requires reviews. Separate identities make the review a genuine second-party approval.

### Token Usage by Workflow

| Workflow | Token Used | Why |
|----------|-----------|-----|
| `agent-dispatcher.yml` | **impl App** | Needs to fire `repository_dispatch` (GITHUB_TOKEN can't trigger workflows) |
| `agent-implement.yml` | **impl App** | Creates branches, PRs, dispatches review |
| `agent-review.yml` (PR review) | **reviewer App** | Submits review as a different actor from the PR author |
| `agent-review.yml` (dispatch/queue) | **impl App** | Dispatches retry or merge, manages queue file |
| `agent-merge.yml` | **impl App** | Merges PR, manages queue, dispatches next issue |

Note: The review workflow generates **two** tokens — the reviewer token for submitting the review, and the impl token for all other operations (dispatch, queue, issue comments). This keeps the review genuinely independent while allowing the workflow to manage pipeline orchestration.

### Secrets

| Secret | Purpose |
|--------|---------|
| `ANTHROPIC_API_KEY` | Claude Code API calls (implementation + review) |
| `AGENT_IMPL_APP_ID` | GitHub App ID for implementation agent |
| `AGENT_IMPL_PRIVATE_KEY` | GitHub App private key for implementation agent |
| `AGENT_REVIEWER_APP_ID` | GitHub App ID for review agent |
| `AGENT_REVIEWER_PRIVATE_KEY` | GitHub App private key for review agent |

## Workflow 0: agent-dispatcher.yml

### Purpose

Solves the problem that GitHub Actions concurrency groups silently drop queued runs when more than one is pending. This dispatcher maintains a proper FIFO queue.

### Trigger

```yaml
on:
  issues:
    types: [opened, reopened]
```

### Queue Mechanism

Uses a JSON file in a dedicated branch (`agent-queue`) as the queue:

```json
{
  "queue": [
    { "issue_number": 42, "queued_at": "2026-03-17T10:00:00Z" },
    { "issue_number": 43, "queued_at": "2026-03-17T10:01:00Z" }
  ],
  "processing": null,
  "started_at": null
}
```

### Steps

1. **Filter**: Skip if issue author is the agent App bot user
2. **Append** the issue to the queue (commit to `agent-queue` branch)
3. **Check** if an agent is currently processing (the `processing` field)
4. If not processing: set `processing` to this issue and `started_at` to current timestamp, fire `repository_dispatch` event `agent-implement` with issue number as payload
5. If already processing: do nothing (the merge workflow will dispatch the next issue when it finishes)

### Concurrency

```yaml
concurrency:
  group: agent-dispatcher
  cancel-in-progress: false
```

This is safe because the dispatcher is fast (just a queue write + optional dispatch) — no risk of losing items. The concurrency group serialization also prevents conflicts on the queue file commit, since only one dispatcher run writes at a time.

## Workflow 1: agent-implement.yml

### Trigger

```yaml
on:
  repository_dispatch:
    types: [agent-implement]
```

Payload contains: `issue_number`, `attempt` (default 1), `review_feedback` (empty on first attempt).

### Triage Step

Before implementation, Claude Code classifies the issue:

1. Read issue title and body
2. Classify as one of:
   - **Actionable**: Bug report with enough detail, feature request with clear scope, refactoring task
   - **Not actionable**: Question, discussion, incomplete report, meta-issue
3. If not actionable: comment on the issue explaining why the agent can't act on it, suggest what information is needed, pop the queue, dispatch next issue
4. If actionable: proceed to implementation

### Implementation Steps

1. **Checkout** main at HEAD
2. **Create/reset branch** `agent/issue-{number}` — force-push from main HEAD on every attempt (clean slate avoids accumulated cruft from failed attempts)
3. **Determine attempt context**:
   - Attempt number from dispatch payload
   - Review feedback from dispatch payload (if retrying)
4. **Run Claude Code** with prompt containing:
   - Issue title and body (fetched fresh via API to catch edits)
   - `CLAUDE.md` and `agents.md` as context
   - Attempt number and previous review feedback (if retrying)
   - Explicit instruction to run the full test suite before finishing
   - Escalating strategy directive based on attempt number
5. **Validate** — run the full CI checks locally:
   - `cargo fmt --all -- --check`
   - `cargo clippy --all-targets --all-features -- -D warnings`
   - `cargo nextest run --workspace --all-features`
   - `cd frontend && npm test -- --run && npm run lint`
   - If validation fails, give Claude Code one more pass to fix
6. **Push branch** (force-push since we reset from main each attempt)
7. **Create or update PR** with structured body:
   ```markdown
   ## Closes #{issue_number}

   ## What changed
   <agent-generated summary>

   ## Approach
   <why this approach was chosen>

   ## Testing
   <what was tested, test output summary>

   ## Attempt
   {n}/3 — {strategy}
   ```
8. **Dispatch review**: Fire `repository_dispatch` event `agent-review` with `{ issue_number, pr_number, attempt }`

### Timeout

```yaml
timeout-minutes: 45
```

Prevents runaway invocations from burning unlimited credits.

### Escalating Strategy

- **Attempt 1 (Straightforward)**: Read issue, implement the most direct fix
- **Attempt 2 (Deeper analysis)**: Read more of the codebase, study related files and tests, try a fundamentally different approach informed by attempt 1's failure and review feedback
- **Attempt 3 (Decompose)**: Break into smaller changes, tackle the simplest viable fix that still addresses the core issue

### Failure After 3 Attempts

- Comment on the issue with a detailed analysis: what was tried, what failed, what's blocking
- Label the issue `agent-stuck`
- Pop the queue and dispatch the next issue

## Workflow 2: agent-review.yml

### Trigger

```yaml
on:
  repository_dispatch:
    types: [agent-review]
```

Payload contains: `issue_number`, `pr_number`, `attempt`.

### Authentication

Uses the **reviewer** GitHub App token — a different identity from the implementation agent. This ensures the approval is a genuine second-party review.

### Review Process

A fresh Claude Code session (independent from the implementation session) receives:
- The full PR diff (via `gh pr diff`)
- The linked issue body (via API)
- The codebase context (`CLAUDE.md`, `agents.md`)

### Review Checklist

All must pass for approval:

1. **Correctness**: Does the code solve what the issue asks for?
2. **Security**: No injection vulnerabilities, unsafe unwraps, secret leaks, OWASP top 10 violations
3. **Existing patterns**: Follows codebase conventions (trait-based storage, client-side encryption, module structure)
4. **Test coverage**: New/changed behavior has tests that would catch regressions
5. **No scope creep**: Only changes what's needed, no unnecessary refactoring or feature additions
6. **Build confidence**: Changes are consistent with a passing CI

### Decision

- **Approve**: All checks pass — submit an approving GitHub review using the reviewer App token, then dispatch `agent-merge` with `{ issue_number, pr_number }`
- **Request changes**: Post specific, actionable review comments on the PR. Then:
  - If attempt < 3: dispatch `agent-implement` with `{ issue_number, attempt: attempt+1, review_feedback: <structured feedback read from the review comments> }`
  - If attempt >= 3: comment on the issue that all attempts are exhausted, label `agent-stuck`, pop queue, dispatch next issue

### What the Review Does NOT Do

- Nitpick style (that's what `cargo fmt` and `clippy` enforce)
- Suggest "nice to have" improvements
- Rewrite the approach unless fundamentally broken

### Timeout

```yaml
timeout-minutes: 20
```

## Workflow 3: agent-merge.yml

### Trigger

```yaml
on:
  repository_dispatch:
    types: [agent-merge]
```

Payload contains: `issue_number`, `pr_number`.

### Gate Verification (HARD GATES — no exceptions)

**RULE: Never merge unless every single CI check has passed. No partial passes. No skipped checks. No "it's probably fine."**

Before merging, explicitly verify via API calls:

1. **CI status (HARD GATE)**:
   - Run `gh pr checks {pr_number} --watch --fail-level all` to wait for all checks to complete
   - After completion, run `gh pr checks {pr_number} --json name,state,conclusion` to get structured results
   - Parse the JSON: **every** check must have `conclusion: "success"`. If ANY check has `conclusion: "failure"`, `"cancelled"`, `"timed_out"`, or `"skipped"`, **do not merge**
   - Specifically verify these required checks passed:
     - `cargo fmt` (formatting)
     - `cargo clippy` (linting)
     - `cargo nextest` (tests)
     - `cargo llvm-cov` (75% coverage minimum)
     - Frontend `npm test` (Vitest)
     - Frontend `npm run lint` (ESLint)
   - If any required check is missing (not just failed, but absent), **do not merge** — this indicates a CI configuration issue
2. **Review status (HARD GATE)**: `gh api repos/{owner}/{repo}/pulls/{pr_number}/reviews` — at least one `APPROVED` review from the reviewer App. No `CHANGES_REQUESTED` reviews may be outstanding.
3. **If CI times out or fails**: The workflow fails, comments on the PR with which checks failed, labels the issue `agent-stuck`, pops the queue, dispatches next issue

### Merge Steps

1. **Determine conventional commit type**: Parse the issue labels and PR body to choose `fix:`, `feat:`, `docs:`, `refactor:`, etc. Default to `fix:` if ambiguous
2. **Squash merge** with message: `{type}: resolve #{issue_number} — {issue_title}`
3. **Delete branch** `agent/issue-{number}`
4. **Close issue** with comment: `Resolved in #{pr_number}. Merged to main.`
5. **Pop queue and dispatch next**: Update the queue file on the `agent-queue` branch — set `processing` to the next issue (if any) and fire `repository_dispatch` `agent-implement`

### Failure Handling

- If merge fails due to conflicts (main moved since branch was created): rebase onto main and retry once
- If rebase fails: comment on the PR explaining the conflict, label issue `agent-stuck`, pop queue, dispatch next

### Timeout

```yaml
timeout-minutes: 20
```

## Workflow Permissions

All workflows use:
```yaml
permissions:
  contents: write
  issues: write
  pull-requests: write
  actions: write          # needed for repository_dispatch
```

## Repository Settings Required

- **Allow GitHub Actions to create and approve pull requests**: Settings > Actions > General
- **Allow auto-merge**: Settings > General
- **Branch protection on `main`**: Require CI to pass + require 1 approving review. The reviewer App satisfies the review requirement. The implementation App is not in the bypass list, so it cannot self-approve.

## Loop Prevention

- Issues authored by either agent App bot user are skipped in the dispatcher
- Workflows only trigger via `repository_dispatch` — no event cascading through `GITHUB_TOKEN`
- Review workflow uses a separate App identity from the implement workflow
- Maximum 3 retry attempts per issue (tracked in dispatch payload)
- Queue mechanism prevents concurrent processing

## Cost Control

- Each Claude Code invocation has a `max_turns` limit (configurable, default ~30)
- Explicit `timeout-minutes` on all workflows (45 for implement, 20 for review, 20 for merge)
- 3-attempt retry cap prevents infinite loops
- Sequential queue ensures only one issue is processed at a time
- Predictable, linear API spend

## Queue Recovery

If a workflow crashes unexpectedly (runner failure, infrastructure error, timeout), the `processing` field in the queue file is never cleared, permanently blocking the pipeline.

### Recovery Mechanism: agent-recovery.yml

```yaml
on:
  schedule:
    - cron: '0 * * * *'   # every hour
```

Steps:
1. Read the queue file from the `agent-queue` branch
2. If `processing` is set and `processing.started_at` is older than 90 minutes, the pipeline is stuck
3. Clear `processing`, label the stuck issue with `agent-stuck`, comment with the failure context
4. Dispatch the next issue in the queue (if any)

### Workflow-Level Failure Cleanup

All workflows (implement, review, merge) include a final cleanup step:

```yaml
- name: Pop queue on failure
  if: failure()
  run: |
    # Clear processing, label issue agent-stuck, dispatch next
```

This handles most failures immediately. The hourly cron is a safety net for cases where even the `if: failure()` step doesn't run (e.g., runner killed mid-execution).

## Observability

- Each workflow step logs to GitHub Actions (visible in the Actions tab)
- Failed issues get the `agent-stuck` label for easy filtering
- PR bodies include attempt number and strategy for audit trail
- Queue state is inspectable on the `agent-queue` branch
- Consider adding a Slack/Discord webhook notification for `agent-stuck` events (future enhancement)
