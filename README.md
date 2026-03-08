# Vigil

Vigil is a Rust CLI that orchestrates scheduled task execution, tracks run history, and provides an escape hatch for human intervention when things go wrong. Initially focused on automating Claude Code skills (daily briefings, code reviews, etc.) on recurring schedules, its architecture cleanly separates runners (what to execute) from schedulers (when to trigger), making it extensible to shell commands, HTTP webhooks, and other execution backends.

## Quick start

```sh
# Register a shell task
vigil register shell hello "echo hello world"

# Register a Claude Code task
vigil register claude daily-briefing /daily-briefing --dir ~/Dev

# Run a task manually
vigil run hello

# Schedule recurring execution (backed by macOS launchd)
vigil schedule hello "every 5 minutes"
vigil schedule daily-briefing "weekdays at 09:00"

# Check task status and schedules
vigil status

# View recent runs
vigil runs

# Remove a schedule
vigil unschedule hello

# List registered tasks
vigil list

# Remove a task (also unschedules if needed)
vigil unregister hello
```

## Scheduling

Vigil uses macOS launchd as its scheduling backend. Trigger expressions support:

- `"daily at 09:00"` — every day at a specific time
- `"weekdays at 09:30"` — Monday through Friday
- `"weekends at 10:00"` — Saturday and Sunday
- `"mon,wed,fri at 14:00"` — specific days
- `"every 5 minutes"` — fixed intervals (hours, minutes, or seconds)

Schedules are persisted in the database and registered as launchd agents under `~/Library/LaunchAgents/com.vigil.task.*.plist`.

## Project structure

| Crate | Path | Description |
|---|---|---|
| `vigil-core` | `core/` | Shared traits and models (no I/O dependencies) |
| `vigil-runner-shell` | `runner-shell/` | Shell command executor |
| `vigil-runner-claude` | `runner-claude/` | Claude Code executor (skills and prompts) |
| `vigil-scheduler-launchd` | `scheduler-launchd/` | macOS launchd scheduling backend |
| `vigil-registry` | `registry/` | Composition root: store, runner dispatch, dyn glue |
| `vigil-cli` | `cli/` | Command-line interface (clap) |

## Status

Vigil is under active development. See [PLAN.md](PLAN.md) for build phases and progress, and [DESIGN.md](DESIGN.md) for the full architecture document.
