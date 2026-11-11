//! neosh NSH-72: bash's `\#` (this command's number) and `\!` (its history
//! number) in a prompt, which brush does not expand; and a prompt brush
//! cannot expand is drawn plain, said once, rather than ending the shell.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The number of the command the next line will be, from 1, as bash's `\#`.
static COMMAND: AtomicUsize = AtomicUsize::new(1);
static SAID: AtomicBool = AtomicBool::new(false);

/// The prompt variables brush expands.
const PROMPTS: [&str; 4] = ["PS0", "PS1", "PS2", "BRUSH_PS_ALT"];

/// A line was run: the next one is the next command.
pub fn ran_command() {
    COMMAND.fetch_add(1, Ordering::Relaxed);
}

/// `spec` with `\#` and `\!` put as numbers; `None` when it has neither.
pub fn numbered(spec: &str, command: usize, history: usize) -> Option<String> {
    let mut out = String::with_capacity(spec.len());
    let mut changed = false;
    let mut chars = spec.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('#') => {
                out.push_str(&command.to_string());
                changed = true;
            }
            Some('!') => {
                out.push_str(&history.to_string());
                changed = true;
            }
            Some(n) => {
                out.push('\\');
                out.push(n);
            }
            None => out.push('\\'),
        }
    }
    changed.then_some(out)
}

/// Puts the numbers into the prompt variables, for `restore` to put back.
pub fn put<SE: brush_core::ShellExtensions>(shell: &mut brush_core::Shell<SE>) -> Vec<(&'static str, String)> {
    let command = COMMAND.load(Ordering::Relaxed);
    let history = shell.history().map_or(0, |h| h.count()) + 1;
    let mut saved = Vec::new();
    for name in PROMPTS {
        let Some(spec) = shell.env_str(name).map(|s| s.into_owned()) else { continue };
        let Some(with) = numbered(&spec, command, history) else { continue };
        if let Some((_, var)) = shell.env_mut().get_mut(name) {
            if var.assign(brush_core::variables::ShellValueLiteral::Scalar(with), false).is_ok() {
                saved.push((name, spec));
            }
        }
    }
    saved
}

/// Puts back what `put` changed.
pub fn restore<SE: brush_core::ShellExtensions>(shell: &mut brush_core::Shell<SE>, saved: Vec<(&'static str, String)>) {
    for (name, spec) in saved {
        if let Some((_, var)) = shell.env_mut().get_mut(name) {
            let _ = var.assign(brush_core::variables::ShellValueLiteral::Scalar(spec), false);
        }
    }
}

/// A prompt that could not be expanded: said the first time, then `plain`.
pub fn or_plain(result: Result<String, brush_core::Error>, name: &str, plain: &str) -> String {
    result.unwrap_or_else(|e| {
        if !SAID.swap(true, Ordering::Relaxed) {
            eprintln!("neosh: {name} cannot be shown ({e}); using a plain prompt");
        }
        plain.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_and_history_numbers_are_put_in() {
        assert_eq!(numbered(r"\w \$ ", 3, 9), None);
        assert_eq!(numbered(r"[\#] \u $ ", 3, 9).unwrap(), r"[3] \u $ ");
        assert_eq!(numbered(r"\! \#", 3, 9).unwrap(), "9 3");
        // An escaped backslash before `#` is a backslash, then `#`.
        assert_eq!(numbered(r"\\# \#", 3, 9).unwrap(), r"\\# 3");
    }
}
