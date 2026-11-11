//! Setup (NSH-47 to NSH-51): how the prompt looks, plugins, then the model
//! navi answers with, in the terminal, written to `~/.neoshrc` between markers as zsh's
//! first-run setup writes `~/.zshrc`.

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const BEGIN: &str = "# >>> neosh setup >>>";
pub const END: &str = "# <<< neosh setup <<<";
/// What "never" leaves in `~/.neoshrc`, so neosh does not offer again.
const NEVER: &str = "# neosh: setup declined; run `neosh --setup` any time.\n";
const INSTALL_NAVI: &str = "curl -fsSL https://raw.githubusercontent.com/Jeanno/neoai/master/neonavi/install.sh | sh";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Neosh,
    Zsh,
    /// NSH-77: starship's prompt.
    Starship,
}

/// NSH-77: the line that makes starship neosh's prompt.
/// Guarded, so a neosh with no starship draws its own.
const STARSHIP_INIT: &str = "command -v starship >/dev/null && eval \"$(starship init bash)\"";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub look: Option<Look>,
    pub yes: bool,
    /// Offered as neosh starts, so what is written counts at once.
    pub at_start: bool,
    /// NSH-75: the parts of neosh's own prompt, from `--prompt`.
    pub parts: Option<String>,
    /// NSH-76: their colours, from `--colors`.
    pub colors: Option<String>,
    /// NSH-77: starship's preset, from `--preset`.
    pub preset: Option<String>,
    /// NSH-78: the plugins, from `--plugins`.
    pub plugins: Option<Vec<String>>,
}

/// NSH-51: what follows `--setup`.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.iter();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--yes" | "-y" => options.yes = true,
            "--look" => {
                options.look = Some(match args.next().map(String::as_str) {
                    Some("zsh") => Look::Zsh,
                    Some("neosh") => Look::Neosh,
                    Some("starship") => Look::Starship,
                    other => return Err(format!("--look takes zsh, neosh or starship, not {}", other.unwrap_or("nothing"))),
                })
            }
            // NSH-75: neosh's own look, with these parts.
            "--prompt" => {
                let words = args.next().ok_or("--prompt takes the parts to show, as 'dir branch time'")?;
                let (shown, unknown) = crate::prompt::shown(Some(words));
                if let Some(word) = unknown.first() {
                    let known: Vec<&str> = crate::prompt::PARTS.iter().map(|(p, _)| *p).collect();
                    return Err(format!("--prompt: {word} is not a part (parts: {})", known.join(" ")));
                }
                options.parts = Some(shown.join(" "));
                options.look = Some(Look::Neosh);
            }
            // NSH-78
            "--plugins" => {
                let words = args.next().ok_or("--plugins takes the plugins to load, as 'z' ('' for none)")?;
                let names: Vec<String> = words.split_whitespace().map(String::from).collect();
                if let Some(name) = names.iter().find(|n| !crate::plugins::KNOWN.iter().any(|(k, ..)| k == n)) {
                    let known: Vec<&str> = crate::plugins::KNOWN.iter().map(|(k, ..)| *k).collect();
                    return Err(format!("--plugins: {name} is not a plugin neosh knows (known: {})", known.join(" ")));
                }
                options.plugins = Some(names);
            }
            // NSH-76
            "--colors" => {
                let words = args.next().ok_or("--colors takes part=colour words, as 'dir=green branch=yellow'")?;
                if let Some(word) = crate::prompt::colors(Some(words)).1.first() {
                    return Err(format!("--colors: {word} is not part=colour (a colour is a name, bright-<name>, 0-255, #rrggbb, bold, dim, italic, underline or none)"));
                }
                options.colors = Some(words.to_string());
                options.look = Some(Look::Neosh);
            }
            // NSH-77
            "--preset" => {
                options.preset = Some(args.next().ok_or("--preset takes one of starship's presets, as tokyo-night")?.to_string());
                options.look = Some(Look::Starship);
            }
            other => return Err(format!("unknown option {other} for --setup")),
        }
    }
    Ok(options)
}

/// NSH-47: offered when an interactive neosh starts with no `~/.neoshrc`.
pub fn offer(rc: &Path) {
    println!("{}", bold("Welcome to neosh."));
    println!("Start a line with ? to ask navi anything. Setup takes a minute: how the prompt\nlooks, plugins, the model that answers ?, and your login shell.");
    match ask("Set neosh up now? [Y]es, [l]ater, [n]ever: ").as_str() {
        "" | "y" | "yes" => {
            let _ = run(&Options { at_start: true, ..Options::default() }, rc);
        }
        "n" | "never" => {
            if std::fs::write(rc, NEVER).is_ok() {
                println!("{}\n", dim("Not asking again. Run `neosh --setup` any time."));
            }
        }
        _ => println!("{}\n", dim("Later, then: it is offered next time, or run `neosh --setup`.")),
    }
}

/// What setup writes for neosh's own look: no prompt, only the choice, so a
/// first start is not offered setup again (NSH-50).
const OWN_LOOK: &str = "# neosh's own look; `neosh --setup` changes it.";

/// NSH-47 to NSH-50: setup itself; the exit status.
pub fn run(options: &Options, rc: &Path) -> i32 {
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if !terminal && !(options.look.is_some() && options.yes) {
        eprintln!("neosh: --setup asks its questions on a terminal; without one, give --look zsh|neosh|starship (or --prompt '<parts>') and --yes");
        return 1;
    }
    // Every step is asked unless the look was given (NSH-51).
    let steps = options.look.is_none();
    let before = std::fs::read_to_string(rc).unwrap_or_default();
    // NSH-49: the look first, starship's the default (NSH-77).
    if steps {
        println!("\n{}", bold("Step 1 of 4 · How the prompt looks"));
        println!("  1  starship     {}", dim("themes and icons, drawn by starship"));
        println!("  2  neosh's own  {}", dim("the directory, git branch and a failed status"));
        println!("  3  my own       {}", dim("neosh's, with the parts and colours you pick"));
        println!("  4  like my zsh  {}", dim("its prompt and colours"));
    }
    let mut parts = options.parts.clone();
    let mut colors = options.colors.clone();
    let mut preset = options.preset.clone();
    let mut look = options.look.unwrap_or_else(|| match ask("Choose [1]: ").as_str() {
        "2" => Look::Neosh,
        "3" => {
            let picked = pick(&setting_in(&before, "NEOSH_PROMPT").unwrap_or_else(|| crate::prompt::DEFAULT_PARTS.into()), terminal);
            colors = pick_colors(&picked, setting_in(&before, "NEOSH_PROMPT_COLORS"), terminal);
            parts = Some(picked);
            Look::Neosh
        }
        "4" => Look::Zsh,
        _ => Look::Starship,
    });
    // NSH-77: starship, installed if need be, and the preset that becomes its config.
    let starship = if look == Look::Starship {
        match find("starship").or_else(|| if steps { install_starship() } else { None }) {
            None if !steps => {
                println!("starship draws this prompt and is not installed. Install it, then run neosh --setup again:");
                println!("  brew install starship\n  {}", dim(&format!("or: {INSTALL_STARSHIP}")));
                return if options.yes { 1 } else { 0 };
            }
            None => {
                println!("{}", dim("· neosh's own look for now; run neosh --setup once starship is installed."));
                look = Look::Neosh;
                None
            }
            Some(bin) => {
                if steps {
                    preset = pick_preset(&bin, terminal);
                }
                if let Some(name) = &preset {
                    if !presets(&bin).contains(name) {
                        eprintln!("neosh: {name} is not one of starship's presets ({})", presets(&bin).join(", "));
                        return 2;
                    }
                }
                Some(bin)
            }
        }
    } else {
        None
    };
    // NSH-78: the plugins; an earlier setup's are kept when not asked.
    let plugins = match &options.plugins {
        _ if own_plugins(&before) => {
            if steps || options.plugins.is_some() {
                if steps {
                    println!("\n{}", bold("Step 2 of 4 · Plugins"));
                }
                println!("{}", dim(&format!("· {} names its own plugins; they stay as they are.", tilde(rc))));
            }
            None
        }
        Some(names) => Some(names.clone()),
        None if steps => Some(pick_plugins(earlier_plugins(&before))),
        None => earlier_plugins(&before),
    };
    // NSH-79: zsh, asked once, for its look and its PATH.
    let zsh = if find("zsh").is_some() { ask_zsh() } else { Err("zsh is not installed, so there is no zsh look to copy".to_string()) };
    // NSH-49
    let mut lines = match look {
        // NSH-74: the parts, unless they are the default ones.
        Look::Neosh => {
            let mut lines = vec![OWN_LOOK.to_string()];
            if let Some(p) = parts.filter(|p| p != crate::prompt::DEFAULT_PARTS) {
                lines.push(format!("NEOSH_PROMPT={}", quote(&p)));
            }
            // NSH-76: the colours, unless they are the default ones.
            if let Some(c) = colors.filter(|c| !c.trim().is_empty()) {
                lines.push(format!("NEOSH_PROMPT_COLORS={}", quote(c.trim())));
            }
            lines
        }
        Look::Starship => {
            let mut lines = Vec::new();
            // Installed where PATH does not look (~/.local/bin): put it there.
            if let Some(dir) = starship.as_ref().and_then(|b| b.parent()).filter(|_| find("starship").is_none()) {
                lines.push(format!("export PATH={}:\"$PATH\"", quote(&dir.display().to_string())));
            }
            lines.push(STARSHIP_INIT.to_string());
            lines
        }
        Look::Zsh => {
            let (lines, notes) = zsh_look(&zsh);
            for note in notes {
                println!("{}", dim(&format!("· {note}")));
            }
            lines
        }
    };
    // NSH-79: the folders zsh puts on PATH, first, so what follows finds them.
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(line) = zsh.as_ref().ok().and_then(|z| path_line(&z.path, &system_path(), home.as_deref(), |d| Path::new(d).is_dir())) {
        println!("{}", dim("· The folders your zsh adds to PATH are added for neosh too."));
        lines.insert(usize::from(look == Look::Neosh && lines.first().is_some_and(|l| l == OWN_LOOK)), line);
    }
    if let Some(names) = plugins.filter(|p| !p.is_empty()) {
        lines.push(format!("plugins=({})", names.join(" ")));
    }
    // NSH-50: shown, then written only on yes; neosh's own look, which sets
    // nothing, is written without asking unless it takes an earlier look out.
    let after = with_block(&before, &lines);
    let earlier = block(&before).is_some_and(|b| b.iter().any(|l| l.trim() != OWN_LOOK && !l.trim().is_empty()));
    // NSH-77: the preset goes in starship's config, the one before kept beside it.
    let config = starship.as_ref().and(preset.as_ref()).map(|name| (name.clone(), starship_config()));
    if let Some((name, path)) = &config {
        let kept = if path.exists() { format!(" (the one there now is kept as {}.bak)", tilde(path)) } else { String::new() };
        println!("{} becomes starship's preset {name}{kept}.", tilde(path));
    }
    if after == before && config.is_none() {
        println!("{}", dim(&format!("· {} already has this look.", tilde(rc))));
    } else {
        let custom = lines.len() > 1;
        let asking = !options.yes && (look != Look::Neosh || custom || earlier);
        if look == Look::Neosh && earlier && !custom {
            println!("neosh's own look: the lines an earlier setup wrote in {} go.", tilde(rc));
        } else if (look != Look::Neosh || custom) && after != before {
            println!("This goes in {}:\n", tilde(rc));
            for line in &lines {
                println!("  {line}");
            }
            println!();
        }
        let write = !asking || yes_no("Write it? [Y/n] ", true);
        if !write {
            println!("Nothing written.");
        }
        if write && after != before {
            if let Err(e) = std::fs::write(rc, after) {
                eprintln!("neosh: could not write {}: {e}", rc.display());
                return 1;
            }
            println!("{}", dim(&format!("· Saved in {}.", tilde(rc))));
        }
        if let (true, Some(bin), Some((name, path))) = (write, &starship, &config) {
            if let Err(e) = write_preset(bin, name, path) {
                eprintln!("neosh: could not write {}: {e}", path.display());
                return 1;
            }
            println!("{}", dim(&format!("· Saved starship's preset {name} in {}.", tilde(path))));
        }
    }
    // NSH-48: the model navi answers with, by navi's own setup.
    if steps {
        println!("\n{}", bold("Step 3 of 4 · The model that answers ?"));
        match find("navi") {
            // navi's setup says itself when it leaves no model answering (SET-29).
            Some(navi) => {
                let _ = Command::new(navi).arg("setup").status();
            }
            None => println!("navi answers ? and is not installed. Install it, then run neosh --setup again:\n  {INSTALL_NAVI}"),
        }
    }
    // NSH-80: neosh as the login shell, asked; no unless yes.
    if steps {
        println!("\n{}", bold("Step 4 of 4 · Your login shell"));
        login_step();
    }
    if terminal {
        println!("\n{}", bold("neosh is ready."));
        println!("  ?what's in this folder  {}", dim("ask navi anything"));
        println!("  ?                       {}", dim("navi mode: every line goes to navi, until Esc"));
        println!("  neosh --setup           {}", dim("change any of this"));
        if !options.at_start {
            println!("{}", dim(&format!("The look counts in a new neosh, or after: source {}", tilde(rc))));
        }
        println!();
    }
    0
}

const INSTALL_STARSHIP: &str = "curl -sS https://starship.rs/install.sh | sh";

/// NSH-77: installs starship, chosen and not there: by Homebrew when there
/// is one, else by starship's installer into `~/.local/bin`; where it is.
fn install_starship() -> Option<PathBuf> {
    println!("\nstarship draws this prompt and is not installed; installing it…");
    if let Some(brew) = find("brew") {
        println!("{}", dim("· brew install starship"));
        let ok = Command::new(&brew).args(["install", "starship"]).status().is_ok_and(|s| s.success());
        let beside = brew.parent().map(|d| d.join("starship")).filter(|p| p.is_file());
        if let Some(bin) = find("starship").or(beside).filter(|_| ok) {
            return Some(bin);
        }
        println!("brew could not install starship.");
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let dir = home.join(".local/bin");
        let _ = std::fs::create_dir_all(&dir);
        println!("{}", dim(&format!("· {INSTALL_STARSHIP} -s -- --yes --bin-dir {}", tilde(&dir))));
        let script = format!("{INSTALL_STARSHIP} -s -- --yes --bin-dir \"$1\"");
        let ok = Command::new("sh").args(["-c", &script, "sh"]).arg(&dir).status().is_ok_and(|s| s.success());
        let bin = dir.join("starship");
        if ok && bin.is_file() {
            return Some(bin);
        }
        println!("starship's installer did not install it.");
    }
    println!("Install it any time:\n  brew install starship\n  {}", dim(&format!("or: {INSTALL_STARSHIP}")));
    None
}

/// NSH-80: the login shell the system has for this user (not `$SHELL`,
/// which apps keep from when they started).
fn login_shell() -> Option<String> {
    let user = std::env::var("USER").ok()?;
    let out = |program: &str, args: &[&str]| {
        Command::new(program).args(args).stderr(Stdio::null()).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    if let Some(text) = out("dscl", &[".", "-read", &format!("/Users/{user}"), "UserShell"]) {
        return text.split_whitespace().nth(1).map(String::from);
    }
    out("getent", &["passwd", &user]).and_then(|line| line.trim().rsplit(':').next().map(String::from)).filter(|s| !s.is_empty())
}

/// NSH-80: offers to make this neosh the login shell: added to
/// `/etc/shells` (by sudo, which asks for the password) and set by `chsh`.
fn login_step() {
    let Some(me) = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok()) else { return };
    let me_str = me.display().to_string();
    let same = |shell: &str| Path::new(shell).canonicalize().is_ok_and(|p| p == me);
    if login_shell().is_some_and(|s| same(&s)) {
        println!("{}", dim("· neosh is already your login shell."));
        return;
    }
    println!("  {}", dim(&format!("Terminals then start neosh ({}); it asks for your password.", tilde(&me))));
    if !yes_no("Make neosh your login shell? [y/N] ", false) {
        println!("{}", dim("· Not changed. Run neosh --setup to do it later."));
        return;
    }
    let listed = std::fs::read_to_string("/etc/shells").unwrap_or_default().lines().any(|l| l.trim() == me_str || same(l.trim()));
    if !listed {
        let ok = find("sudo").is_some_and(|sudo| {
            let mut child = match Command::new(sudo).args(["tee", "-a", "/etc/shells"]).stdin(Stdio::piped()).stdout(Stdio::null()).spawn() {
                Ok(c) => c,
                Err(_) => return false,
            };
            let wrote = child.stdin.take().is_some_and(|mut i| writeln!(i, "{me_str}").is_ok());
            wrote && child.wait().is_ok_and(|s| s.success())
        });
        if !ok {
            println!("neosh could not be added to /etc/shells. To do it yourself:\n  echo {me_str} | sudo tee -a /etc/shells && chsh -s {me_str}");
            return;
        }
    }
    let ok = find("chsh").is_some_and(|chsh| Command::new(chsh).args(["-s", &me_str]).status().is_ok_and(|s| s.success()));
    if ok {
        println!("{}", dim("· neosh is your login shell. Apps already open keep their shell until you log out and back in."));
    } else {
        println!("chsh did not change it. To do it yourself:\n  chsh -s {me_str}");
    }
}

/// NSH-78: whether `plugins` is set outside setup's block, by the rc's own line.
fn own_plugins(rc: &str) -> bool {
    with_block(rc, &[]).lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("plugins=") || l.starts_with("plugins+=")
    })
}

/// NSH-78: the plugins an earlier setup chose.
fn earlier_plugins(rc: &str) -> Option<Vec<String>> {
    let value = setting_in(rc, "plugins")?;
    Some(value.trim_matches(['(', ')']).split_whitespace().map(|w| w.trim_matches(['\'', '"']).to_string()).collect())
}

/// NSH-78: each plugin neosh knows, asked in turn; yes unless an earlier
/// setup left it out.
fn pick_plugins(earlier: Option<Vec<String>>) -> Vec<String> {
    println!("\n{}", bold("Step 2 of 4 · Plugins"));
    let mut taken = Vec::new();
    for (name, repo, what) in crate::plugins::KNOWN {
        let default = earlier.as_ref().is_none_or(|e| e.iter().any(|n| n == name));
        println!("  {name}  {}", dim(&format!("{what} ({repo})")));
        let question = format!("Add {name}? {} ", if default { "[Y/n]" } else { "[y/N]" });
        if yes_no(&question, default) {
            taken.push(name.to_string());
        }
    }
    taken
}

/// NSH-74: what an earlier setup set `name` to, from its `name=` line.
fn setting_in(rc: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    block(rc)?.iter().find_map(|l| l.trim().strip_prefix(prefix.as_str())).map(|v| v.trim_matches(['\'', '"']).to_string())
}

/// NSH-74: the parts of neosh's own prompt, picked by number with the prompt
/// shown as it would be after each change; the parts picked.
fn pick(start: &str, terminal: bool) -> String {
    use crate::prompt::{PARTS, shown};
    let mut on: Vec<&str> = shown(Some(start)).0;
    println!("\nType numbers to switch parts on or off ({}), Enter when done.", dim("as 4 8"));
    loop {
        for (i, (part, what)) in PARTS.iter().enumerate() {
            let mark = if on.contains(part) { "[x]" } else { "[ ]" };
            println!("  {:>2} {mark} {part:<8} {}", i + 1, dim(what));
        }
        let show: Vec<&str> = PARTS.iter().map(|(p, _)| *p).filter(|p| on.contains(p)).collect();
        let preview = preview(&show, None, terminal);
        println!("\n  {}\n{preview}\n", dim("After a command that failed and took 12s:"));
        let answer = ask("Switch [Enter: done]: ");
        if answer.is_empty() {
            return show.join(" ");
        }
        for word in answer.split([' ', ',']).filter(|w| !w.is_empty()) {
            match word.parse::<usize>().ok().and_then(|n| PARTS.get(n.wrapping_sub(1))) {
                Some((part, _)) => match on.iter().position(|p| p == part) {
                    Some(at) => {
                        on.remove(at);
                    }
                    None => {
                        on.push(part);
                        // The whole directory or its last folder, not both.
                        let other = match *part { "dir" => Some("folder"), "folder" => Some("dir"), _ => None };
                        on.retain(|p| Some(*p) != other);
                    }
                },
                None => println!("{}", dim(&format!("· {word} is not one of the numbers"))),
            }
        }
    }
}

/// NSH-74: neosh's own prompt with these parts and colours, as after a
/// command that failed and took 12s, in the directory setup runs in.
fn preview(show: &[&str], colors: Option<&str>, terminal: bool) -> String {
    use crate::prompt::{Parts, branch, clock, render};
    let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let user = std::env::var("USER").unwrap_or_else(|_| "you".into());
    render(&Parts {
        show,
        user: &user,
        host: &hostname(),
        clock: &clock(),
        colors: &crate::prompt::colors(colors).0,
        dir: &dir,
        home: home.as_deref(),
        branch: Some(branch(&dir).unwrap_or_else(|| "main".into())),
        changed: false,
        status: 1,
        took: Some(std::time::Duration::from_secs(12)),
        root: false,
        color: terminal && std::env::var_os("NO_COLOR").is_none(),
    })
}

/// NSH-76: the colours of the parts picked, typed as `part=colour` with the
/// prompt shown again each time; `None` keeps neosh's.
fn pick_colors(parts: &str, start: Option<String>, terminal: bool) -> Option<String> {
    let show: Vec<&str> = parts.split_whitespace().collect();
    let mut colors = start;
    println!(
        "\nColours: type part=colour to change one ({}), none for no colours, default for neosh's; Enter when done.",
        dim("as dir=green branch=bright-yellow,bold time=#888888")
    );
    loop {
        println!("\n{}\n", preview(&show, colors.as_deref(), terminal));
        let answer = ask("Colours [Enter: done]: ");
        match answer.as_str() {
            "" => return colors,
            "default" => colors = None,
            "none" => colors = Some("none".into()),
            _ => {
                let (_, wrong) = crate::prompt::colors(Some(&answer));
                if let Some(word) = wrong.first() {
                    println!("{}", dim(&format!("· {word} is not part=colour: a colour is a name, bright-<name>, 0-255, #rrggbb, bold, dim, italic, underline or none")));
                    continue;
                }
                // Later words win over earlier ones for the same part.
                let mut all: Vec<String> = colors.filter(|c| c != "none").map(|c| c.split_whitespace().map(String::from).collect()).unwrap_or_default();
                for word in answer.split_whitespace() {
                    let part = word.split('=').next().unwrap_or_default();
                    all.retain(|w| w.split('=').next() != Some(part));
                    all.push(word.to_string());
                }
                colors = Some(all.join(" "));
            }
        }
    }
}

/// NSH-77: starship's presets, as `starship preset --list` names them.
fn presets(bin: &Path) -> Vec<String> {
    Command::new(bin)
        .args(["preset", "--list"])
        .stdin(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

/// NSH-77: where starship reads its config.
fn starship_config() -> PathBuf {
    match std::env::var_os("STARSHIP_CONFIG") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/starship.toml"),
    }
}

/// NSH-77: starship's prompt, with `config` (or its own), as after a
/// command that failed and took 12s.
fn starship_preview(bin: &Path, config: Option<&Path>) -> String {
    let mut command = Command::new(bin);
    command.args(["prompt", "--status=1", "--cmd-duration=12000"]).stdin(Stdio::null()).stderr(Stdio::null());
    if let Some(config) = config {
        command.env("STARSHIP_CONFIG", config);
    }
    let width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
    command.arg(format!("--terminal-width={width}"));
    command.output().map(|o| String::from_utf8_lossy(&o.stdout).trim_start_matches('\n').to_string()).unwrap_or_default()
}

/// NSH-77: picks a preset by number, each shown as starship would draw it;
/// `None` keeps the config there is (or starship's own).
fn pick_preset(bin: &Path, terminal: bool) -> Option<String> {
    let names = presets(bin);
    let config = starship_config();
    let first = if config.exists() { format!("keep yours ({})", tilde(&config)) } else { "starship's own".to_string() };
    let scratch = std::env::temp_dir().join(format!("neosh-starship-{}.toml", std::process::id()));
    let mut chosen: Option<String> = None;
    println!("\nstarship's presets: type a number to see it, Enter to take the one shown. {}", dim("Most need a Nerd Font in your terminal."));
    loop {
        println!("  {:>2}  {first}", 0);
        for (i, name) in names.iter().enumerate() {
            println!("  {:>2}  {name}", i + 1);
        }
        let shown = match &chosen {
            Some(name) if write_preset(bin, name, &scratch).is_ok() => starship_preview(bin, Some(&scratch)),
            _ => starship_preview(bin, None),
        };
        let label = chosen.clone().unwrap_or(first.clone());
        let shown = if terminal && std::env::var_os("NO_COLOR").is_none() { shown } else { strip_ansi(&shown) };
        println!("\n  {}\n{shown}\n", dim(&format!("{label}, after a command that failed and took 12s:")));
        let answer = ask("Preset [Enter: take it]: ");
        if answer.is_empty() {
            let _ = std::fs::remove_file(&scratch);
            return chosen;
        }
        match answer.parse::<usize>() {
            Ok(0) => chosen = None,
            Ok(n) if n <= names.len() => chosen = Some(names[n - 1].clone()),
            _ => println!("{}", dim(&format!("· {answer} is not one of the numbers"))),
        }
    }
}

/// NSH-77: writes starship's preset `name` to `path`, the file there kept as `.bak`.
fn write_preset(bin: &Path, name: &str, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let out = Command::new(bin).args(["preset", name]).stdin(Stdio::null()).output()?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err(std::io::Error::other(format!("starship has no preset {name}")));
    }
    if path.exists() && !path.to_string_lossy().contains("neosh-starship-") {
        let mut bak = path.as_os_str().to_owned();
        bak.push(".bak");
        std::fs::copy(path, bak)?;
    }
    std::fs::write(path, out.stdout)
}

fn strip_ansi(s: &str) -> String {
    regex::Regex::new("\x1b\\[[0-9;]*m").map(|r| r.replace_all(s, "").into_owned()).unwrap_or_else(|_| s.to_string())
}

/// This computer's short name.
fn hostname() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: gethostname writes at most the length given.
    if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } != 0 {
        return "host".into();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).split('.').next().unwrap_or("host").to_string()
}

/// The lines between setup's markers, when there are markers.
fn block(rc: &str) -> Option<Vec<&str>> {
    let mut lines = rc.lines().skip_while(|l| l.trim() != BEGIN);
    lines.next()?;
    Some(lines.take_while(|l| l.trim() != END).collect())
}

/// Bold and dim on a terminal that wants colour; plain otherwise.
fn styled(code: &str, s: &str) -> String {
    if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn bold(s: &str) -> String {
    styled("1", s)
}

fn dim(s: &str) -> String {
    styled("2", s)
}

/// `rc` with setup's block made `lines`, or taken out when there are none;
/// everything else as it was.
pub fn with_block(rc: &str, lines: &[String]) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in rc.lines() {
        match line.trim() {
            BEGIN => inside = true,
            END if inside => inside = false,
            _ if !inside => kept.push(line),
            _ => {}
        }
    }
    let mut out = kept.join("\n");
    while out.ends_with('\n') {
        out.pop();
    }
    if !lines.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out += &format!("{BEGIN}\n{}\n{END}", lines.join("\n"));
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// What zsh says of itself, as your zsh starts.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Zsh {
    pub prompt: String,
    pub rprompt: String,
    pub theme: String,
    pub starship: bool,
    pub p10k: bool,
    pub prompt_subst: bool,
    pub colors: Vec<(&'static str, String)>,
    /// NSH-79: zsh's PATH, as a login zsh makes it.
    pub path: String,
}

const SENTINEL: &str = "neosh-setup-look";
const COLOR_VARS: [&str; 3] = ["CLICOLOR", "LSCOLORS", "LS_COLORS"];

/// NSH-49: zsh's prompt and colours, asked of an interactive zsh with no terminal.
fn ask_zsh() -> Result<Zsh, String> {
    let zsh = find("zsh").ok_or("zsh is not installed, so there is no zsh look to copy")?;
    let script = format!(
        "print -rN -- {SENTINEL} \"$PROMPT\" \"$RPROMPT\" \"${{ZSH_THEME-}}\" \"${{+functions[starship_precmd]}}\" \"${{+functions[p10k]}}\" \"${{options[promptsubst]}}\" \"$PATH\" {}",
        COLOR_VARS.iter().map(|v| format!("\"${{{v}-}}\"")).collect::<Vec<_>>().join(" ")
    );
    let mut child = Command::new(zsh)
        // NSH-79: a login zsh, from the system's PATH, so the PATH it
        // makes is its own and not the one setup was started with.
        .args(["-l", "-i", "-c", &script])
        .env("PATH", system_path().join(":"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("zsh could not be started: {e}"))?;
    let started = Instant::now();
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        if started.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            return Err("zsh took more than 5 seconds to start, so its look was not read".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    parse_zsh(&String::from_utf8_lossy(&out.stdout)).ok_or_else(|| "zsh did not say what its prompt is".into())
}

/// What `ask_zsh`'s script printed, after anything `~/.zshrc` printed first.
pub fn parse_zsh(out: &str) -> Option<Zsh> {
    let at = out.find(&format!("{SENTINEL}\0"))?;
    let fields: Vec<&str> = out[at..].split('\0').skip(1).collect();
    let field = |i: usize| fields.get(i).copied().unwrap_or("").to_string();
    Some(Zsh {
        prompt: field(0),
        rprompt: field(1),
        theme: field(2),
        starship: field(3) == "1",
        p10k: field(4) == "1",
        prompt_subst: field(5) == "on",
        path: field(6),
        colors: COLOR_VARS.iter().enumerate().map(|(i, v)| (*v, field(7 + i))).filter(|(_, value)| !value.is_empty()).collect(),
    })
}

/// NSH-49: the lines for `~/.neoshrc` and what to say about them.
fn zsh_look(zsh: &Result<Zsh, String>) -> (Vec<String>, Vec<String>) {
    match zsh {
        Ok(zsh) => lines_for(zsh, find("starship").is_some()),
        Err(why) => (Vec::new(), vec![format!("{why}; neosh's own look is kept.")]),
    }
}

/// NSH-79: the PATH a login shell starts from: `/etc/paths` and
/// `/etc/paths.d` (macOS's path_helper), else the usual folders.
fn system_path() -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    let mut files = vec![PathBuf::from("/etc/paths")];
    let mut more: Vec<PathBuf> = std::fs::read_dir("/etc/paths.d").into_iter().flatten().flatten().map(|e| e.path()).collect();
    more.sort();
    files.extend(more);
    for file in files {
        for line in std::fs::read_to_string(file).unwrap_or_default().lines().map(str::trim) {
            if !line.is_empty() && !line.starts_with('#') && !dirs.iter().any(|d| d == line) {
                dirs.push(line.to_string());
            }
        }
    }
    if dirs.is_empty() {
        dirs = ["/usr/local/sbin", "/usr/local/bin", "/usr/sbin", "/usr/bin", "/sbin", "/bin"].map(String::from).to_vec();
    }
    dirs
}

/// NSH-79: the line that adds the folders zsh puts on PATH and a login
/// neosh would not have, each once; `None` when there are none.
pub fn path_line(zsh_path: &str, system: &[String], home: Option<&Path>, exists: impl Fn(&str) -> bool) -> Option<String> {
    let mut dirs: Vec<String> = Vec::new();
    for dir in zsh_path.split(':').filter(|d| d.starts_with('/')) {
        if !system.iter().any(|s| s == dir) && !dirs.iter().any(|d| d == dir) && exists(dir) {
            dirs.push(dir.to_string());
        }
    }
    if dirs.is_empty() {
        return None;
    }
    let home = home.map(|h| h.display().to_string());
    let words: Vec<String> = dirs
        .iter()
        .map(|d| match home.as_deref().and_then(|h| d.strip_prefix(h)).filter(|r| r.starts_with('/')) {
            Some(rest) => format!("\"$HOME\"{}", quote(rest)),
            None => quote(d),
        })
        .collect();
    Some(format!(
        "for d in {}; do case \":$PATH:\" in *\":$d:\"*) ;; *) PATH=\"$PATH:$d\" ;; esac; done; export PATH",
        words.join(" ")
    ))
}

/// The lines and notes for what zsh said; `starship` is whether neosh can run it.
pub fn lines_for(zsh: &Zsh, starship: bool) -> (Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut notes = Vec::new();
    let keep = "neosh's own prompt is kept, which shows the git branch and a failed status";
    if zsh.starship {
        if starship {
            lines.push(STARSHIP_INIT.to_string());
            notes.push("zsh's prompt is starship's: neosh uses starship too, so it looks the same.".into());
        } else {
            notes.push(format!("zsh's prompt is starship's, but starship is not on neosh's PATH; {keep}."));
        }
    } else if zsh.p10k {
        notes.push(format!("zsh's prompt is powerlevel10k's, which only zsh can draw; {keep}."));
    } else {
        match translate(&zsh.prompt, zsh.prompt_subst) {
            Ok((ps1, approximations)) => {
                lines.push(format!("PS1={}", quote(&ps1)));
                notes.extend(approximations);
                if !zsh.rprompt.is_empty() {
                    notes.push("zsh's right-hand prompt (RPROMPT) is left out: bash has none.".into());
                }
            }
            Err(why) => {
                let theme = if zsh.theme.is_empty() { String::new() } else { format!(" (oh-my-zsh's {} theme)", zsh.theme) };
                notes.push(format!("zsh's prompt{theme} {why}, which bash's prompt cannot do; {keep}."));
            }
        }
    }
    for (name, value) in &zsh.colors {
        lines.push(format!("export {name}={}", quote(value)));
    }
    (lines, notes)
}

/// NSH-49: a zsh `PROMPT` as bash's `PS1`, with what is only close; or why not.
pub fn translate(prompt: &str, prompt_subst: bool) -> Result<(String, Vec<String>), String> {
    let chars: Vec<char> = prompt.chars().collect();
    let mut out = String::new();
    let mut notes = Vec::new();
    let mut i = 0;
    let sgr = |code: &str| format!("\\[\\e[{code}m\\]");
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        match c {
            '$' | '`' if prompt_subst => return Err("runs commands or reads variables as it is drawn".into()),
            '$' => out += "\\044",
            '`' => out += "\\140",
            '\\' => out += "\\\\",
            '%' => {
                let digits: String = chars[i..].iter().take_while(|c| c.is_ascii_digit()).collect();
                i += digits.len();
                let n: Option<u32> = digits.parse().ok();
                let Some(&e) = chars.get(i) else {
                    out.push('%');
                    break;
                };
                i += 1;
                // `{…}` after %F, %K and %D.
                let mut braced = || -> Option<String> {
                    (chars.get(i) == Some(&'{')).then(|| {
                        let end = chars[i..].iter().position(|&c| c == '}').map_or(chars.len(), |p| i + p);
                        let s: String = chars[i + 1..end].iter().collect();
                        i = (end + 1).min(chars.len());
                        s
                    })
                };
                match e {
                    'n' => out += "\\u",
                    'm' => out += "\\h",
                    'M' => out += "\\H",
                    '~' | 'd' | '/' if n == Some(1) => out += "\\W",
                    '~' => {
                        if n.is_some_and(|n| n > 1) {
                            notes.push(format!("%{digits}~ shows the whole path in bash, not the last {digits} parts."));
                        }
                        out += "\\w";
                    }
                    'd' | '/' => out += "${PWD}",
                    'c' | '.' => out += "\\W",
                    '#' => out += "\\$",
                    '?' => out += "$?",
                    'j' => out += "\\j",
                    'h' | '!' => out += "\\!",
                    'l' | 'y' => out += "\\l",
                    'T' => out += "\\A",
                    't' | '@' => out += "\\@",
                    '*' => out += "\\t",
                    'D' => out += &format!("\\D{{{}}}", braced().unwrap_or_else(|| "%y-%m-%d".into())),
                    'w' => out += "\\D{%a %e}",
                    'W' => out += "\\D{%m/%d/%y}",
                    '%' => out.push('%'),
                    ')' => out.push(')'),
                    'F' | 'K' => {
                        let fg = e == 'F';
                        let color = braced().or_else(|| n.map(|n| n.to_string()));
                        let code = match color {
                            None => Ok(if fg { "39".to_string() } else { "49".to_string() }),
                            Some(c) => crate::prompt::color_code(&c, fg),
                        };
                        out += &sgr(&code.map_err(|()| format!("names a colour bash does not know (%{e}{{…}})"))?);
                    }
                    'f' => out += &sgr("39"),
                    'k' => out += &sgr("49"),
                    'B' => out += &sgr("1"),
                    'b' => out += &sgr("22"),
                    'U' => out += &sgr("4"),
                    'u' => out += &sgr("24"),
                    'S' => out += &sgr("7"),
                    's' => out += &sgr("27"),
                    'E' => out += "\\[\\e[K\\]",
                    '{' => {
                        let rest: String = chars[i..].iter().collect();
                        let end = rest.find("%}").ok_or("has a %{ with no %}")?;
                        out += &format!("\\[{}\\]", &rest[..end]);
                        i += rest[..end].chars().count() + 2;
                    }
                    '(' => return Err("has a %(…) condition".into()),
                    other => return Err(format!("uses %{other}")),
                }
            }
            c => out.push(c),
        }
    }
    Ok((out, notes))
}

/// `s` in single quotes for `~/.neoshrc`.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn ask(question: &str) -> String {
    print!("{question}");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    let _ = std::io::stdin().lock().read_line(&mut answer);
    answer.trim().to_lowercase()
}

fn yes_no(question: &str, default: bool) -> bool {
    match ask(question).as_str() {
        "" => default,
        a => a.starts_with('y'),
    }
}

fn find(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| std::env::split_paths(&path).map(|d| d.join(program)).find(|p| p.is_file()))
}

fn tilde(path: &Path) -> String {
    match std::env::var_os("HOME").map(PathBuf::from) {
        Some(home) if path.starts_with(&home) => format!("~/{}", path.strip_prefix(&home).unwrap().display()),
        _ => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NSH-49
    #[test]
    fn a_zsh_prompt_is_written_as_bash_s() {
        let t = |p: &str| translate(p, false).map(|(ps1, _)| ps1);
        assert_eq!(t("%n@%m %~ %# ").unwrap(), r"\u@\h \w \$ ");
        assert_eq!(t("%F{green}%n%f:%1~%(").unwrap_err(), "has a %(…) condition");
        assert_eq!(t("%F{green}%n%f:%1~ ").unwrap(), r"\[\e[32m\]\u\[\e[39m\]:\W ");
        assert_eq!(t("%B%F{208}%c%f%b %K{#ff0000}x%k").unwrap(), r"\[\e[1m\]\[\e[38;5;208m\]\W\[\e[39m\]\[\e[22m\] \[\e[48;2;255;0;0m\]x\[\e[49m\]");
        assert_eq!(t("%D{%H:%M} %* %T %? %% %d $ `x` \\").unwrap(), r"\D{%H:%M} \t \A $? % ${PWD} \044 \140x\140 \\");
        assert_eq!(t("%{\x1b[1m%}>").unwrap(), "\\[\x1b[1m\\]>");
        let (ps1, notes) = translate("%3~ ", false).unwrap();
        assert_eq!(ps1, r"\w ");
        assert_eq!(notes.len(), 1);
        assert_eq!(translate("$(git_prompt_info) %~", true).unwrap_err(), "runs commands or reads variables as it is drawn");
        assert_eq!(t("%v").unwrap_err(), "uses %v");
    }

    // NSH-49
    #[test]
    fn what_zsh_said_becomes_lines_and_notes() {
        let zsh = parse_zsh(&format!("hello from zshrc\n{SENTINEL}\0%n %~ %# \0%T\0\00\00\0off\0/usr/bin:/h/bin\01\0Gxfx\0\0")).unwrap();
        assert_eq!(zsh.prompt, "%n %~ %# ");
        assert_eq!(zsh.path, "/usr/bin:/h/bin");
        assert_eq!(zsh.colors, vec![("CLICOLOR", "1".to_string()), ("LSCOLORS", "Gxfx".to_string())]);
        let (lines, notes) = lines_for(&zsh, false);
        assert_eq!(lines, vec![r"PS1='\u \w \$ '", "export CLICOLOR='1'", "export LSCOLORS='Gxfx'"]);
        assert!(notes[0].contains("RPROMPT"), "{notes:?}");
        let starship = Zsh { starship: true, ..Zsh::default() };
        assert_eq!(lines_for(&starship, true).0, vec![STARSHIP_INIT]);
        assert!(lines_for(&starship, false).0.is_empty());
        let omz = Zsh { prompt: "$(git_prompt_info)%~".into(), prompt_subst: true, theme: "robbyrussell".into(), ..Zsh::default() };
        let (lines, notes) = lines_for(&omz, false);
        assert!(lines.is_empty());
        assert!(notes[0].contains("oh-my-zsh's robbyrussell theme") && notes[0].contains("neosh's own prompt is kept"), "{notes:?}");
    }

    // NSH-50
    #[test]
    fn setup_s_block_is_replaced_and_nothing_else() {
        let lines = vec!["PS1='> '".to_string()];
        assert_eq!(with_block("", &lines), format!("{BEGIN}\nPS1='> '\n{END}\n"));
        let mine = "alias ll='ls -l'\n";
        let once = with_block(mine, &lines);
        assert_eq!(once, format!("alias ll='ls -l'\n\n{BEGIN}\nPS1='> '\n{END}\n"));
        let again = with_block(&once, &["PS1='$ '".to_string()]);
        assert_eq!(again, format!("alias ll='ls -l'\n\n{BEGIN}\nPS1='$ '\n{END}\n"));
        assert_eq!(with_block(&again, &[]), mine);
        assert_eq!(with_block(&format!("{BEGIN}\nx\n{END}\n"), &[]), "");
    }

    // NSH-51
    // NSH-79
    #[test]
    fn the_folders_zsh_adds_are_put_on_path_once() {
        let system: Vec<String> = ["/usr/bin", "/bin"].map(String::from).to_vec();
        let home = Path::new("/h");
        let line = path_line("/usr/bin:/h/.local/bin:/bin:/opt/x y/bin:/h/.local/bin:/gone:rel", &system, Some(home), |d| d != "/gone").unwrap();
        assert_eq!(line, "for d in \"$HOME\"'/.local/bin' '/opt/x y/bin'; do case \":$PATH:\" in *\":$d:\"*) ;; *) PATH=\"$PATH:$d\" ;; esac; done; export PATH");
        assert_eq!(path_line("/usr/bin:/bin", &system, Some(home), |_| true), None);
    }

    #[test]
    fn setup_options() {
        let args = |a: &[&str]| parse(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(args(&["--look", "zsh", "--yes"]), Ok(Options { look: Some(Look::Zsh), yes: true, ..Options::default() }));
        assert_eq!(args(&[]), Ok(Options::default()));
        assert!(args(&["--look", "fish"]).is_err());
        // NSH-75
        let o = args(&["--prompt", "time dir user", "--yes"]).unwrap();
        assert_eq!((o.look, o.parts.as_deref()), (Some(Look::Neosh), Some("user dir time")));
        assert!(args(&["--prompt", "dir sparkles"]).unwrap_err().contains("sparkles is not a part"));
        // NSH-76, NSH-77
        assert_eq!(args(&["--colors", "dir=red"]).unwrap().colors.as_deref(), Some("dir=red"));
        let o = args(&["--preset", "tokyo-night"]).unwrap();
        assert_eq!((o.look, o.preset.as_deref()), (Some(Look::Starship), Some("tokyo-night")));
        assert_eq!(args(&["--look", "starship"]).unwrap().look, Some(Look::Starship));
        assert!(args(&["--what"]).is_err());
    }
}
