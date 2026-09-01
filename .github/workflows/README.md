# GitHub Actions

This is the Grok / agent map of every workflow. After a push, wait for **all** of them on that SHA — not only `ci.yml`. Invalid YAML fails as a 0-job run named `.github/workflows/<file>.yml`.

Lint locally: `python3 scripts/lint-workflows.py` (duplicate top-level keys are a hard fail).

| Workflow | Trigger | Merge blocker? | What it does |
|---|---|---|---|
| [ci.yml](ci.yml) | PR · push `main` (skips markdown-only) | **Yes** | fmt, clippy, tests, coverage ≥75%, frontend, workflow lint |
| [ocaml-ci.yml](ocaml-ci.yml) | `ocaml-crypto-verifier/**` | Yes, if that tree changed | OCaml 5.2 verifier tests |
| [deploy-fly.yml](deploy-fly.yml) | backend paths on `main` · manual | No (deploy) | `fly deploy` for Rocket + verifier |
| [docker-publish.yml](docker-publish.yml) | push `main` (skips markdown) | No | GHCR `edge` / `latest`. Concurrency cancels in-flight publishes — that is expected. |
| [release.yml](release.yml) | tag `v*` | n/a | CLI binaries for darwin/linux amd64+arm64 |
| [auto-research.yml](auto-research.yml) | **`workflow_dispatch` only** | Must not fail on push | Writes a competitor briefing issue. Do **not** add `schedule:` — daily runs flooded the tracker. |

## Agent rule

1. Run the jobs that cover your diff before push.
2. Open a PR. Poll `gh run list --commit <sha>` until every run is `completed`.
3. Red → fix on the same branch. Do not merge. Do not start something else.
4. After merge, `CI` on `main` must be green and Auto Research must not have run (or must not have failed).

Full product contract: [AGENTS.md](../../AGENTS.md).
