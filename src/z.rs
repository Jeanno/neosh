//! The `z` plugin, built in (NSH-69, NSH-70): the directories the shell is
//! in are recorded as rupa/z records them, and `z <words>` goes to the one
//! that matches with the highest frecency.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use brush_core::builtins::{BoxFuture, ContentOptions, ContentType, Registration};
use brush_core::commands::{CommandArg, ExecutionContext};
use brush_core::ExecutionResult;
use crate::errors::Extensions as SE;

/// Whether the plugin is loaded, so the prompt records directories.
static ON: AtomicBool = AtomicBool::new(false);
/// NSH-69: ranks past this all shrink.
const MAX_TOTAL: f64 = 9000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: String,
    pub rank: f64,
    pub time: u64,
}

/// Puts `z` in the shell and starts recording.
pub fn install(shell: &mut brush_core::Shell<SE>) {
    shell.register_builtin(
        "z",
        Registration { execute_func: execute, content_func: content, disabled: false, special_builtin: false, declaration_builtin: false },
    );
    ON.store(true, Ordering::Relaxed);
}

fn content(_name: &str, kind: ContentType, _options: &ContentOptions) -> Result<String, brush_core::Error> {
    Ok(match kind {
        ContentType::ShortUsage => "z [-l | -x] [words ...]\n".to_string(),
        ContentType::ShortDescription => "z - go to the directory you use most that matches\n".to_string(),
        _ => "z: z [-l | -x] [words ...]\n    Go to the recorded directory that matches the words in order, the one\n    you use most and most lately. -l lists the matches, -x forgets this\n    directory.\n".to_string(),
    })
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// NSH-69: where the record is kept, from `_Z_DATA` or home.
fn data_file<X: brush_core::ShellExtensions>(shell: &brush_core::Shell<X>) -> Option<PathBuf> {
    if let Some(f) = shell.env_str("_Z_DATA").filter(|f| !f.is_empty()) {
        return Some(PathBuf::from(f.as_ref()));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".neosh_z"))
}

pub fn parse(text: &str) -> Vec<Entry> {
    text.lines()
        .filter_map(|l| {
            let mut parts = l.rsplitn(3, '|');
            let time = parts.next()?.trim().parse().ok()?;
            let rank = parts.next()?.trim().parse().ok()?;
            let path = parts.next()?.to_string();
            (!path.is_empty()).then_some(Entry { path, rank, time })
        })
        .collect()
}

fn format(entries: &[Entry]) -> String {
    entries.iter().map(|e| format!("{}|{}|{}\n", e.path, e.rank, e.time)).collect()
}

/// The record; a missing one starts from rupa/z's `~/.z` (NSH-69).
fn load(file: &Path) -> Vec<Entry> {
    if let Ok(text) = std::fs::read_to_string(file) {
        return parse(&text);
    }
    std::env::var_os("HOME")
        .and_then(|h| std::fs::read_to_string(PathBuf::from(h).join(".z")).ok())
        .map(|t| parse(&t))
        .unwrap_or_default()
}

/// Written whole and then moved into place, so two shells never leave half a file.
fn save(file: &Path, entries: &[Entry]) {
    let tmp = file.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, format(entries)).is_ok() && std::fs::rename(&tmp, file).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// NSH-69: one more visit to `dir`, the ranks shrunk when they add up past 9000.
pub fn add(entries: &mut Vec<Entry>, dir: &str, at: u64) {
    match entries.iter_mut().find(|e| e.path == dir) {
        Some(e) => {
            e.rank += 1.0;
            e.time = at;
        }
        None => entries.push(Entry { path: dir.to_string(), rank: 1.0, time: at }),
    }
    if entries.iter().map(|e| e.rank).sum::<f64>() > MAX_TOTAL {
        for e in entries.iter_mut() {
            e.rank *= 0.99;
        }
        entries.retain(|e| e.rank >= 1.0);
    }
}

/// The last directory recorded by this shell, so a prompt in the same place
/// again does not count as another visit.
static LAST: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// NSH-69: called at each prompt; records the directory when it changed.
pub fn at_prompt<X: brush_core::ShellExtensions>(shell: &brush_core::Shell<X>) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let dir = shell.working_dir().to_path_buf();
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.as_deref() == Some(dir.as_path()) {
        return;
    }
    *last = Some(dir.clone());
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if dir == Path::new("/") || home.as_deref() == Some(dir.as_path()) {
        return;
    }
    let Some(file) = data_file(shell) else { return };
    let mut entries = load(&file);
    add(&mut entries, &dir.to_string_lossy(), now());
    save(&file, &entries);
}

/// NSH-70: rank weighed by how lately: ×4 within the hour, ×2 the day, ×½ the week, ×¼ before.
pub fn frecency(e: &Entry, at: u64) -> f64 {
    let ago = at.saturating_sub(e.time);
    e.rank
        * if ago < 3600 {
            4.0
        } else if ago < 86_400 {
            2.0
        } else if ago < 604_800 {
            0.5
        } else {
            0.25
        }
}

/// Whether the words are found in the path, in order.
fn in_order(path: &str, words: &[String], fold: bool) -> bool {
    let (path, words): (String, Vec<String>) =
        if fold { (path.to_lowercase(), words.iter().map(|w| w.to_lowercase()).collect()) } else { (path.to_string(), words.to_vec()) };
    let mut rest = path.as_str();
    for w in &words {
        match rest.find(w.as_str()) {
            Some(i) => rest = &rest[i + w.len()..],
            None => return false,
        }
    }
    true
}

/// NSH-70: the entries matching the words, worst first; case-sensitive, else not.
pub fn matches<'a>(entries: &'a [Entry], words: &[String], at: u64, exists: impl Fn(&str) -> bool) -> Vec<(&'a Entry, f64)> {
    for fold in [false, true] {
        let mut hits: Vec<(&Entry, f64)> =
            entries.iter().filter(|e| in_order(&e.path, words, fold) && exists(&e.path)).map(|e| (e, frecency(e, at))).collect();
        if !hits.is_empty() {
            hits.sort_by(|a, b| a.1.total_cmp(&b.1));
            return hits;
        }
    }
    Vec::new()
}

fn execute(context: ExecutionContext<'_, SE>, args: Vec<CommandArg>) -> BoxFuture<'_, Result<ExecutionResult, brush_core::Error>> {
    Box::pin(async move {
        let words: Vec<String> = args.iter().skip(1).filter_map(|a| if let CommandArg::String(s) = a { Some(s.clone()) } else { None }).collect();
        let Some(file) = data_file(context.shell) else {
            eprintln!("z: HOME is not set");
            return Ok(ExecutionResult::general_error());
        };
        let mut entries = load(&file);
        let is_dir = |p: &str| Path::new(p).is_dir();
        // Directories gone are dropped (NSH-70).
        let before = entries.len();
        entries.retain(|e| is_dir(&e.path));
        if entries.len() != before {
            save(&file, &entries);
        }
        match words.first().map(String::as_str) {
            Some("-x") => {
                let here = context.shell.working_dir().to_string_lossy().into_owned();
                entries.retain(|e| e.path != here);
                save(&file, &entries);
                return Ok(ExecutionResult::success());
            }
            None | Some("-l") => {
                let rest = if words.is_empty() { &words[..] } else { &words[1..] };
                for (e, score) in matches(&entries, rest, now(), is_dir) {
                    println!("{score:<10.1} {}", e.path);
                }
                return Ok(ExecutionResult::success());
            }
            Some("-h") | Some("--help") => {
                print!("{}", content("z", ContentType::DetailedHelp, &ContentOptions::default())?);
                return Ok(ExecutionResult::success());
            }
            _ => {}
        }
        // A word that is a directory goes there, as `cd` would.
        let target = if words.len() == 1 && context.shell.absolute_path(Path::new(&words[0])).is_dir() {
            context.shell.absolute_path(Path::new(&words[0])).to_string_lossy().into_owned()
        } else {
            match matches(&entries, &words, now(), is_dir).last() {
                Some((e, _)) => e.path.clone(),
                None => {
                    eprintln!("z: no directory matches {}", words.join(" "));
                    return Ok(ExecutionResult::general_error());
                }
            }
        };
        let Some(cd) = crate::cd::brush_cd() else {
            return Ok(ExecutionResult::general_error());
        };
        let ExecutionContext { shell, params, .. } = context;
        cd(ExecutionContext { shell, command_name: "cd".to_string(), params }, vec![CommandArg::String("cd".into()), CommandArg::String(target)]).await
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(path: &str, rank: f64, time: u64) -> Entry {
        Entry { path: path.into(), rank, time }
    }

    // NSH-69
    #[test]
    fn the_record_reads_as_rupa_z_writes_it() {
        let got = parse("/a/b|12|1700000000\n/with|bar|3.5|1700000100\nbad line\n");
        assert_eq!(got, vec![e("/a/b", 12.0, 1700000000), e("/with|bar", 3.5, 1700000100)]);
        assert_eq!(parse(&format(&got)), got);
    }

    // NSH-69
    #[test]
    fn a_visit_ranks_up_and_ranks_shrink_past_9000() {
        let mut v = vec![e("/a", 1.0, 10)];
        add(&mut v, "/a", 20);
        add(&mut v, "/b", 30);
        assert_eq!(v, vec![e("/a", 2.0, 20), e("/b", 1.0, 30)]);
        let mut big = vec![e("/big", 9000.0, 0), e("/small", 1.0, 0)];
        add(&mut big, "/new", 5);
        assert_eq!(big.len(), 1, "under 1 after shrinking goes: {big:?}");
        assert!((big[0].rank - 8910.0).abs() < 0.01);
    }

    // NSH-70
    #[test]
    fn frecency_weighs_how_lately() {
        let at = 1_000_000;
        assert_eq!(frecency(&e("/", 10.0, at - 60), at), 40.0);
        assert_eq!(frecency(&e("/", 10.0, at - 7200), at), 20.0);
        assert_eq!(frecency(&e("/", 10.0, at - 200_000), at), 5.0);
        assert_eq!(frecency(&e("/", 10.0, 0), at), 2.5);
    }

    // NSH-70
    #[test]
    fn the_best_match_of_the_words_in_order_wins() {
        let at = 1_000_000;
        let v = vec![
            e("/Users/me/Projects/neosh", 5.0, at - 60),
            e("/Users/me/Projects/neoai/neonavi", 50.0, at - 900_000),
            e("/Users/me/shared/neo", 2.0, at - 60),
            e("/gone/neosh", 99.0, at),
        ];
        let there = |p: &str| !p.starts_with("/gone");
        let w = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(matches(&v, &w("neo"), at, there).last().unwrap().0.path, "/Users/me/Projects/neosh");
        // No case-sensitive match, so case is folded.
        assert_eq!(matches(&v, &w("proj navi"), at, there).last().unwrap().0.path, "/Users/me/Projects/neoai/neonavi");
        // A case-sensitive match wins over folded ones.
        let cased = vec![e("/x/Neo", 1.0, at), e("/x/neo", 50.0, at)];
        assert_eq!(matches(&cased, &w("Neo"), at, |_| true).len(), 1);
        assert!(matches(&v, &w("navi projects"), at, there).is_empty(), "words in order");
        assert!(matches(&v, &w("nothing"), at, there).is_empty());
    }
}
