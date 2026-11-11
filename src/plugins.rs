//! Plugins (NSH-66 to NSH-68): `plugins=(…)` in `~/.neoshrc` names what to
//! load — a plugin built in, a file or folder, or a git repo cloned once.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use brush_core::builtins::{BoxFuture, ContentOptions, ContentType, Registration};
use brush_core::commands::{CommandArg, ExecutionContext};
use brush_core::ExecutionResult;
use crate::errors::Extensions as SE;

/// The plugins built into neosh.
pub const BUILT_IN: &[&str] = &["z"];

/// What each loaded plugin was, and where it came from, for `plugin list`.
static LOADED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

#[derive(Debug, PartialEq)]
pub enum Source {
    BuiltIn(String),
    Path(PathBuf),
    /// A repo: where to clone it from, and its `owner/repo` name.
    Repo { url: String, name: String },
}

/// NSH-66: what an entry names; `None` when it is none of these.
pub fn source_of(entry: &str, home: Option<&Path>) -> Option<Source> {
    if BUILT_IN.contains(&entry) {
        return Some(Source::BuiltIn(entry.to_string()));
    }
    if let Some(rest) = entry.strip_prefix("~/") {
        return home.map(|h| Source::Path(h.join(rest)));
    }
    if entry.starts_with('/') || entry.starts_with("./") || entry.starts_with("../") {
        return Some(Source::Path(PathBuf::from(entry)));
    }
    let is_url = ["https://", "http://", "ssh://", "file://", "git@"].iter().any(|p| entry.starts_with(p));
    if is_url {
        let path = entry.trim_end_matches('/').trim_end_matches(".git");
        let parts: Vec<&str> = path.rsplit(['/', ':']).take(2).collect();
        if parts.len() == 2 && parts.iter().all(|p| !p.is_empty()) {
            return Some(Source::Repo { url: entry.to_string(), name: format!("{}/{}", parts[1], parts[0]) });
        }
        return None;
    }
    let mut parts = entry.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    let word = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) && w != "." && w != "..";
    (parts.next().is_none() && word(owner) && word(repo))
        .then(|| Source::Repo { url: format!("https://github.com/{owner}/{repo}.git"), name: format!("{owner}/{repo}") })
}

/// NSH-66: the file a plugin folder is loaded by.
pub fn file_in(dir: &Path) -> Option<PathBuf> {
    let base = dir.file_name()?.to_string_lossy().into_owned();
    let stem = base.strip_suffix(".git").unwrap_or(&base).to_string();
    [format!("{stem}.plugin.sh"), format!("{stem}.plugin.bash"), "plugin.sh".into(), "init.sh".into(), format!("{stem}.sh")]
        .into_iter()
        .map(|f| dir.join(f))
        .find(|f| f.is_file())
}

/// Where repos are cloned (NSH-67).
fn plugins_dir(home: &Path) -> PathBuf {
    home.join(".neosh").join("plugins")
}

/// The entries `plugins` names: an array, or words.
fn entries(shell: &brush_core::Shell<SE>) -> Vec<String> {
    let mut names = Vec::new();
    match shell.env_var("plugins").map(|v| v.value().clone()) {
        Some(brush_core::ShellValue::IndexedArray(a)) => names.extend(a.values().cloned()),
        Some(brush_core::ShellValue::String(s)) => names.extend(s.split_whitespace().map(String::from)),
        _ => {}
    }
    let mut seen = std::collections::HashSet::new();
    names.retain(|n| !n.is_empty() && seen.insert(n.clone()));
    names
}

fn say(line: &str) {
    eprintln!("neosh: {line}");
}

/// NSH-66 to NSH-68: loads what `plugins` names, once the rc has run; what
/// cannot load is said in a line and the rest still load.
pub async fn load(shell: &mut brush_core::Shell<SE>) {
    shell.register_builtin(
        "plugin",
        Registration { execute_func: execute, content_func: content, disabled: false, special_builtin: false, declaration_builtin: false },
    );
    let home = std::env::var_os("HOME").map(PathBuf::from);
    for entry in entries(shell) {
        let file = match source_of(&entry, home.as_deref()) {
            None => {
                say(&format!("plugin {entry}: no such plugin (built in: {}; or a path, or owner/repo)", BUILT_IN.join(", ")));
                continue;
            }
            Some(Source::BuiltIn(name)) => {
                if name == "z" {
                    crate::z::install(shell);
                }
                LOADED.lock().unwrap_or_else(|e| e.into_inner()).push((entry.clone(), "built in".into()));
                continue;
            }
            Some(Source::Path(path)) if path.is_dir() => match file_in(&path) {
                Some(f) => f,
                None => {
                    say(&format!("plugin {entry}: no plugin file in {} (looked for <name>.plugin.sh, plugin.sh, init.sh)", path.display()));
                    continue;
                }
            },
            Some(Source::Path(path)) if path.is_file() => path,
            Some(Source::Path(path)) => {
                say(&format!("plugin {entry}: {} is not there", path.display()));
                continue;
            }
            Some(Source::Repo { url, name }) => {
                let Some(home) = home.as_deref() else {
                    say(&format!("plugin {entry}: HOME is not set, so it has nowhere to go"));
                    continue;
                };
                let dir = plugins_dir(home).join(&name);
                // NSH-67: cloned the first time, never fetched again at start.
                if !dir.is_dir() {
                    say(&format!("installing plugin {name} from {url}…"));
                    let _ = std::fs::create_dir_all(dir.parent().unwrap_or(home));
                    let ok = std::process::Command::new("git")
                        .args(["clone", "--quiet", "--depth", "1", &url])
                        .arg(&dir)
                        .stdin(std::process::Stdio::null())
                        .status()
                        .is_ok_and(|s| s.success());
                    if !ok {
                        let _ = std::fs::remove_dir_all(&dir);
                        say(&format!("plugin {entry}: could not clone {url}; starting without it"));
                        continue;
                    }
                }
                match file_in(&dir) {
                    Some(f) => f,
                    None => {
                        say(&format!("plugin {entry}: no plugin file in {} (looked for <name>.plugin.sh, plugin.sh, init.sh)", dir.display()));
                        continue;
                    }
                }
            }
        };
        let params = shell.default_exec_params();
        match shell.source_script(&file, std::iter::empty::<String>(), &params).await {
            Ok(_) => LOADED.lock().unwrap_or_else(|e| e.into_inner()).push((entry.clone(), file.display().to_string())),
            Err(e) => say(&format!("plugin {entry}: {e}")),
        }
    }
}

fn content(_name: &str, kind: ContentType, _options: &ContentOptions) -> Result<String, brush_core::Error> {
    Ok(match kind {
        ContentType::ShortUsage => "plugin [list | update]\n".to_string(),
        ContentType::ShortDescription => "plugin - list the plugins loaded, or update the cloned ones\n".to_string(),
        _ => USAGE.to_string(),
    })
}

const USAGE: &str = "plugin list     the plugins loaded, and where from\n\
plugin update   pull every plugin cloned from git (they count from the next neosh)\n\
\n\
Name plugins in ~/.neoshrc:  plugins=(z ~/my/plugin owner/repo)\n";

fn execute(context: ExecutionContext<'_, SE>, args: Vec<CommandArg>) -> BoxFuture<'_, Result<ExecutionResult, brush_core::Error>> {
    Box::pin(async move {
        let _ = &context;
        let sub = args.get(1).and_then(|a| if let CommandArg::String(s) = a { Some(s.as_str()) } else { None });
        match sub {
            Some("list") => {
                let loaded = LOADED.lock().unwrap_or_else(|e| e.into_inner());
                if loaded.is_empty() {
                    println!("No plugins loaded. Name them in ~/.neoshrc: plugins=(z)");
                }
                let wide = loaded.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0);
                for (name, from) in loaded.iter() {
                    println!("{name:<wide$}  {from}");
                }
                Ok(ExecutionResult::success())
            }
            Some("update") => Ok(if update() { ExecutionResult::success() } else { ExecutionResult::general_error() }),
            _ => {
                print!("{USAGE}");
                Ok(if sub.is_none() { ExecutionResult::success() } else { ExecutionResult::general_error() })
            }
        }
    })
}

/// NSH-67: pulls every cloned plugin; whether all of them updated.
fn update() -> bool {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return false };
    let mut repos: Vec<PathBuf> = std::fs::read_dir(plugins_dir(&home))
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|owner| std::fs::read_dir(owner.path()).into_iter().flatten().flatten().map(|r| r.path()))
        .filter(|p| p.join(".git").exists())
        .collect();
    repos.sort();
    if repos.is_empty() {
        println!("No plugins cloned from git.");
        return true;
    }
    let mut all = true;
    for repo in repos {
        let name = repo.strip_prefix(plugins_dir(&home)).unwrap_or(&repo).display().to_string();
        let ok = std::process::Command::new("git")
            .args(["-C"])
            .arg(&repo)
            .args(["pull", "--quiet", "--ff-only"])
            .stdin(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        println!("{name}: {}", if ok { "up to date" } else { "could not update" });
        all &= ok;
    }
    if all {
        println!("Updates count from the next neosh.");
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    // NSH-66
    #[test]
    fn an_entry_is_a_built_in_a_path_or_a_repo() {
        let home = Path::new("/h");
        assert_eq!(source_of("z", Some(home)), Some(Source::BuiltIn("z".into())));
        assert_eq!(source_of("~/x/p", Some(home)), Some(Source::Path("/h/x/p".into())));
        assert_eq!(source_of("/opt/p.sh", Some(home)), Some(Source::Path("/opt/p.sh".into())));
        assert_eq!(source_of("./p", Some(home)), Some(Source::Path("./p".into())));
        assert_eq!(
            source_of("zsh-users/zsh-z", Some(home)),
            Some(Source::Repo { url: "https://github.com/zsh-users/zsh-z.git".into(), name: "zsh-users/zsh-z".into() })
        );
        assert_eq!(
            source_of("https://gitlab.com/me/thing.git", Some(home)),
            Some(Source::Repo { url: "https://gitlab.com/me/thing.git".into(), name: "me/thing".into() })
        );
        assert_eq!(
            source_of("git@github.com:me/thing.git", Some(home)),
            Some(Source::Repo { url: "git@github.com:me/thing.git".into(), name: "me/thing".into() })
        );
        for nothing in ["nosuch", "a/b/c", "a/..", "a b/c"] {
            assert_eq!(source_of(nothing, Some(home)), None, "{nothing}");
        }
    }

    // NSH-66
    #[test]
    fn a_folder_is_loaded_by_its_plugin_file() {
        let base = std::env::temp_dir().join(format!("neosh-plugin-test-{}", std::process::id()));
        let dir = base.join("thing");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(file_in(&dir), None);
        std::fs::write(dir.join("init.sh"), "").unwrap();
        assert_eq!(file_in(&dir), Some(dir.join("init.sh")));
        std::fs::write(dir.join("thing.plugin.sh"), "").unwrap();
        assert_eq!(file_in(&dir), Some(dir.join("thing.plugin.sh")));
        let _ = std::fs::remove_dir_all(&base);
    }
}
