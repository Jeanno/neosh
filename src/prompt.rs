//! neosh's own prompt (NSH-10, NSH-12): the directory, the git branch and a
//! failed status.

use std::path::{Path, PathBuf};

/// The PS1 neosh sets; while it is still the shell's, neosh draws the prompt itself.
pub const DEFAULT_PS1: &str = r"\w \$ ";

/// NSH-10: whether a PS1 is zsh's, as one exported by a zsh prompt (powerlevel10k,
/// oh-my-zsh): zsh's `%` escapes or its prompt functions, which bash cannot show.
pub fn is_zsh(ps1: &str) -> bool {
    if ["_p9k", "${(", "$+functions", "%{", "%F{", "%K{", "%B", "%b"].iter().any(|m| ps1.contains(m)) {
        return true;
    }
    let mut chars = ps1.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            if let Some(n) = chars.peek() {
                if "~nmMdcC/#?(jlTt*D@hiLx!".contains(*n) {
                    return true;
                }
            }
        }
    }
    false
}

/// NSH-73: the parts of neosh's own prompt `NEOSH_PROMPT` can name, in the
/// order they are shown, with what each is.
pub const PARTS: &[(&str, &str)] = &[
    ("user", "your user name"),
    ("host", "this computer's name"),
    ("dir", "the directory"),
    ("folder", "only the directory's last folder"),
    ("branch", "the git branch, * when files changed"),
    ("took", "how long a slow command took"),
    ("status", "a failed command's status"),
    ("time", "the time"),
    ("newline", "$ on a line of its own"),
];

/// NSH-73: what neosh's own prompt shows with no `NEOSH_PROMPT`.
pub const DEFAULT_PARTS: &str = "dir branch took status";

/// NSH-73: the parts a `NEOSH_PROMPT` names, in their order; and the words
/// in it that are not parts.
pub fn shown(spec: Option<&str>) -> (Vec<&'static str>, Vec<String>) {
    let words: Vec<&str> = spec.unwrap_or(DEFAULT_PARTS).split_whitespace().collect();
    let unknown = words.iter().filter(|w| !PARTS.iter().any(|(p, _)| p == *w)).map(|w| (*w).to_string()).collect();
    (PARTS.iter().map(|(p, _)| *p).filter(|p| words.contains(p)).collect(), unknown)
}

/// NSH-76: each part's colour unless `NEOSH_PROMPT_COLORS` says otherwise.
pub const DEFAULT_COLORS: &[(&str, &str)] =
    &[("user", "green"), ("host", "green"), ("dir", "bold,blue"), ("folder", "bold,blue"), ("branch", "magenta"), ("took", "yellow"), ("status", "red"), ("time", "dim")];

/// NSH-76: the SGR code of a colour or style: a name (`red`, `bright-red`),
/// a number up to 255, `#rrggbb`, or `bold`, `dim`, `italic`, `underline`.
pub fn color_code(color: &str, fg: bool) -> Result<String, ()> {
    match color {
        "bold" => return Ok("1".into()),
        "dim" => return Ok("2".into()),
        "italic" => return Ok("3".into()),
        "underline" => return Ok("4".into()),
        _ => {}
    }
    if let Some(bright) = color.strip_prefix("bright-") {
        const NAMES: [&str; 8] = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
        if let Some(i) = NAMES.iter().position(|n| *n == bright) {
            return Ok((if fg { 90 } else { 100 } + i).to_string());
        }
    }
    let base = if fg { 30 } else { 40 };
    const NAMES: [&str; 8] = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
    if let Some(i) = NAMES.iter().position(|n| *n == color) {
        return Ok((base + i).to_string());
    }
    if color == "default" {
        return Ok((base + 9).to_string());
    }
    if let Ok(n) = color.parse::<u8>() {
        return Ok(if n < 8 { (base + n as usize).to_string() } else { format!("{};5;{n}", base + 8) });
    }
    if let Some(hex) = color.strip_prefix('#').filter(|h| h.len() == 6) {
        let part = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ());
        return Ok(format!("{};2;{};{};{}", base + 8, part(0)?, part(2)?, part(4)?));
    }
    Err(())
}

/// NSH-76: the colour of each part, `NEOSH_PROMPT_COLORS` over the default
/// ones (`none` alone: no colours); and the words in it that are not right.
pub fn colors(spec: Option<&str>) -> (Vec<(&'static str, String)>, Vec<String>) {
    let mut out: Vec<(&'static str, String)> = DEFAULT_COLORS
        .iter()
        .map(|(p, c)| (*p, c.split(',').filter_map(|s| color_code(s, true).ok()).collect::<Vec<_>>().join(";")))
        .collect();
    let mut wrong = Vec::new();
    let spec = spec.unwrap_or("").trim();
    if spec == "none" {
        return (out.into_iter().map(|(p, _)| (p, String::new())).collect(), wrong);
    }
    for word in spec.split_whitespace() {
        let Some((part, styles)) = word.split_once('=') else {
            wrong.push(word.to_string());
            continue;
        };
        let codes: Result<Vec<String>, ()> =
            if styles == "none" { Ok(Vec::new()) } else { styles.split(',').map(|s| color_code(s, true)).collect() };
        match (out.iter_mut().find(|(p, _)| *p == part), codes) {
            (Some(slot), Ok(codes)) => slot.1 = codes.join(";"),
            _ => wrong.push(word.to_string()),
        }
    }
    (out, wrong)
}

/// The time now, `14:05`, in the local time zone.
pub fn clock() -> String {
    // SAFETY: time and localtime_r fill the values given.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return String::new();
        }
        format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
    }
}

/// What the prompt shows.
pub struct Parts<'a> {
    /// NSH-73: which parts, from `shown`.
    pub show: &'a [&'a str],
    pub user: &'a str,
    pub host: &'a str,
    pub clock: &'a str,
    /// NSH-76: each part's SGR code, from `colors`.
    pub colors: &'a [(&'static str, String)],
    pub dir: &'a Path,
    pub home: Option<&'a Path>,
    pub branch: Option<String>,
    /// Whether tracked files have changes (NSH-22).
    pub changed: bool,
    pub status: u8,
    /// How long the last command took, when long enough to show (NSH-15).
    pub took: Option<std::time::Duration>,
    pub root: bool,
    pub color: bool,
}

/// NSH-12, NSH-73: the prompt, `~/proj main 1 $ `, of the parts `show` names.
pub fn render(p: &Parts) -> String {
    let paint = |part: &str, text: &str| match p.colors.iter().find(|(n, _)| *n == part).map(|(_, c)| c.as_str()) {
        Some(code) if p.color && !code.is_empty() => format!("\x1b[{code}m{text}\x1b[0m"),
        _ => text.to_string(),
    };
    let on = |part: &str| p.show.contains(&part);
    let mut pieces = Vec::new();
    let who = match (on("user"), on("host")) {
        (true, true) => format!("{}@{}", p.user, p.host),
        (true, false) => p.user.to_string(),
        (false, true) => p.host.to_string(),
        (false, false) => String::new(),
    };
    if !who.is_empty() {
        pieces.push(paint(if on("user") { "user" } else { "host" }, &who));
    }
    if on("folder") {
        let full = tilde(p.dir, p.home);
        let last = if full == "~" || full == "/" { full } else { full.rsplit('/').next().unwrap_or_default().to_string() };
        pieces.push(paint("folder", &last));
    } else if on("dir") {
        pieces.push(paint("dir", &tilde(p.dir, p.home)));
    }
    if let Some(branch) = p.branch.as_ref().filter(|_| on("branch")) {
        pieces.push(paint("branch", &if p.changed { format!("{branch}*") } else { branch.clone() }));
    }
    if let Some(took) = p.took.and_then(long).filter(|_| on("took")) {
        pieces.push(paint("took", &took));
    }
    if p.status != 0 && on("status") {
        pieces.push(paint("status", &p.status.to_string()));
    }
    if on("time") && !p.clock.is_empty() {
        pieces.push(paint("time", p.clock));
    }
    let sign = if p.root { "# " } else { "$ " };
    match (pieces.is_empty(), on("newline")) {
        (true, _) => sign.to_string(),
        (false, true) => format!("{}\n{sign}", pieces.join(" ")),
        (false, false) => format!("{} {sign}", pieces.join(" ")),
    }
}

/// NSH-15: a duration worth showing, `12s`, `1m 4s` or `2h 5m`; none under 5 seconds.
pub fn long(took: std::time::Duration) -> Option<String> {
    let s = took.as_secs();
    match s {
        0..5 => None,
        5..60 => Some(format!("{s}s")),
        60..3600 => Some(format!("{}m {}s", s / 60, s % 60)),
        _ => Some(format!("{}h {}m", s / 3600, s % 3600 / 60)),
    }
}

/// The directory with `~` for home, also when `$HOME` is reached through a
/// symlink (as `/var` is `/private/var` on macOS) and the directory is not.
pub fn tilde(dir: &Path, home: Option<&Path>) -> String {
    let under = |h: &Path| dir.strip_prefix(h).ok().map(Path::to_path_buf);
    let rel = home.and_then(|h| under(h).or_else(|| std::fs::canonicalize(h).ok().and_then(|real| under(&real))));
    match rel.as_deref() {
        Some(rel) if rel.as_os_str().is_empty() => "~".to_string(),
        Some(rel) => format!("~/{}", rel.display()),
        None => dir.display().to_string(),
    }
}

/// NSH-12: the branch checked out in the repository holding `dir`, or the
/// short hash of a detached HEAD; read from `.git`, without running git.
pub fn branch(dir: &Path) -> Option<String> {
    let git = dir.ancestors().map(|d| d.join(".git")).find(|g| g.exists())?;
    // A worktree or submodule's `.git` is a file naming the real one.
    let git = if git.is_file() {
        let said = std::fs::read_to_string(&git).ok()?;
        let named = PathBuf::from(said.trim().strip_prefix("gitdir:")?.trim());
        if named.is_absolute() { named } else { git.parent()?.join(named) }
    } else {
        git
    };
    let head = std::fs::read_to_string(git.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref:") {
        Some(r) => Some(r.trim().strip_prefix("refs/heads/").unwrap_or(r.trim()).to_string()),
        None if head.len() >= 7 && head.chars().all(|c| c.is_ascii_hexdigit()) => Some(head[..7].to_string()),
        None => None,
    }
}

/// NSH-22: whether files git tracks in the repository holding `dir` have
/// changes, staged or not; false when `git` cannot say within `wait`.
pub fn changed(git: &Path, dir: &Path, wait: std::time::Duration) -> bool {
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new(git)
        .args(["--no-optional-locks", "status", "--porcelain", "--untracked-files=no", "--ignore-submodules=dirty"])
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let until = std::time::Instant::now() + wait;
    loop {
        match child.try_wait() {
            Ok(Some(done)) => {
                let mut said = Vec::new();
                let _ = std::io::Read::read_to_end(&mut child.stdout.take().unwrap(), &mut said);
                return done.success() && !said.is_empty();
            }
            Ok(None) if std::time::Instant::now() < until => std::thread::sleep(std::time::Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NSH-10
    #[test]
    fn a_ps1_from_zsh_is_told_from_bash_s() {
        assert!(is_zsh("${${_p9k_on_expand()}}%F{blue}%~%f "));
        assert!(is_zsh("%n@%m %~ %# "));
        assert!(!is_zsh(r"\u@\h \w \$ "));
        assert!(!is_zsh(r"[\#] \$ "));
        assert!(!is_zsh("100% \\$ "));
    }

    const DEFAULT: &[&str] = &["dir", "branch", "took", "status"];

    fn parts(dir: &Path) -> Parts<'_> {
        Parts {
            show: DEFAULT,
            user: "u",
            host: "box",
            clock: "14:05",
            colors: &[],
            dir,
            home: Some(Path::new("/home/u")),
            branch: None,
            changed: false,
            status: 0,
            took: None,
            root: false,
            color: false,
        }
    }

    // NSH-76
    #[test]
    fn each_part_s_colour_can_be_chosen() {
        let (c, wrong) = colors(Some("dir=green branch=bright-yellow,bold time=#ff8800 status=none nope=red took=sparkle"));
        let code = |part: &str| c.iter().find(|(p, _)| *p == part).unwrap().1.clone();
        assert_eq!((code("dir"), code("branch"), code("time"), code("status")), ("32".into(), "93;1".into(), "38;2;255;136;0".into(), String::new()));
        assert_eq!(code("user"), "32", "the rest keep their default");
        assert_eq!(wrong, ["nope=red", "took=sparkle"]);
        assert!(colors(Some("none")).0.iter().all(|(_, c)| c.is_empty()));
        let dir = Path::new("/home/u/proj");
        let p = Parts { color: true, colors: &c, branch: Some("main".into()), status: 1, ..parts(dir) };
        assert_eq!(render(&p), "\x1b[32m~/proj\x1b[0m \x1b[93;1mmain\x1b[0m 1 $ ");
    }

    // NSH-73
    #[test]
    fn the_prompt_shows_the_parts_named() {
        assert_eq!(shown(None).0, DEFAULT);
        let (show, unknown) = shown(Some("time branch user host folder nope newline"));
        assert_eq!(show, ["user", "host", "folder", "branch", "time", "newline"]);
        assert_eq!(unknown, ["nope"]);
        let dir = Path::new("/home/u/proj/neosh");
        let p = Parts { show: &show, branch: Some("main".into()), status: 1, ..parts(dir) };
        assert_eq!(render(&p), "u@box neosh main 14:05\n$ ");
        assert_eq!(render(&Parts { show: &["host", "dir"], ..parts(dir) }), "box ~/proj/neosh $ ");
        assert_eq!(render(&Parts { show: &["folder"], ..parts(Path::new("/home/u")) }), "~ $ ");
        assert_eq!(render(&Parts { show: &[], status: 1, ..parts(dir) }), "$ ");
    }

    // NSH-10, NSH-12
    #[test]
    fn the_prompt_shows_where_which_branch_and_a_failure() {
        let dir = Path::new("/home/u/proj");
        assert_eq!(render(&parts(dir)), "~/proj $ ");
        assert_eq!(render(&Parts { branch: Some("main".into()), status: 1, ..parts(dir) }), "~/proj main 1 $ ");
        assert_eq!(render(&Parts { branch: Some("main".into()), changed: true, ..parts(dir) }), "~/proj main* $ ");
        assert_eq!(render(&Parts { root: true, ..parts(Path::new("/etc")) }), "/etc # ");
        assert_eq!(render(&parts(Path::new("/home/u"))), "~ $ ");
        let slow = Parts { took: Some(std::time::Duration::from_secs(64)), status: 1, ..parts(dir) };
        assert_eq!(render(&slow), "~/proj 1m 4s 1 $ ");
        let colors = colors(None).0;
        let red = render(&Parts { status: 2, color: true, colors: &colors, ..parts(dir) });
        assert!(red.contains("\x1b[31m2\x1b[0m"), "{red:?}");
    }

    // NSH-10
    #[test]
    fn home_is_tilde_through_a_symlink_too() {
        let top = std::env::temp_dir().join(format!("neosh-tilde-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&top);
        std::fs::create_dir_all(top.join("real/proj")).unwrap();
        std::os::unix::fs::symlink(top.join("real"), top.join("link")).unwrap();
        let real = std::fs::canonicalize(top.join("real/proj")).unwrap();
        assert_eq!(tilde(&real, Some(&top.join("link"))), "~/proj");
        std::fs::remove_dir_all(&top).unwrap();
    }

    // NSH-15
    #[test]
    fn only_a_slow_command_s_time_is_shown() {
        let d = std::time::Duration::from_millis;
        assert_eq!(long(d(4999)), None);
        assert_eq!(long(d(5000)).as_deref(), Some("5s"));
        assert_eq!(long(d(59_900)).as_deref(), Some("59s"));
        assert_eq!(long(d(3_599_000)).as_deref(), Some("59m 59s"));
        assert_eq!(long(d(7_500_000)).as_deref(), Some("2h 5m"));
    }

    // NSH-12
    #[test]
    fn the_branch_is_read_from_git_s_files() {
        let top = std::env::temp_dir().join(format!("neosh-prompt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&top);
        let deep = top.join("repo/src/a");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(top.join("repo/.git")).unwrap();
        assert_eq!(branch(&top), None);
        std::fs::write(top.join("repo/.git/HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        assert_eq!(branch(&deep).as_deref(), Some("feature/x"));
        std::fs::write(top.join("repo/.git/HEAD"), "0796b0b1234567890abcdef0796b0b1234567890\n").unwrap();
        assert_eq!(branch(&deep).as_deref(), Some("0796b0b"));
        // A worktree: `.git` is a file pointing at the real one.
        std::fs::create_dir_all(top.join("wt")).unwrap();
        std::fs::create_dir_all(top.join("repo/.git/worktrees/wt")).unwrap();
        std::fs::write(top.join("repo/.git/worktrees/wt/HEAD"), "ref: refs/heads/other\n").unwrap();
        std::fs::write(top.join("wt/.git"), "gitdir: ../repo/.git/worktrees/wt\n").unwrap();
        assert_eq!(branch(&top.join("wt")).as_deref(), Some("other"));
        std::fs::remove_dir_all(&top).unwrap();
    }

    // NSH-22
    #[test]
    fn changes_to_tracked_files_are_asked_of_git() {
        let top = std::env::temp_dir().join(format!("neosh-changed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&top);
        std::fs::create_dir_all(&top).unwrap();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git").args(args).current_dir(&top)
                .env("GIT_CONFIG_GLOBAL", "/dev/null").env("GIT_CONFIG_NOSYSTEM", "1")
                .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().unwrap();
            assert!(ok.success(), "git {args:?}");
        };
        let wait = std::time::Duration::from_secs(5);
        let real = Path::new("git");
        git(&["init", "-q"]);
        std::fs::write(top.join("a"), "one\n").unwrap();
        git(&["add", "a"]);
        git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-qm", "a"]);
        assert!(!changed(real, &top, wait), "a fresh commit is no change");
        std::fs::write(top.join("new"), "untracked\n").unwrap();
        assert!(!changed(real, &top, wait), "an untracked file is no change");
        std::fs::write(top.join("a"), "two\n").unwrap();
        assert!(changed(real, &top, wait), "an edit is");
        git(&["add", "a"]);
        assert!(changed(real, &top, wait), "a staged edit is too");
        assert!(!changed(&top.join("no-git"), &top, wait), "no git: no mark");
        // A git that does not answer in time holds nothing up.
        let slow = top.join("slow-git");
        std::fs::write(&slow, "#!/bin/sh\nsleep 5\necho ' M a'\n").unwrap();
        std::fs::set_permissions(&slow, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let at = std::time::Instant::now();
        assert!(!changed(&slow, &top, std::time::Duration::from_millis(200)));
        assert!(at.elapsed() < std::time::Duration::from_secs(2), "{:?}", at.elapsed());
        std::fs::remove_dir_all(&top).unwrap();
    }
}
