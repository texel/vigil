use vigil_core::models::{RawTask, Run, ScheduledTask, TriggerSpec};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use libsql::{params, Connection, Database};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct Store {
    db: Database,
}

impl Store {
    pub async fn open(path: &Path) -> Result<Self> {
        let db = libsql::Builder::new_local(path)
            .build()
            .await
            .context("failed to open database")?;
        let store = Self { db };
        store.migrate().await?;
        Ok(store)
    }

    async fn conn(&self) -> Result<Connection> {
        self.db.connect().context("failed to connect to database")
    }

    async fn migrate(&self) -> Result<()> {
        let conn = self.conn().await?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                runner_type TEXT NOT NULL,
                config_json TEXT NOT NULL,
                trigger_json TEXT,
                working_directory TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS runs (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL REFERENCES tasks(id),
                started_at TEXT NOT NULL,
                completed_at TEXT,
                exit_code INTEGER,
                status TEXT NOT NULL,
                metadata_json TEXT,
                log_path TEXT NOT NULL,
                triggered_by TEXT NOT NULL
            );",
        )
        .await
        .context("failed to run migrations")?;
        Ok(())
    }

    pub async fn insert_task(&self, task: &ScheduledTask<RawTask>) -> Result<()> {
        let conn = self.conn().await?;
        conn.execute(
            "INSERT INTO tasks (id, name, runner_type, config_json, trigger_json, working_directory, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                task.id.to_string(),
                task.name.clone(),
                task.task.runner_type.clone(),
                task.task.json.clone(),
                task.trigger.as_ref().map(|t| serde_json::to_string(t).unwrap()),
                task.working_directory.as_ref().map(|p| p.to_string_lossy().to_string()),
                task.enabled as i32,
                task.created_at.to_rfc3339(),
                task.updated_at.to_rfc3339(),
            ],
        )
        .await
        .context("failed to insert task")?;
        Ok(())
    }

    pub async fn get_task_by_name(&self, name: &str) -> Result<Option<ScheduledTask<RawTask>>> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT id, name, runner_type, config_json, trigger_json, working_directory, enabled, created_at, updated_at
                 FROM tasks WHERE name = ?1",
                params![name],
            )
            .await
            .context("failed to query task")?;

        match rows.next().await? {
            Some(row) => Ok(Some(scheduled_task_from_row(&row)?)),
            None => Ok(None),
        }
    }

    pub async fn list_tasks(&self) -> Result<Vec<ScheduledTask<RawTask>>> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT id, name, runner_type, config_json, trigger_json, working_directory, enabled, created_at, updated_at
                 FROM tasks ORDER BY name",
                params![],
            )
            .await
            .context("failed to list tasks")?;

        let mut tasks = Vec::new();
        while let Some(row) = rows.next().await? {
            tasks.push(scheduled_task_from_row(&row)?);
        }
        Ok(tasks)
    }

    pub async fn update_trigger(&self, id: Uuid, trigger: Option<&TriggerSpec>) -> Result<()> {
        let conn = self.conn().await?;
        let trigger_json = trigger.map(|t| serde_json::to_string(t).unwrap());
        conn.execute(
            "UPDATE tasks SET trigger_json = ?1, updated_at = ?2 WHERE id = ?3",
            params![trigger_json, Utc::now().to_rfc3339(), id.to_string()],
        )
        .await
        .context("failed to update trigger")?;
        Ok(())
    }

    pub async fn delete_task(&self, id: Uuid) -> Result<()> {
        let conn = self.conn().await?;
        conn.execute(
            "DELETE FROM runs WHERE task_id = ?1",
            params![id.to_string()],
        )
        .await
        .context("failed to delete runs for task")?;
        conn.execute("DELETE FROM tasks WHERE id = ?1", params![id.to_string()])
            .await
            .context("failed to delete task")?;
        Ok(())
    }

    pub async fn insert_run(&self, run: &Run) -> Result<()> {
        let conn = self.conn().await?;
        let metadata_json = if run.metadata.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&run.metadata)?)
        };
        conn.execute(
            "INSERT INTO runs (id, task_id, started_at, completed_at, exit_code, status, metadata_json, log_path, triggered_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.id.to_string(),
                run.task_id.to_string(),
                run.started_at.to_rfc3339(),
                run.completed_at.map(|t| t.to_rfc3339()),
                run.exit_code,
                run.status.to_string(),
                metadata_json,
                run.log_path.to_string_lossy().to_string(),
                run.triggered_by.to_string(),
            ],
        )
        .await
        .context("failed to insert run")?;
        Ok(())
    }

    pub async fn update_run(&self, run: &Run) -> Result<()> {
        let conn = self.conn().await?;
        let metadata_json = if run.metadata.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&run.metadata)?)
        };
        conn.execute(
            "UPDATE runs SET completed_at = ?1, exit_code = ?2, status = ?3, metadata_json = ?4 WHERE id = ?5",
            params![
                run.completed_at.map(|t| t.to_rfc3339()),
                run.exit_code,
                run.status.to_string(),
                metadata_json,
                run.id.to_string(),
            ],
        )
        .await
        .context("failed to update run")?;
        Ok(())
    }

    pub async fn list_recent_runs(&self, task_name: Option<&str>, limit: u32) -> Result<Vec<(Run, String)>> {
        let conn = self.conn().await?;
        let mut rows = match task_name {
            Some(name) => conn.query(
                "SELECT r.id, r.task_id, r.started_at, r.completed_at, r.exit_code, r.status, r.metadata_json, r.log_path, r.triggered_by, t.name
                 FROM runs r JOIN tasks t ON r.task_id = t.id
                 WHERE t.name = ?1
                 ORDER BY r.started_at DESC LIMIT ?2",
                params![name, limit],
            ).await,
            None => conn.query(
                "SELECT r.id, r.task_id, r.started_at, r.completed_at, r.exit_code, r.status, r.metadata_json, r.log_path, r.triggered_by, t.name
                 FROM runs r JOIN tasks t ON r.task_id = t.id
                 ORDER BY r.started_at DESC LIMIT ?1",
                params![limit],
            ).await,
        }.context("failed to query runs")?;
        let mut results = Vec::new();
        while let Some(row) = rows.next().await? {
            let run = run_from_row(&row)?;
            let name: String = row.get(9)?;
            results.push((run, name));
        }
        Ok(results)
    }

    pub async fn get_run_by_id(&self, id: Uuid) -> Result<Option<Run>> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT id, task_id, started_at, completed_at, exit_code, status, metadata_json, log_path, triggered_by
                 FROM runs WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .context("failed to query run")?;

        match rows.next().await? {
            Some(row) => Ok(Some(run_from_row(&row)?)),
            None => Ok(None),
        }
    }

    pub async fn get_task_by_id(&self, id: Uuid) -> Result<Option<ScheduledTask<RawTask>>> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT id, name, runner_type, config_json, trigger_json, working_directory, enabled, created_at, updated_at
                 FROM tasks WHERE id = ?1",
                params![id.to_string()],
            )
            .await
            .context("failed to query task")?;

        match rows.next().await? {
            Some(row) => Ok(Some(scheduled_task_from_row(&row)?)),
            None => Ok(None),
        }
    }

    pub async fn get_run_by_id_prefix(&self, prefix: &str) -> Result<Option<Run>> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT id, task_id, started_at, completed_at, exit_code, status, metadata_json, log_path, triggered_by
                 FROM runs WHERE id LIKE ?1",
                params![format!("{prefix}%")],
            )
            .await
            .context("failed to query run by prefix")?;

        let first = match rows.next().await? {
            Some(row) => run_from_row(&row)?,
            None => return Ok(None),
        };

        if rows.next().await?.is_some() {
            bail!("ambiguous run ID prefix '{prefix}' — matches multiple runs, use more characters");
        }

        Ok(Some(first))
    }

    pub async fn get_runs_for_task(&self, task_id: Uuid, limit: u32) -> Result<Vec<Run>> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT id, task_id, started_at, completed_at, exit_code, status, metadata_json, log_path, triggered_by
                 FROM runs WHERE task_id = ?1 ORDER BY started_at DESC LIMIT ?2",
                params![task_id.to_string(), limit],
            )
            .await
            .context("failed to query runs")?;

        let mut runs = Vec::new();
        while let Some(row) = rows.next().await? {
            runs.push(run_from_row(&row)?);
        }
        Ok(runs)
    }
}

fn scheduled_task_from_row(row: &libsql::Row) -> Result<ScheduledTask<RawTask>> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let runner_type: String = row.get(2)?;
    let config_json: String = row.get(3)?;
    let trigger_json: Option<String> = row.get(4)?;
    let working_directory: Option<String> = row.get(5)?;
    let enabled: i32 = row.get(6)?;
    let created_at: String = row.get(7)?;
    let updated_at: String = row.get(8)?;

    Ok(ScheduledTask {
        id: id.parse().context("invalid task id")?,
        name,
        task: RawTask {
            runner_type,
            json: config_json,
        },
        trigger: trigger_json
            .map(|j| serde_json::from_str(&j))
            .transpose()
            .context("invalid trigger JSON")?,
        working_directory: working_directory.map(PathBuf::from),
        enabled: enabled != 0,
        created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
    })
}

fn run_from_row(row: &libsql::Row) -> Result<Run> {
    let id: String = row.get(0)?;
    let task_id: String = row.get(1)?;
    let started_at: String = row.get(2)?;
    let completed_at: Option<String> = row.get(3)?;
    let exit_code: Option<i32> = row.get(4)?;
    let status: String = row.get(5)?;
    let metadata_json: Option<String> = row.get(6)?;
    let log_path: String = row.get(7)?;
    let triggered_by: String = row.get(8)?;

    let metadata: HashMap<String, serde_json::Value> = match metadata_json {
        Some(json) => serde_json::from_str(&json)?,
        None => HashMap::new(),
    };

    Ok(Run {
        id: id.parse().context("invalid run id")?,
        task_id: task_id.parse().context("invalid task id")?,
        started_at: DateTime::parse_from_rfc3339(&started_at)?.with_timezone(&Utc),
        completed_at: completed_at
            .map(|t| DateTime::parse_from_rfc3339(&t).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?,
        exit_code,
        status: status.parse()?,
        metadata,
        log_path: PathBuf::from(log_path),
        triggered_by: triggered_by.parse()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vigil_core::models::{DayFilter, RunStatus, TimeOfDay, TriggerSpec, TriggerType};

    async fn test_store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Store::open(&db_path).await.unwrap();
        (store, dir)
    }

    fn make_task(name: &str) -> ScheduledTask<RawTask> {
        ScheduledTask {
            id: Uuid::new_v4(),
            name: name.to_string(),
            task: RawTask {
                runner_type: "shell".to_string(),
                json: r#"{"command":"echo test"}"#.to_string(),
            },
            trigger: None,
            working_directory: None,
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_run(task_id: Uuid) -> Run {
        Run {
            id: Uuid::new_v4(),
            task_id,
            started_at: Utc::now(),
            completed_at: None,
            exit_code: None,
            status: RunStatus::Running,
            metadata: HashMap::new(),
            log_path: PathBuf::from("/tmp/test.log"),
            triggered_by: TriggerType::Manual,
        }
    }

    #[tokio::test]
    async fn insert_and_get_task() {
        let (store, _dir) = test_store().await;
        let task = make_task("hello");
        store.insert_task(&task).await.unwrap();

        let fetched = store.get_task_by_name("hello").await.unwrap().unwrap();
        assert_eq!(fetched.name, "hello");
        assert_eq!(fetched.task.runner_type, "shell");
    }

    #[tokio::test]
    async fn list_tasks() {
        let (store, _dir) = test_store().await;
        store.insert_task(&make_task("bravo")).await.unwrap();
        store.insert_task(&make_task("alpha")).await.unwrap();

        let tasks = store.list_tasks().await.unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "alpha");
        assert_eq!(tasks[1].name, "bravo");
    }

    #[tokio::test]
    async fn delete_task_cascades_runs() {
        let (store, _dir) = test_store().await;
        let task = make_task("doomed");
        store.insert_task(&task).await.unwrap();
        store.insert_run(&make_run(task.id)).await.unwrap();

        store.delete_task(task.id).await.unwrap();
        assert!(store.get_task_by_name("doomed").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn update_trigger() {
        let (store, _dir) = test_store().await;
        let task = make_task("scheduled");
        store.insert_task(&task).await.unwrap();

        let trigger = TriggerSpec::Recurring {
            times: vec![TimeOfDay { hour: 9, minute: 0 }],
            days: Some(DayFilter::Weekdays),
            timezone: None,
        };
        store.update_trigger(task.id, Some(&trigger)).await.unwrap();

        let fetched = store.get_task_by_name("scheduled").await.unwrap().unwrap();
        assert!(fetched.trigger.is_some());

        store.update_trigger(task.id, None).await.unwrap();
        let fetched = store.get_task_by_name("scheduled").await.unwrap().unwrap();
        assert!(fetched.trigger.is_none());
    }

    #[tokio::test]
    async fn insert_and_get_runs() {
        let (store, _dir) = test_store().await;
        let task = make_task("runner");
        store.insert_task(&task).await.unwrap();
        let run = make_run(task.id);
        store.insert_run(&run).await.unwrap();

        let runs = store.get_runs_for_task(task.id, 10).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, RunStatus::Running);
    }

    #[tokio::test]
    async fn list_recent_runs_filters_by_name() {
        let (store, _dir) = test_store().await;
        let t1 = make_task("task-a");
        let t2 = make_task("task-b");
        store.insert_task(&t1).await.unwrap();
        store.insert_task(&t2).await.unwrap();
        store.insert_run(&make_run(t1.id)).await.unwrap();
        store.insert_run(&make_run(t2.id)).await.unwrap();

        let runs = store.list_recent_runs(Some("task-a"), 10).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].1, "task-a");
    }

    #[tokio::test]
    async fn list_recent_runs_respects_limit() {
        let (store, _dir) = test_store().await;
        let task = make_task("many-runs");
        store.insert_task(&task).await.unwrap();
        for _ in 0..3 {
            store.insert_run(&make_run(task.id)).await.unwrap();
        }

        let runs = store.list_recent_runs(None, 2).await.unwrap();
        assert_eq!(runs.len(), 2);
    }

    #[tokio::test]
    async fn update_run() {
        let (store, _dir) = test_store().await;
        let task = make_task("updatable");
        store.insert_task(&task).await.unwrap();
        let mut run = make_run(task.id);
        store.insert_run(&run).await.unwrap();

        run.completed_at = Some(Utc::now());
        run.status = RunStatus::Succeeded;
        run.exit_code = Some(0);
        store.update_run(&run).await.unwrap();

        let runs = store.get_runs_for_task(task.id, 10).await.unwrap();
        assert_eq!(runs[0].status, RunStatus::Succeeded);
        assert_eq!(runs[0].exit_code, Some(0));
        assert!(runs[0].completed_at.is_some());
    }
}
