# Vigil — Design Document

## Context

Claude Code skills (daily-briefing, weekly-review, pr-review, etc.) need to run on recurring schedules. Claude Code has no built-in scheduling. The CLI supports non-interactive execution (`claude -p`) with session persistence, and sessions can be resumed interactively (`claude --resume <id>`), enabling a "drop into a running container" recovery pattern.

Vigil is a Rust CLI that orchestrates scheduled task runs, tracks their state, and provides an escape hatch for human intervention when things go wrong.

Though initially focused on Claude Code, Vigil's architecture cleanly separates execution logic from scheduling and persistence. Executors and schedulers are independent extension axes — adding a new runner (Claude, shell, HTTP) or a new scheduler backend (launchd, cron, daemon) doesn't require changes to the other.

## Core Abstractions

### ScheduledTask

A task config paired with scheduling metadata. Generic over its task config type — `ScheduledTask<T>`. At the DB boundary, `T = RawTask` (untyped JSON). After deserialization, `T` is a concrete task type like `ShellTask` or `ClaudeTask`.

```rust
struct ScheduledTask<T> {
    id: Uuid,
    name: String,
    task: T,
    trigger: Option<TriggerSpec>,
    working_directory: Option<PathBuf>,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

struct RawTask {
    runner_type: String,
    json: String,
}
```

### Task and Runner

Config and behavior are separated. `Task` is pure serializable data describing what to run. `Runner` is stateless behavior that executes a task.

```rust
trait Task: Send + Sync + Serialize + DeserializeOwned + Debug {
    fn runner_type(&self) -> &'static str;
}

#[async_trait]
trait Runner {
    type Task: Task;
    async fn run(&self, task: &Self::Task, context: &RunContext) -> Result<RunOutput>;
}
```

### Schedulable

The dyn boundary for heterogeneous task collections. The scheduler and storage layers work with `Box<dyn Schedulable>` so they don't need to know about specific runner types.

```rust
#[async_trait]
trait Schedulable: Send + Sync {
    fn task_id(&self) -> Uuid;
    fn task_name(&self) -> &str;
    fn runner_type(&self) -> &'static str;
    fn trigger(&self) -> Option<&TriggerSpec>;
    async fn run(&self, context: &RunContext) -> Result<RunOutput>;
}
```

### TriggerSpec

Canonical scheduling representation, not tied to any backend. Scheduler backends compile it into platform-specific formats (cron strings, launchd CalendarIntervals, daemon tick timers).

```rust
enum TriggerSpec {
    Recurring {
        times: Vec<TimeOfDay>,
        days: Option<DayFilter>,
        timezone: Option<String>,
    },
    Interval {
        every: Duration,
    },
}
```

Minimal to start. Expressiveness can grow without breaking changes.

### Run

Tracks a single execution of a task.

```rust
struct Run {
    id: Uuid,
    task_id: Uuid,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    exit_code: Option<i32>,
    status: RunStatus,
    metadata_json: Option<String>,  // runner-specific (e.g., session_id for Claude)
    log_path: PathBuf,
    triggered_by: TriggerType,      // Manual, Schedule
}
```

### Scheduler trait

Backend-agnostic scheduling interface.

```rust
#[async_trait]
trait Scheduler: Send + Sync {
    async fn register(&self, task_id: Uuid, name: &str, trigger: &TriggerSpec) -> Result<()>;
    async fn unregister(&self, task_id: Uuid) -> Result<()>;
    async fn list_registered(&self) -> Result<Vec<Uuid>>;
}
```

Planned implementations:
- `LaunchdScheduler` — generates plists in ~/Library/LaunchAgents/. Default on macOS.
- `CronScheduler` — writes crontab entries. Fallback / Linux default.
- `DaemonScheduler` — built-in long-running process (`vigil daemon`). Most portable.

## Extensibility Model

Two independent axes:

- **Runners** (what to run): shell, Claude, future HTTP/webhook, etc.
- **Schedulers** (when to trigger): launchd, cron, daemon, future systemd, etc.

These are m+n, not m*n. A Claude runner doesn't know about launchd, and a launchd scheduler doesn't know about Claude. Both depend only on `vigil-core` traits.

Each axis gets its own crate, which:
- Prevents accidental dependency leakage
- Keeps platform-specific code (`cfg`) at crate boundaries
- Makes the extension points obvious from the dependency graph

Deserialization of heterogeneous tasks uses a match-based registry at the CLI boundary. Can evolve to `inventory` crate auto-registration later.

## Storage

**libSQL (Turso)** for the task registry and run history.

- Local-first: `Builder::new_local("~/.vigil/state.db")` — embedded, no server needed.
- Handles concurrent writes from simultaneous launchd jobs without contention.
- Evolution path: swap to `Builder::new_remote_replica(...)` for cloud sync.
- Still inspectable via `sqlite3` CLI.

Run log output stored as plain files at `~/.vigil/logs/<task-name>/<date>.json`.

## CLI Structure

Generic commands where runner is a subcommand:

```
vigil register claude <name> <skill-path> [--budget] [--allowed-tools]
vigil register shell <name> <command>
vigil run <name>
vigil list
vigil status
vigil logs <name>
vigil schedule <name> <trigger>
vigil unschedule <name>
vigil unregister <name>
```

Runner-specific namespaces for capabilities unique to that runner:

```
vigil claude resume <run-id>
```

Top-level convenience aliases that dispatch by looking up the runner type:

```
vigil resume <run-id>
```

## Execution Flow

1. `vigil run <name>` (or triggered by scheduler):
   - Load task from DB
   - Create Run record (status: Running)
   - Dispatch to the task's runner
   - Capture output to log file
   - On completion: update Run with exit code, status, completed_at
   - On failure: send notification with run ID for easy resume (if runner supports it)

2. `vigil resume <run-id>` (runner-specific, e.g. Claude):
   - Look up Run record, get runner-specific metadata (session_id)
   - Dispatch to runner's resume logic
   - User is now in an interactive session with full prior context

## Project Structure

```
vigil/
├── Cargo.toml              # workspace
├── DESIGN.md               # this document
├── PLAN.md                 # build phases and progress
├── core/                   # vigil-core: traits, models, storage
├── cli/                    # vigil-cli: clap, wiring, registry
├── runner-claude/          # vigil-runner-claude
├── runner-shell/           # vigil-runner-shell
├── scheduler-launchd/      # vigil-scheduler-launchd (macOS)
├── scheduler-cron/         # vigil-scheduler-cron (Linux)
└── scheduler-daemon/       # vigil-scheduler-daemon (portable)
```

Crate names use `vigil-*` prefix. Directory names drop the prefix since the repo is already `vigil/`.

## Key Dependencies

- `tokio` — async runtime
- `libsql` — database (Turso/libSQL)
- `clap` — CLI argument parsing
- `uuid` — task/run/session IDs
- `chrono` — datetime handling
- `serde` / `serde_json` — serialization
- `async-trait` — async fn in dyn-safe traits
- `anyhow` — error handling (with `.context()` at call sites)
- `tracing` — structured logging

## Session Persistence Notes (Claude)

- `claude -p` creates a persistent session by default
- Session ID extractable from `--output-format json` output (`jq -r '.session_id'`)
- Pre-assigned session IDs work via `--session-id <uuid>`
- `claude --resume <id>` drops into full interactive session with prior context
- `--allowedTools "Tool1,Tool2"` auto-approves specific tools (avoids permission prompts in headless mode)
- Sessions are directory-scoped (tied to working directory)

## Open Questions

- Does `--allowedTools` cover MCP tools (Things, Obsidian, etc.)? Needs testing.
- What's the right default budget per run? Needs experimentation.
- Should `vigil daemon` be a launchd-managed service itself?
- Log rotation / retention policy.
- Notification architecture: should notification backends be their own extension axis? Configurable per-task?

## Reference

- `claude-code-scheduler` (jshchnz/claude-code-scheduler): TypeScript Claude Code plugin with similar goals. Good reference for launchd plist generation. Key gap: no session awareness, no resume path, no budget controls.
