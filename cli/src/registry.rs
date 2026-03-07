use anyhow::{bail, Result};
use vigil_core::executor::ExecutorConfig;
use vigil_core::models::{RunContext, RunOutput};
use vigil_executor_shell::{ShellConfig, ShellExecutor};
use vigil_core::executor::Executor;

/// Deserialize an executor config from JSON given the executor type discriminant.
pub fn deserialize_config(
    executor_type: &str,
    json: &str,
) -> Result<Box<dyn ExecutorConfigDyn>> {
    match executor_type {
        "shell" => {
            let config: ShellConfig = serde_json::from_str(json)?;
            Ok(Box::new(DynWrapper {
                config,
                executor: ShellExecutor,
            }))
        }
        _ => bail!("unknown executor type: {executor_type}"),
    }
}

/// Object-safe wrapper that pairs a config with its executor.
/// This is the dyn boundary — used at the CLI layer for dispatch.
#[async_trait::async_trait]
pub trait ExecutorConfigDyn: Send + Sync {
    fn executor_type(&self) -> &'static str;
    async fn execute(&self, context: &RunContext) -> Result<RunOutput>;
}

struct DynWrapper<C: ExecutorConfig, E: Executor<Config = C>> {
    config: C,
    executor: E,
}

#[async_trait::async_trait]
impl<C: ExecutorConfig, E: Executor<Config = C>> ExecutorConfigDyn for DynWrapper<C, E> {
    fn executor_type(&self) -> &'static str {
        self.config.executor_type()
    }

    async fn execute(&self, context: &RunContext) -> Result<RunOutput> {
        self.executor.execute(&self.config, context).await
    }
}
