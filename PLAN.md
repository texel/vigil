# Vigil — Build Plan

## Phase 0: Repo docs
- [x] Rewrite DESIGN.md with revised architecture
- [x] Create PLAN.md (this file)

## Phase 1: Core traits + shell executor + manual run

Validates the architecture end-to-end with the simplest possible executor.

- [x] `core/` — Task<C>, ExecutorConfig, Executor, Schedulable traits, Run model, TriggerSpec types, libSQL storage
- [x] `runner-shell/` — ShellTask, ShellRunner
- [x] `cli/` — register shell, run, list, unregister
- [x] Workspace Cargo.toml

**End state:** `vigil register shell hello "echo hello world"` -> `vigil run hello` -> `vigil list` works.

## Phase 1.1: Fixups

- [x] Bump `libsql` from 0.6 to 0.9
- [x] Switch `core/src/db.rs` from deprecated `Database::open()` to `Builder::new_local().build().await?`
- [x] `ExecutorConfig` trait -> `Task` trait (`ShellTask`, `ClaudeTask`)
- [x] `Executor` trait -> `Runner` trait, `.execute()` -> `.run()`
- [x] `Task<C>` struct -> `ScheduledTask<T>` (task + scheduling metadata)
- [x] `TaskRow` -> eliminated. DB layer returns `ScheduledTask<RawTask>`
- [x] `DynWrapper` -> `ConfiguredRunner` (runner bound to its task)
- [x] `ExecutorConfigDyn` trait -> `Runnable` trait
- [x] `executor_type()` -> `runner_type()` throughout
- [x] Rename `core/src/executor.rs` -> `core/src/runner.rs`
- [x] Replace `SELECT *` with explicit column lists
- [x] `Weekday` -> `DayOfWeek`
- [x] DB column `executor_type` -> `runner_type`
- [x] Extract `vigil-registry` crate (Store + Runnable + ConfiguredRunner + deserialize_task)
- [x] Strip core of DB/libsql dependency — core is now pure traits and models
- [x] Remove speculative `Schedulable` trait (unused)
- [x] Rename `executor-shell/` directory -> `runner-shell/`, crate `vigil-executor-shell` -> `vigil-runner-shell`

## Phase 2: Claude runner

- [ ] `runner-claude/` — ClaudeTask, ClaudeRunner, session capture
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
