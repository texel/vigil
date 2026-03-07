# Vigil — Build Plan

## Phase 0: Repo docs
- [x] Rewrite DESIGN.md with revised architecture
- [x] Create PLAN.md (this file)

## Phase 1: Core traits + shell executor + manual run

Validates the architecture end-to-end with the simplest possible executor.

- [ ] `core/` — Task<C>, ExecutorConfig, Executor, Schedulable traits, Run model, TriggerSpec types, libSQL storage
- [ ] `executor-shell/` — ShellConfig, ShellExecutor
- [ ] `cli/` — register shell, run, list, unregister
- [ ] Workspace Cargo.toml

**End state:** `vigil register shell hello "echo hello world"` -> `vigil run hello` -> `vigil list` works.

## Phase 2: Claude executor

- [ ] `executor-claude/` — ClaudeConfig, ClaudeExecutor, session capture
- [ ] `cli/` additions — register claude, claude resume, top-level resume

**End state:** register a Claude skill, run it, resume a failed session.

## Phase 3: Scheduling

- [ ] `core/` additions — Scheduler trait, TriggerSpec parsing
- [ ] `scheduler-launchd/` — launchd backend (macOS)
- [ ] `cli/` additions — schedule, unschedule, status

**End state:** `vigil schedule daily-briefing "weekdays at 9:00"` wires up recurring execution.

## Phase 4: Polish

- [ ] logs command
- [ ] Notifications on failure
- [ ] Additional scheduler backends (cron, daemon)
- [ ] Log rotation / retention
