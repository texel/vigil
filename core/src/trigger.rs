// Trigger parsing has moved to vigil-schedule.
// This module is kept for backward compatibility of the `parse_trigger` function.

use anyhow::Result;
pub use vigil_schedule::TriggerSpec;

/// Parse a trigger expression string into a [`TriggerSpec`].
///
/// Convenience wrapper — delegates to `vigil_schedule::TriggerSpec::from_str`.
pub fn parse_trigger(input: &str) -> Result<TriggerSpec> {
    input.parse()
}
