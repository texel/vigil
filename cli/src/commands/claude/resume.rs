use anyhow::{Result, bail};
use vigil_registry::Store;

pub async fn handle(run_id: &str, store: &Store) -> Result<()> {
    let run = store
        .get_run_by_id_prefix(run_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no run found matching '{run_id}'"))?;

    let task = store
        .get_task_by_id(run.task_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task for run '{run_id}' not found"))?;

    if task.task.runner_type != "claude" {
        bail!(
            "resume is only supported for claude tasks (this is a '{}' task)",
            task.task.runner_type
        );
    }

    // Use the run ID as session ID (they're the same by design)
    let session_id = run
        .metadata
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| run.id.to_string());

    let working_directory = task
        .working_directory
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    println!("Resuming claude session {session_id}...");

    // Replace the current process with claude --resume
    let mut cmd = std::process::Command::new("claude");
    cmd.arg("--resume").arg(&session_id);
    cmd.current_dir(&working_directory);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        bail!("failed to exec claude: {err}");
    }

    #[cfg(not(unix))]
    {
        let status = cmd.status().context("failed to run claude")?;
        std::process::exit(status.code().unwrap_or(1));
    }
}
