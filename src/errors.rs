//! NSH-3: errors said as bash says them: `s.sh: line 2: x: command not found`,
//! not brush's `error: command not found: x`.

use brush_core::extensions::{ErrorFormatter, ShellExtensionsImpl};

pub type Extensions = ShellExtensionsImpl<BashErrors>;

#[derive(Clone, Default)]
pub struct BashErrors;

impl ErrorFormatter for BashErrors {
    fn format_error(&self, error: &brush_core::Error, shell: &brush_core::Shell<impl brush_core::ShellExtensions>) -> String {
        let stack = shell.call_stack();
        let line = stack.current_frame().and_then(|f| f.current_line());
        let script = stack.iter().find(|f| f.frame_type.is_script()).map(|f| f.frame_type.name().into_owned());
        let interactive = shell.options().interactive;
        let message = error.to_string();
        let message = message.strip_prefix("command not found: ").map_or(message.clone(), |c| format!("{c}: command not found"));
        say(script.as_deref(), line, interactive, &message)
    }
}

/// Where the error was, then what it was. A script names itself and the
/// line; a line typed at the prompt names neosh; `-c` and stdin give the line.
pub fn say(script: Option<&str>, line: Option<usize>, interactive: bool, message: &str) -> String {
    match (script, line) {
        (Some(script), Some(line)) => format!("{script}: line {line}: {message}\n"),
        (Some(script), None) => format!("{script}: {message}\n"),
        (None, Some(line)) if !interactive => format!("neosh: line {line}: {message}\n"),
        _ => format!("neosh: {message}\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NSH-3
    #[test]
    fn errors_are_said_as_bash_says_them() {
        assert_eq!(say(Some("s.sh"), Some(2), false, "x: command not found"), "s.sh: line 2: x: command not found\n");
        assert_eq!(say(None, Some(1), false, "x: command not found"), "neosh: line 1: x: command not found\n");
        assert_eq!(say(None, Some(1), true, "x: command not found"), "neosh: x: command not found\n");
    }
}
