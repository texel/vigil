# Vigil

Vigil is a Rust CLI that orchestrates scheduled task execution, tracks run history, and provides an escape hatch for human intervention when things go wrong. Initially focused on automating Claude Code skills (daily briefings, code reviews, etc.) on recurring schedules, its architecture cleanly separates runners (what to execute) from schedulers (when to trigger), making it extensible to shell commands, HTTP webhooks, and other execution backends.

## Quick start

```sh
# Register a shell task
vigil register shell hello "echo hello world"

# Run it manually
vigil run hello

# List registered tasks
vigil list

# View recent runs
vigil runs

# Remove a task
vigil unregister hello
```

## Project structure

| Crate | Path | Description |
|---|---|---|
| `vigil-core` | `core/` | Shared traits and models (no I/O dependencies) |
| `vigil-runner-shell` | `runner-shell/` | Shell command executor |
| `vigil-registry` | `registry/` | Composition root: store, runner dispatch, dyn glue |
| `vigil-cli` | `cli/` | Command-line interface (clap) |

## Status

Vigil is under active development. See [PLAN.md](PLAN.md) for build phases and progress, and [DESIGN.md](DESIGN.md) for the full architecture document.
