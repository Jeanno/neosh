//! Setup (NSH-47 to NSH-51): how the prompt looks, plugins, the model navi
//! answers with, and the login shell, in the terminal, written to
//! `~/.neoshrc` between markers.

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const BEGIN: &str = "# >>> neosh setup >>>";
pub const END: &str = "# <<< neosh setup <<<";
/// What "never" leaves in `~/.neoshrc`, so neosh does not offer again.
const NEVER: &str = "# neosh: setup declined; run `neosh --setup` any time.\n";
const INSTALL_NAVI: &str = "curl -fsSL https://raw.githubusercontent.com/Jeanno/neoai/master/neonavi/install.sh | sh";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Neosh,
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
                    Some("neosh") => Look::Neosh,
                    Some("starship") => Look::Starship,
                    other => return Err(format!("--look takes starship or neosh, not {}", other.unwrap_or("nothing"))),
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
        eprintln!("neosh: --setup asks its questions on a terminal; without one, give --look starship|neosh (or --prompt '<parts>') and --yes");
        return 1;
    }
    // Every step is asked unless the look was given (NSH-51).
    let steps = options.look.is_none();
    let before = std::fs::read_to_string(rc).unwrap_or_default();
    let total = 4;
    let step = std::cell::Cell::new(0);
    let head = |title: &str| {
        step.set(step.get() + 1);
        println!("\n{}", bold(&format!("Step {} of {total} · {title}", step.get())));
    };
    // NSH-49: the look, starship's the default (NSH-77).
    if steps {
        head("How the prompt looks");
        println!("  1  starship     {}", dim("themes and icons, drawn by starship"));
        println!("  2  neosh's own  {}", dim("the directory, git branch and a failed status"));
        println!("  3  my own       {}", dim("neosh's, with the parts and colours you pick"));
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
                    // NSH-84: the font its icons are drawn from.
                    nerd_font(preset.as_deref());
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
                    head("Plugins");
                }
                println!("{}", dim(&format!("· {} names its own plugins; they stay as they are.", tilde(rc))));
            }
            None
        }
        Some(names) => Some(names.clone()),
        None if steps => {
            head("Plugins");
            Some(pick_plugins(earlier_plugins(&before)))
        }
        None => earlier_plugins(&before),
    };
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
    };
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
        head("The model that answers ?");
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
        head("Your login shell");
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
        tell_apps(&me_str);
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
        println!("{}", dim("· neosh is your login shell."));
        tell_apps(&me_str);
    } else {
        println!("chsh did not change it. To do it yourself:\n  chsh -s {me_str}");
    }
}

/// NSH-80: apps opened from now on start neosh: on macOS they are handed
/// `SHELL` as the login shell was at login, so launchd is told the new one.
fn tell_apps(me: &str) {
    let told = cfg!(target_os = "macos")
        && find("launchctl").is_some_and(|l| Command::new(l).args(["setenv", "SHELL", me]).status().is_ok_and(|s| s.success()));
    if told {
        println!("{}", dim("· New Terminal windows start neosh. An app already open (Ghostty, iTerm) does once you quit it (⌘Q) and open it again."));
    } else {
        println!("{}", dim("· New terminal windows start neosh; apps already open may keep their shell until you log out and back in."));
    }
}

/// NSH-84: starship's looks that draw no icons from a Nerd Font.
const PLAIN_PRESETS: [&str; 3] = ["plain-text-symbols", "no-nerd-font", "pure-preset"];
const NERD_FONT_CASK: &str = "font-jetbrains-mono-nerd-font";
/// The font Terminal is set to: its PostScript name, and how it is said.
const NERD_FONT: (&str, &str) = ("JetBrainsMonoNFM-Regular", "JetBrainsMono Nerd Font Mono");

/// NSH-84: whether a Nerd Font is installed, by its files' names.
fn has_nerd_font(home: Option<&Path>) -> bool {
    let mut dirs: Vec<PathBuf> = vec!["/Library/Fonts".into(), "/usr/share/fonts".into(), "/usr/local/share/fonts".into()];
    if let Some(h) = home {
        dirs.extend([h.join("Library/Fonts"), h.join(".local/share/fonts"), h.join(".fonts")]);
    }
    let named = |p: &Path| p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains("nerd"));
    dirs.iter().any(|d| {
        std::fs::read_dir(d).into_iter().flatten().flatten().any(|e| {
            let p = e.path();
            named(&p) || (p.is_dir() && std::fs::read_dir(&p).into_iter().flatten().flatten().any(|f| named(&f.path())))
        })
    })
}

/// NSH-84: a starship look with icons needs a Nerd Font in the terminal:
/// installed (by Homebrew) when there is none, and, asked, set as
/// Terminal's font; Ghostty draws the icons itself.
fn nerd_font(preset: Option<&str>) {
    if preset.is_some_and(|p| PLAIN_PRESETS.contains(&p)) {
        return;
    }
    let app = std::env::var("TERM_PROGRAM").unwrap_or_default();
    if app == "ghostty" {
        println!("{}", dim("· Ghostty draws the icons itself; no font to install."));
        return;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let (postscript, said) = NERD_FONT;
    if !has_nerd_font(home.as_deref()) {
        println!("\nThis look draws icons from a Nerd Font, and none is installed; installing {said}…");
        let installed = find("brew").is_some_and(|brew| {
            println!("{}", dim(&format!("· brew install --cask {NERD_FONT_CASK}")));
            Command::new(brew).args(["install", "--cask", NERD_FONT_CASK]).status().is_ok_and(|s| s.success())
        });
        if !installed {
            println!("Install a Nerd Font from https://www.nerdfonts.com and choose it in your terminal's settings,");
            println!("{}", dim("or pick plain-text-symbols or no-nerd-font: neosh --setup"));
            return;
        }
    }
    if app == "Apple_Terminal" {
        let set = format!("tell application \"Terminal\" to set font name of default settings to \"{postscript}\"");
        if yes_no(&format!("Use {said} in Terminal? [Y/n] "), true)
            && find("osascript").is_some_and(|o| Command::new(o).args(["-e", &set]).stdout(Stdio::null()).status().is_ok_and(|s| s.success()))
        {
            println!("{}", dim("· Terminal's default profile uses it; new windows show the icons."));
        } else {
            println!("{}", dim(&format!("· Choose {said} in Terminal › Settings › Profiles › Text › Font.")));
        }
    } else {
        println!("{}", dim(&format!("· Choose {said} in your terminal's font settings to see the icons.")));
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

    #[test]
    fn setup_options() {
        let args = |a: &[&str]| parse(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(args(&["--look", "neosh", "--yes"]), Ok(Options { look: Some(Look::Neosh), yes: true, ..Options::default() }));
        assert!(args(&["--look", "zsh"]).is_err());
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
