//! `fg` around brush's own: a job brought back that ends in the foreground
//! is not reported as Done at the next prompt, as bash does not (NSH-63).

use std::sync::OnceLock;

use brush_core::builtins::{BoxFuture, CommandExecuteFunc};
use brush_core::commands::{CommandArg, ExecutionContext};
use brush_core::jobs::JobState;
use brush_core::ExecutionResult;
use crate::errors::Extensions as SE;

static BRUSH_FG: OnceLock<CommandExecuteFunc<SE>> = OnceLock::new();

/// Puts neosh's `fg` in place of brush's, which it then calls.
pub fn install(shell: &mut brush_core::Shell<SE>) {
    if let Some(fg) = shell.builtin_mut("fg") {
        let _ = BRUSH_FG.set(fg.execute_func);
        fg.execute_func = execute;
    }
}

/// The job spec `fg` was given, as brush reads it: the first word after
/// `fg`, or the current job.
fn spec(args: &[CommandArg]) -> String {
    args.iter()
        .skip(1)
        .find_map(|a| match a {
            CommandArg::String(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "%+".to_string())
}

/// NSH-63: the job is forgotten once it has ended in the foreground, so the
/// next prompt does not say `[1]+ Done`; one stopped again (^Z) stays.
fn forget_if_done(jobs: &mut brush_core::jobs::JobManager, id: usize) {
    jobs.jobs.retain(|j| j.id != id || !matches!(j.state, JobState::Done));
}

fn execute(context: ExecutionContext<'_, SE>, args: Vec<CommandArg>) -> BoxFuture<'_, Result<ExecutionResult, brush_core::Error>> {
    Box::pin(async move {
        let Some(brush_fg) = BRUSH_FG.get().copied() else {
            return Ok(ExecutionResult::general_error());
        };
        let ExecutionContext { shell, command_name, params } = context;
        let id = shell.jobs_mut().resolve_job_spec(&spec(&args)).map(|j| j.id);
        let result = brush_fg(ExecutionContext { shell: &mut *shell, command_name, params }, args).await;
        if let Some(id) = id {
            forget_if_done(shell.jobs_mut(), id);
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // NSH-63
    #[test]
    fn fg_reads_its_job_spec_as_brush_does() {
        let word = |s: &str| CommandArg::String(s.to_string());
        assert_eq!(spec(&[word("fg")]), "%+");
        assert_eq!(spec(&[word("fg"), word("%2")]), "%2");
    }
}
