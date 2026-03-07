mod store;

pub use store::Store;

use anyhow::{Result, bail};
use vigil_core::models::{RunContext, RunOutput};
use vigil_core::runner::{Runner, Task};
use vigil_runner_shell::{ShellRunner, ShellTask};

/// Deserialize a task config from JSON given the runner type discriminant,
/// and pair it with the appropriate runner.
pub fn deserialize_task(runner_type: &str, json: &str) -> Result<Box<dyn Runnable>> {
    match runner_type {
        "shell" => {
            let task: ShellTask = serde_json::from_str(json)?;
            Ok(Box::new(RunnableTask {
                task,
                runner: ShellRunner,
            }))
        }
        _ => bail!("unknown runner type: {runner_type}"),
    }
}

/// Dyn-safe wrapper for runnable tasks.
/// This is the dyn boundary used by composition roots (CLI, daemon, etc.) for dispatch,
/// and allows heterogeneous tasks to be handled in the same collection.
#[async_trait::async_trait]
pub trait Runnable: Send + Sync {
    fn runner_type(&self) -> &'static str;
    async fn run(&self, context: &RunContext) -> Result<RunOutput>;
}

/// Composes a Task with its Runner.
struct RunnableTask<T: Task, R: Runner<Task = T>> {
    task: T,
    runner: R,
}

#[async_trait::async_trait]
impl<T: Task, R: Runner<Task = T>> Runnable for RunnableTask<T, R> {
    fn runner_type(&self) -> &'static str {
        self.task.runner_type()
    }

    async fn run(&self, context: &RunContext) -> Result<RunOutput> {
        self.runner.run(&self.task, context).await
    }
}
