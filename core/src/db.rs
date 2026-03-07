use crate::models::Run;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use libsql::{params, Connection, Database};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Raw task row from the database, before executor-specific deserialization.
#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: Uuid,
    pub name: String,
    pub executor_type: String,
    pub config_json: String,
    pub trigger_json: Option<String>,
    pub working_directory: Option<PathBuf>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct Store {
    db: Database,
}

impl Store {
    pub async fn open(path: &Path) -> Result<Self> {
        #[allow(deprecated)]
        let db = Database::open(path.to_string_lossy().as_ref())
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
                executor_type TEXT NOT NULL,
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

    pub async fn insert_task(&self, row: &TaskRow) -> Result<()> {
        let conn = self.conn().await?;
        conn.execute(
            "INSERT INTO tasks (id, name, executor_type, config_json, trigger_json, working_directory, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.id.to_string(),
                row.name.clone(),
                row.executor_type.clone(),
                row.config_json.clone(),
                row.trigger_json.clone(),
                row.working_directory.as_ref().map(|p| p.to_string_lossy().to_string()),
                row.enabled as i32,
                row.created_at.to_rfc3339(),
                row.updated_at.to_rfc3339(),
            ],
        )
        .await
        .context("failed to insert task")?;
        Ok(())
    }

    pub async fn get_task_by_name(&self, name: &str) -> Result<Option<TaskRow>> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query("SELECT * FROM tasks WHERE name = ?1", params![name])
            .await
            .context("failed to query task")?;

        match rows.next().await? {
            Some(row) => Ok(Some(task_row_from_libsql(&row)?)),
            None => Ok(None),
        }
    }

    pub async fn list_tasks(&self) -> Result<Vec<TaskRow>> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query("SELECT * FROM tasks ORDER BY name", params![])
            .await
            .context("failed to list tasks")?;

        let mut tasks = Vec::new();
        while let Some(row) = rows.next().await? {
            tasks.push(task_row_from_libsql(&row)?);
        }
        Ok(tasks)
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

    pub async fn get_runs_for_task(&self, task_id: Uuid, limit: u32) -> Result<Vec<Run>> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT * FROM runs WHERE task_id = ?1 ORDER BY started_at DESC LIMIT ?2",
                params![task_id.to_string(), limit],
            )
            .await
            .context("failed to query runs")?;

        let mut runs = Vec::new();
        while let Some(row) = rows.next().await? {
            runs.push(run_from_libsql(&row)?);
        }
        Ok(runs)
    }
}

fn task_row_from_libsql(row: &libsql::Row) -> Result<TaskRow> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let executor_type: String = row.get(2)?;
    let config_json: String = row.get(3)?;
    let trigger_json: Option<String> = row.get(4)?;
    let working_directory: Option<String> = row.get(5)?;
    let enabled: i32 = row.get(6)?;
    let created_at: String = row.get(7)?;
    let updated_at: String = row.get(8)?;

    Ok(TaskRow {
        id: id.parse().context("invalid task id")?,
        name,
        executor_type,
        config_json,
        trigger_json,
        working_directory: working_directory.map(PathBuf::from),
        enabled: enabled != 0,
        created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
    })
}

fn run_from_libsql(row: &libsql::Row) -> Result<Run> {
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
