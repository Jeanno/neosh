//! Aliases brush cannot run as they are (NSH-71). brush splits an alias's
//! text on spaces and never parses it, so `alias z='_z 2>&1'` hands `_z` the
//! word `2>&1`. Such an alias becomes the function bash would have run.

/// Whether brush runs this alias text as bash would: plain words only.
pub fn plain(value: &str) -> bool {
    value.chars().all(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || "-_./=,:+@%^~".contains(c))
}

/// Whether a function can have this name.
fn nameable(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || "_-.:+@".contains(c)) && !name.starts_with('-')
}

/// NSH-71: the function an alias is run as: its text, then the words after
/// it. An alias whose first word is its own name calls the command, not itself.
pub fn function_for(name: &str, value: &str) -> Option<String> {
    if plain(value) || !nameable(name) {
        return None;
    }
    let body = value.trim();
    let first = body.split(|c: char| c.is_ascii_whitespace() || "|&;<>()".contains(c)).next().unwrap_or("");
    let body = if first == name { format!("command {body}") } else { body.to_string() };
    // Text that takes the words itself, or ends a list, is left to do so.
    let takes = ["\"$@\"", "$@", "$*", "$1"].iter().any(|w| body.contains(w));
    let ends = body.ends_with(';') || body.ends_with('&') || body.ends_with('|');
    let args = if takes || ends { "" } else { " \"$@\"" };
    Some(format!("{name}() {{ {body}{args}\n}}"))
}

/// NSH-71: turns each alias brush would run wrong into its function; run at each prompt.
pub fn settle<X: brush_core::ShellExtensions>(shell: &mut brush_core::Shell<X>) {
    let turned: Vec<(String, String)> =
        shell.aliases().iter().filter_map(|(n, v)| function_for(n, v).map(|f| (n.clone(), f))).collect();
    for (name, function) in turned {
        shell.aliases_mut().remove(&name);
        let params = shell.default_exec_params();
        let status = shell.last_exit_status();
        let _ = tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(shell.run_string(function, &brush_core::SourceInfo::from("alias"), &params)));
        shell.set_last_exit_status(status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NSH-71
    #[test]
    fn an_alias_brush_would_split_wrong_becomes_a_function() {
        assert_eq!(function_for("ll", "ls -la"), None, "plain words run as they are");
        assert_eq!(function_for("z", "_z 2>&1").unwrap(), "z() { _z 2>&1 \"$@\"\n}");
        assert_eq!(function_for("l", "ls -la | less").unwrap(), "l() { ls -la | less \"$@\"\n}");
        assert_eq!(function_for("g", "git commit -m \"wip\"").unwrap(), "g() { git commit -m \"wip\" \"$@\"\n}");
        assert_eq!(function_for("grep", "grep --color=auto 'x'").unwrap(), "grep() { command grep --color=auto 'x' \"$@\"\n}");
        assert_eq!(function_for("up", "cd ..; ls").unwrap(), "up() { cd ..; ls \"$@\"\n}");
        assert_eq!(function_for("a", "echo \"$@\" done").unwrap(), "a() { echo \"$@\" done\n}");
        assert_eq!(function_for("bg1", "sleep 1 &").unwrap(), "bg1() { sleep 1 &\n}");
        assert_eq!(function_for("a b", "x 2>&1"), None);
    }
}
