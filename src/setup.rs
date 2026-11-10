//! Setup (NSH-47 to NSH-51): the model navi answers with, then neosh's look,
//! in the terminal, written to `~/.neoshrc` between markers as zsh's
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
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub look: Option<Look>,
    pub yes: bool,
    /// Offered as neosh starts, so what is written counts at once.
    pub at_start: bool,
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
                    other => return Err(format!("--look takes zsh or neosh, not {}", other.unwrap_or("nothing"))),
                })
            }
            other => return Err(format!("unknown option {other} for --setup")),
        }
    }
    Ok(options)
}

/// NSH-47: offered when an interactive neosh starts with no `~/.neoshrc`.
pub fn offer(rc: &Path) {
    println!("{}", bold("Welcome to neosh."));
    println!("Start a line with ? to ask navi anything. Setup takes a minute: the model that\nanswers ?, then how the prompt looks.");
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
        eprintln!("neosh: --setup asks its questions on a terminal; without one, give --look zsh|neosh and --yes");
        return 1;
    }
    // NSH-48: the model navi answers with, by navi's own setup.
    if options.look.is_none() {
        println!("\n{}", bold("Step 1 of 2 · The model that answers ?"));
        match find("navi") {
            // navi's setup says itself when it leaves no model answering (SET-29).
            Some(navi) => {
                let _ = Command::new(navi).arg("setup").status();
            }
            None => println!("navi answers ? and is not installed. Install it, then run neosh --setup again:\n  {INSTALL_NAVI}"),
        }
        println!("\n{}", bold("Step 2 of 2 · How the prompt looks"));
        println!("  1  neosh's own  {}", dim("the directory, git branch and a failed status"));
        println!("  2  like my zsh  {}", dim("its prompt and colours"));
    }
    let look = options.look.unwrap_or_else(|| if ask("Choose [1]: ") == "2" { Look::Zsh } else { Look::Neosh });
    // NSH-49
    let lines = match look {
        Look::Neosh => vec![OWN_LOOK.to_string()],
        Look::Zsh => {
            let (lines, notes) = zsh_look();
            for note in notes {
                println!("{}", dim(&format!("· {note}")));
            }
            lines
        }
    };
    // NSH-50: shown, then written only on yes; neosh's own look, which sets
    // nothing, is written without asking unless it takes an earlier look out.
    let before = std::fs::read_to_string(rc).unwrap_or_default();
    let after = with_block(&before, &lines);
    let earlier = block(&before).is_some_and(|b| b.iter().any(|l| l.trim() != OWN_LOOK && !l.trim().is_empty()));
    if after == before {
        println!("{}", dim(&format!("· {} already has this look.", tilde(rc))));
    } else {
        let asking = !options.yes && (look == Look::Zsh || earlier);
        if look == Look::Neosh && earlier {
            println!("neosh's own look: the lines an earlier setup wrote in {} go.", tilde(rc));
        } else if look == Look::Zsh {
            println!("This goes in {}:\n", tilde(rc));
            for line in &lines {
                println!("  {line}");
            }
            println!();
        }
        if asking && !yes_no("Write it? [Y/n] ", true) {
            println!("Nothing written.");
            return 0;
        }
        if let Err(e) = std::fs::write(rc, after) {
            eprintln!("neosh: could not write {}: {e}", rc.display());
            return 1;
        }
        println!("{}", dim(&format!("· Saved in {}.", tilde(rc))));
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
}

const SENTINEL: &str = "neosh-setup-look";
const COLOR_VARS: [&str; 3] = ["CLICOLOR", "LSCOLORS", "LS_COLORS"];

/// NSH-49: zsh's prompt and colours, asked of an interactive zsh with no terminal.
fn ask_zsh() -> Result<Zsh, String> {
    let zsh = find("zsh").ok_or("zsh is not installed, so there is no zsh look to copy")?;
    let script = format!(
        "print -rN -- {SENTINEL} \"$PROMPT\" \"$RPROMPT\" \"${{ZSH_THEME-}}\" \"${{+functions[starship_precmd]}}\" \"${{+functions[p10k]}}\" \"${{options[promptsubst]}}\" {}",
        COLOR_VARS.iter().map(|v| format!("\"${{{v}-}}\"")).collect::<Vec<_>>().join(" ")
    );
    let mut child = Command::new(zsh)
        .args(["-i", "-c", &script])
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
        colors: COLOR_VARS.iter().enumerate().map(|(i, v)| (*v, field(6 + i))).filter(|(_, value)| !value.is_empty()).collect(),
    })
}

/// NSH-49: the lines for `~/.neoshrc` and what to say about them.
fn zsh_look() -> (Vec<String>, Vec<String>) {
    match ask_zsh() {
        Ok(zsh) => lines_for(&zsh, find("starship").is_some()),
        Err(why) => (Vec::new(), vec![format!("{why}; neosh's own look is kept.")]),
    }
}

/// The lines and notes for what zsh said; `starship` is whether neosh can run it.
pub fn lines_for(zsh: &Zsh, starship: bool) -> (Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut notes = Vec::new();
    let keep = "neosh's own prompt is kept, which shows the git branch and a failed status";
    if zsh.starship {
        if starship {
            lines.push("eval \"$(starship init bash)\"".to_string());
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
                            Some(c) => color_code(&c, fg),
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

/// An SGR code for a zsh colour: a name, 0 to 255, or #rrggbb.
fn color_code(color: &str, fg: bool) -> Result<String, ()> {
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
        let zsh = parse_zsh(&format!("hello from zshrc\n{SENTINEL}\0%n %~ %# \0%T\0\00\00\0off\01\0Gxfx\0\0")).unwrap();
        assert_eq!(zsh.prompt, "%n %~ %# ");
        assert_eq!(zsh.colors, vec![("CLICOLOR", "1".to_string()), ("LSCOLORS", "Gxfx".to_string())]);
        let (lines, notes) = lines_for(&zsh, false);
        assert_eq!(lines, vec![r"PS1='\u \w \$ '", "export CLICOLOR='1'", "export LSCOLORS='Gxfx'"]);
        assert!(notes[0].contains("RPROMPT"), "{notes:?}");
        let starship = Zsh { starship: true, ..Zsh::default() };
        assert_eq!(lines_for(&starship, true).0, vec!["eval \"$(starship init bash)\""]);
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
    #[test]
    fn setup_options() {
        let args = |a: &[&str]| parse(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(args(&["--look", "zsh", "--yes"]), Ok(Options { look: Some(Look::Zsh), yes: true, ..Options::default() }));
        assert_eq!(args(&[]), Ok(Options::default()));
        assert!(args(&["--look", "fish"]).is_err());
        assert!(args(&["--what"]).is_err());
    }
}
