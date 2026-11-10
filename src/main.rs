//! neosh: a bash-compatible shell with navi inside (spec.md).
//!
//! The interpreter is brush's (brush-core, MIT, github.com/reubeno/brush);
//! starting it follows brush-shell's entry. What neosh adds is the line
//! reader around brush's own, which sends lines for navi to navi (NSH-4).

mod away;
mod bang;
mod cd;
mod errors;
mod fg;
mod histctl;
mod histsize;
mod navi;
mod prompt;
mod record;
mod setup;
mod term;

use std::io::{IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use brush_builtins::ShellBuilderExt as _;
use brush_interactive::{InputBackend, InteractivePrompt, ReadResult, ShellError, ShellRef};

const USAGE: &str = "\
usage: neosh [-l] [--norc] [-c '<commands>' [name [args…]] | <script> [args…]]
       neosh --setup [--look zsh|neosh] [--yes]

neosh is a bash-compatible shell with navi inside: start a line with ? to
ask navi, or write a sentence that is not a command.

  -c COMMANDS  run the commands and exit with the last one's status
  -l, --login  read ~/.profile first, as a login shell does
  --norc       do not read ~/.neoshrc
  --setup      choose navi's model and neosh's look (zsh's or neosh's own)
  --version    say which neosh this is";

/// What the command line asked for (NSH-1).
#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    command: Option<String>,
    login: bool,
    norc: bool,
    /// The script and its arguments, or with -c the name and arguments.
    rest: Vec<String>,
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut args = Args { login: argv.first().is_some_and(|a| a.starts_with('-')), ..Default::default() };
    let mut i = 1;
    while i < argv.len() {
        let a = argv[i].as_str();
        match a {
            "-c" => {
                i += 1;
                args.command = Some(argv.get(i).ok_or("-c needs the commands to run")?.clone());
            }
            "-l" | "--login" => args.login = true,
            "--norc" => args.norc = true,
            // Everything after is the script's, or -c's name and arguments.
            "--" => {
                args.rest.extend(argv[i + 1..].iter().cloned());
                break;
            }
            _ if a.starts_with('-') && a.len() > 1 => return Err(format!("unknown option {a}")),
            _ => {
                args.rest.extend(argv[i..].iter().cloned());
                break;
            }
        }
        i += 1;
    }
    Ok(args)
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    if argv.iter().skip(1).any(|a| a == "--version") {
        println!("neosh {} (brush-core {})", env!("CARGO_PKG_VERSION"), "0.5");
        return;
    }
    if argv.iter().skip(1).any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return;
    }
    let rc = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".neoshrc"));
    // NSH-47, NSH-51: setup, then nothing else.
    if argv.get(1).is_some_and(|a| a == "--setup") {
        let Some(rc) = rc else {
            eprintln!("neosh: --setup writes ~/.neoshrc, and HOME is not set");
            std::process::exit(1);
        };
        let code = match setup::parse(&argv[2..]) {
            Ok(options) => setup::run(&options, &rc),
            Err(e) => {
                eprintln!("neosh: {e}\n{USAGE}");
                2
            }
        };
        std::process::exit(code);
    }
    let args = match parse_args(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("neosh: {e}\n{USAGE}");
            std::process::exit(2);
        }
    };
    // NSH-34: an interactive neosh runs inside its recorder.
    if args.command.is_none() && args.rest.is_empty() {
        if let Some(code) = record::wrap(&argv) {
            std::process::exit(code);
        }
    }
    if !record::recorded() {
        // SAFETY: before any thread is started.
        unsafe { std::env::remove_var(record::INDEX) };
    }
    // NSH-47: offered once a terminal session starts with no ~/.neoshrc.
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if let Some(rc) = rc.filter(|rc| terminal && args.command.is_none() && args.rest.is_empty() && !args.norc && !rc.exists()) {
        setup::offer(&rc);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("a runtime");
    let code = runtime.block_on(run(args)).unwrap_or_else(|e| {
        eprintln!("neosh: {e}");
        1
    });
    std::process::exit(i32::from(code));
}

async fn run(args: Args) -> Result<u8, ShellError> {
    let interactive = args.command.is_none() && args.rest.is_empty() && std::io::stdin().is_terminal();
    let from_stdin = args.command.is_none() && args.rest.is_empty();
    let mut builder = brush_core::Shell::builder_with_extensions::<errors::Extensions>()
        .login(args.login)
        .interactive(interactive)
        .command_string_mode(args.command.is_some())
        .profile(brush_core::ProfileLoadBehavior::Skip)
        .rc(brush_core::RcLoadBehavior::Skip)
        .maybe_shell_args(args.command.as_ref().map(|_| args.rest.iter().skip(1).cloned().collect()))
        .read_commands_from_stdin(from_stdin)
        .maybe_shell_name(Some(if args.command.is_some() { args.rest.first().cloned().unwrap_or_else(|| "neosh".into()) } else { "neosh".into() }))
        .shell_product_display_str(format!("neosh {}", env!("CARGO_PKG_VERSION")))
        .shell_version(env!("CARGO_PKG_VERSION").to_string())
        .default_builtins(brush_builtins::BuiltinSet::BashMode);
    if let Some(histfile) = history_file(std::env::var_os("HISTFILE"), std::env::var_os("HOME")) {
        builder = builder.var("HISTFILE", brush_core::ShellVariable::new(histfile.to_string_lossy().into_owned()));
    }
    let mut shell = builder.build().await?;
    // NSH-25: CDPATH, which brush's `cd` does not look in.
    cd::install(&mut shell);
    // NSH-63: a job `fg` brings back and that ends there is not reported Done.
    fg::install(&mut shell);
    let shell: ShellRef<errors::Extensions> = std::sync::Arc::new(tokio::sync::Mutex::new(shell));

    // NSH-1: ~/.profile for a login shell, ~/.neoshrc for an interactive one.
    let rc = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".neoshrc"));
    let rc = match rc {
        Some(path) if interactive && !args.norc && path.exists() => brush_core::RcLoadBehavior::LoadCustom(path),
        _ => brush_core::RcLoadBehavior::Skip,
    };
    let profile = if args.login { brush_core::ProfileLoadBehavior::LoadDefault } else { brush_core::ProfileLoadBehavior::Skip };
    // NSH-5: ^C is for what runs, navi included, never the shell itself. A
    // handler, unlike an ignored signal, is not inherited across exec, so
    // children still get ^C as they expect.
    let _interrupts = if interactive { Some(tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?) } else { None };
    // NSH-10: the directory and `$`, unless a PS1 came with the environment; the rc can set its own.
    if interactive && std::env::var_os("PS1").is_none() {
        shell.lock().await.env_mut().set_global("PS1", brush_core::ShellVariable::new(prompt::DEFAULT_PS1))?;
    }
    // NSH-27: as long as bash's history, unless the environment says otherwise.
    if interactive {
        let mut s = shell.lock().await;
        for name in ["HISTSIZE", "HISTFILESIZE"] {
            if std::env::var_os(name).is_none() {
                s.env_mut().set_global(name, brush_core::ShellVariable::new(histsize::DEFAULT))?;
            }
        }
    }
    shell.lock().await.load_config(&profile, &rc).await?;

    if let Some(command) = args.command {
        shell.lock().await.run_dash_c_command(command).await?;
    } else if let Some(script) = args.rest.first() {
        // NSH-2: said as bash says it, not as brush's "failed to source file".
        if !std::path::Path::new(script).exists() {
            eprintln!("neosh: {script}: No such file or directory");
            return Ok(127);
        }
        shell.lock().await.run_script(std::path::Path::new(script), args.rest.iter().skip(1)).await?;
    } else if interactive {
        // NSH-18: VS Code's command marks, where brush finds them supported.
        let ui = brush_interactive::UIOptions { terminal_shell_integration: true, ..Default::default() };
        brush_interactive::set_completes(completes);
        let mut input = WithNavi::new(brush_interactive::ReedlineInputBackend::new(&ui, &shell)?);
        brush_interactive::InteractiveShell::new(&shell, &mut input, &(&ui).into())?.run_interactively().await?;
        // NSH-27: the file as long as HISTFILESIZE says, now history is written.
        let s = shell.lock().await;
        if let (Some(file), Some(keep)) = (s.env_str("HISTFILE"), histsize::limit(s.env_str("HISTFILESIZE").as_deref())) {
            let _ = histsize::trim_file(Path::new(file.as_ref()), keep);
        }
    } else {
        let ui = brush_interactive::UIOptions::default();
        let mut input = brush_interactive::MinimalInputBackend;
        brush_interactive::InteractiveShell::new(&shell, &mut input, &(&ui).into())?.run_interactively().await?;
    }
    Ok(shell.lock().await.last_exit_status())
}

/// brush's line reader, with lines for navi taken to navi (NSH-4 to NSH-8).
struct WithNavi<IB: InputBackend> {
    inner: IB,
    recent: navi::Recent,
    /// The line handed to the shell last, and where, to learn how it ended (NSH-6).
    handed: Option<(String, PathBuf, Instant)>,
    /// How long the last command took, for the prompt (NSH-15).
    took: Option<Duration>,
    /// The line run for `… | ?`, and as typed, to put right in history (NSH-16).
    piped: Option<(String, String)>,
    /// Whether NSH-19's note has been said this session.
    noted: bool,
    /// What navi suggested running, for the next prompt (NSH-11).
    suggested: Option<String>,
    /// What was put at this prompt for Enter to run, so navi mode runs it (NSH-57).
    offered: Option<String>,
    /// A line not found as a command, to offer navi (NSH-23).
    missed: Option<String>,
    /// A line run in place of the one typed, and that one, for history (NSH-29).
    retyped: Option<(String, String)>,
    /// NSH-32: the last `:s` or `^old^new`, for `:&`.
    substituted: bang::Memory,
    /// The line brush adds to history as it runs it, and whether bash would keep it (NSH-26).
    settle: Option<(String, bool)>,
    session: String,
    /// NSH-36: this terminal's recorder, when there is one.
    marks: Option<Marks>,
    /// NSH-45: how many times `? to ask why` was said.
    hinted: u32,
    /// NSH-34: whether `?` has said output is not kept.
    told_unrecorded: bool,
    /// NSH-62: navi, sent to the background with ^Z, working on.
    away: Option<away::Navi>,
}

/// What neosh needs to mark commands for the recorder (NSH-36, NSH-40).
struct Marks {
    key: String,
    dir: PathBuf,
    n: u32,
    /// The command being run, by id.
    open: Option<String>,
    /// Stopped jobs and the command each is.
    stopped: Vec<(usize, String)>,
}

impl Marks {
    fn from_env() -> Option<Marks> {
        if !record::recorded() {
            return None;
        }
        Some(Marks { key: std::env::var(record::KEY).ok()?, dir: PathBuf::from(std::env::var_os(record::DIR)?), n: 0, open: None, stopped: Vec::new() })
    }
}

impl<IB: InputBackend> WithNavi<IB> {
    fn new(inner: IB) -> Self {
        let started = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        Self { inner, recent: navi::Recent::default(), handed: None, piped: None, noted: false, took: None, suggested: None, offered: None, missed: None, retyped: None, substituted: bang::Memory::default(), settle: None, session: format!("neosh-{started}-{}", std::process::id()), marks: Marks::from_env(), hinted: 0, told_unrecorded: false, away: None }
    }
}

// The session's files go with it.
impl<IB: InputBackend> Drop for WithNavi<IB> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(navi::seen_file(&self.session));
        let _ = std::fs::remove_file(navi::command_file(&self.session));
    }
}

impl<IB: InputBackend> WithNavi<IB> {
    /// NSH-62: navi gave the screen back: done, or sent away with ^Z.
    fn answered(&mut self, shell: &ShellRef<impl brush_core::ShellExtensions>, n: away::Navi, how: away::Ended) {
        let mut guard = shell.try_lock().ok();
        match how {
            away::Ended::Done(status) => {
                self.suggested = navi::take_suggestion(&n.handed);
                if let Some(s) = guard.as_mut() {
                    s.set_last_exit_status(status);
                }
            }
            away::Ended::Away => {
                navi_mode(guard.as_deref_mut(), false);
                eprintln!("\r\n{}", dim_if(guard.as_deref(), "· navi works on in the background: ? brings it back"));
                if let Some(s) = guard.as_mut() {
                    s.set_last_exit_status(0);
                }
                self.away = Some(n);
            }
        }
    }
}

/// NSH-62: what a question, or `| ?`, says while navi is away.
fn still_away<SE: brush_core::ShellExtensions>(away: Option<&away::Navi>, shell: Option<&brush_core::Shell<SE>>) {
    if let Some(n) = away {
        eprintln!("{}", dim_if(shell, &format!("· navi is still on ?{}: ? brings it back, and then you can ask", n.ask)));
    }
}

/// Dim, unless `NO_COLOR` says not to.
fn dim_if<SE: brush_core::ShellExtensions>(shell: Option<&brush_core::Shell<SE>>, text: &str) -> String {
    match shell.is_some_and(|s| s.env_str("NO_COLOR").is_none()) {
        true => format!("\x1b[2m{text}\x1b[0m"),
        false => text.to_string(),
    }
}

impl<IB: InputBackend> InputBackend for WithNavi<IB> {
    fn read_line(&mut self, shell: &ShellRef<impl brush_core::ShellExtensions>, prompt: InteractivePrompt) -> Result<ReadResult, ShellError> {
        loop {
            // The shell is not running anything while it waits for a line.
            let mut guard = shell.try_lock().ok();
            // Only the command just run: an empty line or a question shows none.
            self.took = None;
            // NSH-16: navi has answered; history keeps the line as typed.
            if let Some((ran, typed)) = self.piped.take() {
                if let Some(history) = guard.as_mut().and_then(|s| s.history_mut()) {
                    let found = history.iter().filter(|i| i.command_line == ran).last().cloned();
                    if let Some(item) = found {
                        let _ = history.update_by_id(item.id, brush_core::history::Item { command_line: typed.trim().to_string(), ..item });
                    }
                }
                self.recent.clear();
                self.suggested = navi::take_suggestion(&navi::command_file(&self.session));
            }
            // NSH-29: history keeps what was typed, not the `cd` run for it.
            if let Some((ran, typed)) = self.retyped.take() {
                if let Some(history) = guard.as_mut().and_then(|s| s.history_mut()) {
                    let found = history.iter().filter(|i| i.command_line == ran).last().cloned();
                    if let Some(item) = found {
                        let _ = history.update_by_id(item.id, brush_core::history::Item { command_line: typed, ..item });
                    }
                }
            }
            // NSH-31: the terminal's size, for scripts, while checkwinsize is on.
            if let Some(s) = guard.as_mut().filter(|s| s.options().check_window_size_after_external_commands) {
                if let Ok((columns, lines)) = crossterm::terminal::size() {
                    let _ = s.env_mut().set_global("COLUMNS", brush_core::ShellVariable::new(columns.to_string()));
                    let _ = s.env_mut().set_global("LINES", brush_core::ShellVariable::new(lines.to_string()));
                }
            }
            // NSH-27: no more than HISTSIZE lines.
            if let Some(keep) = guard.as_ref().and_then(|s| histsize::limit(s.env_str("HISTSIZE").as_deref())) {
                if let Some(history) = guard.as_mut().and_then(|s| s.history_mut()) {
                    histsize::trim_list(history, keep);
                }
            }
            // NSH-26: brush keeps every line; drop what bash would not have kept.
            if let Some((line, keep)) = self.settle.take() {
                let erase = guard.as_ref().is_some_and(|s| histctl::erases(s.env_str("HISTCONTROL").as_deref()));
                if let Some(history) = guard.as_mut().and_then(|s| s.history_mut()) {
                    settle(history, &line, keep, erase);
                }
            }
            if let (Some((line, dir, at)), Some(s)) = (self.handed.take(), guard.as_mut()) {
                let took = at.elapsed();
                self.took = Some(took);
                let status = s.last_exit_status();
                // NSH-40: a command ^Z stopped is the newest stopped job.
                let stopped = (status > 128).then(|| s.jobs().jobs.iter().filter(|j| matches!(j.state, brush_core::jobs::JobState::Stopped)).map(|j| j.id).max()).flatten();
                let mut output = None;
                if let Some(marks) = self.marks.as_mut() {
                    if let Some(id) = marks.open.take() {
                        let how = match stopped {
                            Some(job) => {
                                marks.stopped.retain(|(j, _)| *j != job);
                                marks.stopped.push((job, id.clone()));
                                format!("stopped %{job}")
                            }
                            None => format!("exit {status}"),
                        };
                        print!("{}", record::end_mark(&marks.key, &id, &how));
                        let _ = std::io::stdout().flush();
                        output = Some(record::file(&marks.dir, &id));
                    }
                }
                // NSH-45: a failure, not ^C or ^Z, says navi can be asked.
                let hint = s.env_str("NEOSH_HINT").map(|h| h.into_owned());
                let shown = match hint.as_deref() {
                    Some("off") => false,
                    Some("always") => true,
                    _ => self.hinted < 3,
                };
                if status != 0 && status != 130 && stopped.is_none() && shown {
                    self.hinted += 1;
                    let dim = s.env_str("NO_COLOR").is_none();
                    eprintln!("{}", if dim { "\x1b[2m? to ask why\x1b[0m" } else { "? to ask why" });
                }
                // NSH-23: a sentence typed as a command, offered to navi.
                let ask = navi::Ask::from_setting(s.env_str("NEOSH_ASK").as_deref());
                if status == 127 && ask == navi::Ask::Marked && self.suggested.is_none() {
                    self.missed = Some(line.clone());
                }
                self.recent.push(navi::Ran { line, dir, status, took, output });
            }
            // NSH-62: navi, away, done or waiting on you: said in a line; navi notifies too.
            if let Some(n) = self.away.as_mut() {
                if n.finished().is_some() {
                    if !n.told_done {
                        n.told_done = true;
                        eprintln!("{}", dim_if(guard.as_deref(), &format!("· navi is done with ?{}: ? shows its answer", n.ask)));
                    }
                } else if n.asking() && !n.told {
                    n.told = true;
                    eprintln!("{}", dim_if(guard.as_deref(), &format!("· navi waits for your answer about ?{}: ? to answer", n.ask)));
                }
            }
            // NSH-33: zsh's precmd, the status left for the prompt; a PS1 it set counts.
            let mut composed = prompt.prompt.clone();
            if let Some(s) = guard.as_mut() {
                if hooks(s, "precmd", &[]) && s.env_str("PS1").as_deref() != Some(prompt::DEFAULT_PS1) {
                    if let Ok(p) = tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(s.compose_prompt())) {
                        composed = p;
                    }
                }
            }
            if let Some(line) = self.missed.take() {
                if navi::sentence(&line, |word| guard.as_mut().is_none_or(|s| is_command(s, word))) {
                    self.suggested = Some(format!("?{line}"));
                }
            }
            // NSH-12: drawn by neosh while PS1 is still its own.
            let ours = guard.as_ref().filter(|s| s.env_str("PS1").as_deref() == Some(prompt::DEFAULT_PS1)).map(|s| {
                let dir = s.working_dir().to_path_buf();
                let home = std::env::var_os("HOME").map(PathBuf::from);
                let branch = prompt::branch(&dir);
                prompt::render(&prompt::Parts {
                    dir: &dir,
                    home: home.as_deref(),
                    // NSH-22: asked only inside a repository.
                    changed: branch.is_some()
                        && s.find_first_executable_in_path("git").is_some_and(|git| prompt::changed(&git, &dir, std::time::Duration::from_millis(200))),
                    branch,
                    status: s.last_exit_status(),
                    took: self.took,
                    root: s.env_str("EUID").as_deref() == Some("0"),
                    color: s.env_str("NO_COLOR").is_none(),
                })
            });
            // NSH-18: the title and directory, for the window and new tabs.
            let told = guard.as_ref().filter(|s| term::wanted(s.env_str("TERM").as_deref())).map(|s| {
                let dir = s.working_dir().to_path_buf();
                let home = std::env::var_os("HOME").map(PathBuf::from);
                let host = s.env_str("HOSTNAME").map(|h| h.into_owned()).unwrap_or_default();
                format!("{}{}", term::title(&prompt::tilde(&dir, home.as_deref())), term::directory(&host, &dir))
            });
            let again = InteractivePrompt { prompt: ours.unwrap_or(composed), alt_side_prompt: prompt.alt_side_prompt.clone(), continuation_prompt: prompt.continuation_prompt.clone() };
            let titled = guard.as_ref().is_some_and(|s| term::wanted(s.env_str("TERM").as_deref()));
            drop(guard.take());
            if let Some(told) = told {
                print!("{told}");
                let _ = std::io::stdout().flush();
            }
            // NSH-11: ready to run with Enter, change, or clear with ^C.
            self.offered = self.suggested.take();
            if let Some(command) = self.offered.clone() {
                let end = command.len();
                self.inner.set_read_buffer(command, end);
            }
            // NSH-54: in navi mode the prompt ends in `?`.
            let in_mode = NAVI_MODE.load(Ordering::Relaxed);
            let again = if in_mode { InteractivePrompt { prompt: navi_prompt(&again.prompt), ..again } } else { again };
            let read = self.inner.read_line(shell, again)?;
            let mut guard = shell.try_lock().ok();
            let mut line = match read {
                ReadResult::Input(line) => line,
                // NSH-54: Esc, or ^D, leaves navi mode, and the shell stays.
                ReadResult::BoundCommand(command) if command.contains(LEAVE_NAVI_MODE) => {
                    navi_mode(guard.as_deref_mut(), false);
                    continue;
                }
                ReadResult::Eof if in_mode => {
                    navi_mode(guard.as_deref_mut(), false);
                    continue;
                }
                read => return Ok(read),
            };
            // NSH-57: the command navi offered, run as offered, is for the shell.
            let ran_offered = in_mode && self.offered.take().is_some_and(|o| o.trim() == line.trim() && !o.trim_start().starts_with('?'));
            // NSH-54: in navi mode a line is a question, as if typed after `?`.
            if in_mode && !ran_offered {
                let typed = line.trim().to_string();
                if typed.is_empty() {
                    continue;
                }
                if typed == "exit" {
                    navi_mode(guard.as_deref_mut(), false);
                    continue;
                }
                line = format!("?{typed}\n");
            }
            // NSH-24: `!!` and the rest, before anything reads the line.
            let history = guard.as_ref().filter(|s| s.options().enable_bang_style_history_substitution).map(|s| -> Vec<String> { s.history().map(|h| h.iter().map(|i| i.command_line.clone()).collect()).unwrap_or_default() });
            if let Some(history) = history {
                match bang::expand_with(line.trim_end_matches('\n'), &history, &mut self.substituted) {
                    // NSH-32: `:p` prints it and keeps it in history, and runs nothing.
                    Ok(Some(bang::Expanded { line: expanded, print: true })) => {
                        println!("{expanded}");
                        if let Some(h) = guard.as_mut().and_then(|s| s.history_mut()) {
                            let _ = h.add(brush_core::history::Item::new(expanded));
                        }
                        continue;
                    }
                    Ok(Some(bang::Expanded { line: expanded, .. })) => {
                        println!("{expanded}");
                        line = if line.ends_with('\n') { format!("{expanded}\n") } else { expanded };
                    }
                    Ok(None) => {}
                    Err(said) => {
                        eprintln!("neosh: {said}");
                        continue;
                    }
                }
            }
            // NSH-17: read each line, so `NEOSH_ASK=marked` at the prompt counts at once.
            let ask = navi::Ask::from_setting(guard.as_ref().and_then(|s| s.env_str("NEOSH_ASK")).as_deref());
            let dir = guard.as_ref().map(|s| s.working_dir().to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
            // NSH-26: decided on the line as typed, its leading space and all.
            let mut keep = guard.as_ref().is_none_or(|s| {
                let previous = s.history().and_then(|h| h.iter().last().map(|i| i.command_line.clone()));
                histctl::keeps(line.trim_end_matches('\n'), previous.as_deref(), s.env_str("HISTCONTROL").as_deref(), s.env_str("HISTIGNORE").as_deref())
            });
            // NSH-27: room made for it, so history never holds more than HISTSIZE.
            match guard.as_ref().and_then(|s| histsize::limit(s.env_str("HISTSIZE").as_deref())) {
                Some(0) => keep = false,
                Some(size) if keep => {
                    if let Some(history) = guard.as_mut().and_then(|s| s.history_mut()) {
                        histsize::trim_list(history, size - 1);
                    }
                }
                _ => {}
            }
            let what = match in_mode && !ran_offered {
                true => navi::Line::Navi(line.trim()[1..].trim().to_string()),
                false => navi::classify_in(&line, ask, |word| guard.as_mut().is_none_or(|s| is_command(s, word)), |word| navi::globs_a_file(word, &dir)),
            };
            // NSH-62: navi away and still working, not done with an answer unseen.
            let busy = self.away.as_mut().is_some_and(|n| n.finished().is_none());
            match what {
                navi::Line::Shell => {
                    if !line.trim().is_empty() {
                        // NSH-33: zsh's preexec, told the line.
                        if let Some(s) = guard.as_mut() {
                            let typed = line.trim_end_matches('\n');
                            hooks(s, "preexec", &[typed, typed, typed]);
                        }
                        // NSH-18: the command, while it runs.
                        if titled {
                            print!("{}", term::title(line.trim()));
                            let _ = std::io::stdout().flush();
                        }
                        self.handed = Some((line.trim().to_string(), dir.clone(), Instant::now()));
                        self.settle = Some((line.trim().to_string(), keep));
                        // NSH-36, NSH-43: marked, unless typed with a space first or recording is off.
                        let recording = guard.as_ref().is_none_or(|s| s.env_str("NEOSH_RECORD").as_deref() != Some("off"));
                        if let (Some(marks), true) = (self.marks.as_mut(), recording && !line.starts_with(' ')) {
                            // NSH-40: `fg` writes on in the file of the command it brings back.
                            let resumes = guard.as_mut().and_then(|s| brought_back(s, line.trim())).and_then(|job| marks.stopped.iter().find(|(j, _)| *j == job).map(|(_, id)| id.clone()));
                            marks.n += 1;
                            let id = format!("{}-{}", std::process::id(), marks.n);
                            print!("{}", record::start_mark(&marks.key, &id, &dir.to_string_lossy(), line.trim(), resumes.as_deref()));
                            let _ = std::io::stdout().flush();
                            marks.open = Some(resumes.unwrap_or(id));
                        }
                        // NSH-29: a directory's name alone goes there.
                        let home = guard.as_ref().and_then(|s| s.env_str("HOME").map(|h| PathBuf::from(h.as_ref())));
                        let auto = guard.as_mut().filter(|s| s.options().auto_cd).is_some_and(|s| autocd(s, line.trim(), &dir, home.as_deref()));
                        if auto {
                            let typed = line.trim().to_string();
                            let ran = format!("cd -- {typed}");
                            eprintln!("{ran}");
                            self.retyped = Some((ran.clone(), typed));
                            return Ok(ReadResult::Input(format!("{ran}\n")));
                        }
                    }
                    return Ok(ReadResult::Input(line));
                }
                navi::Line::Piped { .. } if busy => {
                    still_away(self.away.as_ref(), guard.as_deref());
                    continue;
                }
                navi::Line::Piped { command, question } => {
                    // An answer left unseen is in the conversation still.
                    self.away = None;
                    let Some(found) = guard.as_ref().and_then(|s| s.find_first_executable_in_path("navi")) else {
                        if let Some(s) = guard.as_mut() {
                            let _ = s.add_to_history(line.trim_end_matches('\n'));
                        }
                        eprintln!("{}", navi::NOT_INSTALLED);
                        if let Some(s) = guard.as_mut() {
                            s.set_last_exit_status(127);
                        }
                        continue;
                    };
                    if let Some(s) = guard.as_mut() {
                        let typed = line.trim_end_matches('\n');
                        hooks(s, "preexec", &[typed, typed, typed]);
                    }
                    let home = std::env::var_os("HOME").map(PathBuf::from);
                    let file = navi::command_file(&self.session);
                    let _ = std::fs::remove_file(&file);
                    let ran = navi::piped_line(&command, &question, &found, &self.session, &self.recent.describe(home.as_deref()), &file);
                    self.piped = Some((ran.clone(), line.trim_end_matches('\n').to_string()));
                    self.settle = Some((line.trim().to_string(), keep));
                    return Ok(ReadResult::Input(ran));
                }
                // NSH-62: `?` alone brings navi back; another question waits for it.
                navi::Line::Navi(ask) if self.away.is_some() && (busy || ask.is_empty()) => {
                    if !ask.is_empty() {
                        still_away(self.away.as_ref(), guard.as_deref());
                        continue;
                    }
                    drop(guard);
                    let mut n = self.away.take().expect("navi away");
                    let ended = n.attach();
                    self.answered(shell, n, ended);
                    continue;
                }
                navi::Line::Navi(ask) => {
                    self.away = None;
                    // NSH-60: `?/usage` shows what navi has used and cost, as `navi usage` does.
                    if ask == "/usage" || ask.starts_with("/usage ") {
                        let found = guard.as_ref().and_then(|s| s.find_first_executable_in_path("navi"));
                        let path = guard.as_ref().and_then(|s| s.env_str("PATH").map(|p| p.into_owned()));
                        drop(guard);
                        let Some(found) = found else {
                            eprintln!("{}", navi::NOT_INSTALLED);
                            continue;
                        };
                        navi::usage(&found, path.as_deref(), &ask["/usage".len()..]);
                        continue;
                    }
                    // NSH-55: `?/resume` takes up a conversation an earlier neosh had.
                    if ask == "/resume" {
                        let found = guard.as_ref().and_then(|s| s.find_first_executable_in_path("navi"));
                        let path = guard.as_ref().and_then(|s| s.env_str("PATH").map(|p| p.into_owned()));
                        drop(guard);
                        let Some(found) = found else {
                            eprintln!("{}", navi::NOT_INSTALLED);
                            continue;
                        };
                        let saved = navi::saved(&found, path.as_deref(), &self.session);
                        if saved.is_empty() {
                            println!("No earlier conversation with navi to resume.");
                            continue;
                        }
                        let home = std::env::var_os("HOME").map(PathBuf::from);
                        for (i, s) in saved.iter().take(9).enumerate() {
                            println!("{}", navi::saved_line(i + 1, s, home.as_deref()));
                        }
                        print!("resume which? [1] ");
                        let _ = std::io::stdout().flush();
                        let mut answer = String::new();
                        let read = std::io::stdin().read_line(&mut answer);
                        let pick = match answer.trim() {
                            "" if read.is_ok_and(|n| n > 0) => Some(0),
                            n => n.parse::<usize>().ok().filter(|n| (1..=saved.len().min(9)).contains(n)).map(|n| n - 1),
                        };
                        match pick {
                            Some(i) => {
                                let s = &saved[i];
                                self.session = s.name.clone();
                                let at = s.cwd.as_deref().map(|d| format!(" from {}", prompt::tilde(d, home.as_deref()))).unwrap_or_default();
                                println!("\x1b[2m· continuing ({} message{}{at})\x1b[0m", s.messages, if s.messages == 1 { "" } else { "s" });
                            }
                            None => println!("Not resumed."),
                        }
                        continue;
                    }
                    // NSH-54: `?` alone turns navi mode on, after asking why a failure failed.
                    if ask.is_empty() && !in_mode {
                        navi_mode(guard.as_deref_mut(), true);
                        let color = guard.as_ref().is_some_and(|s| s.env_str("NO_COLOR").is_none());
                        println!("{}", if color { format!("\x1b[2m{}\x1b[0m", navi::MODE_NOTE) } else { navi::MODE_NOTE.to_string() });
                    }
                    // NSH-8: kept as typed, if bash would keep it (NSH-26).
                    if let Some(s) = guard.as_mut() {
                        let _ = s.add_to_history(line.trim_end_matches('\n'));
                        self.settle = Some((line.trim().to_string(), keep));
                    }
                    // Found as the shell finds programs, so `export PATH=…` here counts.
                    let found = guard.as_ref().and_then(|s| s.find_first_executable_in_path("navi"));
                    let path = guard.as_ref().and_then(|s| s.env_str("PATH").map(|p| p.into_owned()));
                    drop(guard);
                    // NSH-28: how the asking went, as `$?` and in the prompt.
                    let ended = |status: u8| {
                        if let Ok(mut s) = shell.try_lock() {
                            s.set_last_exit_status(status);
                        }
                    };
                    // NSH-9: alone, it asks about a failure just seen, or says how to ask.
                    let ask = match ask.is_empty() {
                        true if self.recent.last_failed() => navi::WHY_FAILED.to_string(),
                        true => {
                            ended(0);
                            continue;
                        }
                        false => ask,
                    };
                    let home = std::env::var_os("HOME").map(PathBuf::from);
                    let Some(found) = found else {
                        eprintln!("{}", navi::NOT_INSTALLED);
                        ended(127);
                        continue;
                    };
                    // NSH-34: once, when the recorder could not start.
                    if let (false, Ok(why)) = (self.told_unrecorded, std::env::var(record::FAILED)) {
                        self.told_unrecorded = true;
                        eprintln!("\x1b[2m· output is not being kept in this terminal ({why}); navi sees only the commands\x1b[0m");
                    }
                    // NSH-19: once, so a sentence meant as a command is not a mystery.
                    if !self.noted && !line.trim_start().starts_with('?') {
                        self.noted = true;
                        eprintln!("{}", navi::SENTENCE_NOTE);
                    }
                    let command = navi::command(&found, path.as_deref(), &ask, &dir, &self.session, &self.recent.describe(home.as_deref()));
                    // What navi was told about; what runs while it is away, it hears of next time.
                    self.recent.clear();
                    // NSH-62: on a terminal of its own, so ^Z can send it away.
                    match away::start(command, &ask, navi::command_file(&self.session), navi::away_file(&self.session)) {
                        Ok(mut n) => {
                            let how = n.attach();
                            self.answered(shell, n, how);
                        }
                        Err(away::Failed::NoTerminal(mut command)) => match command.status() {
                            Ok(status) => {
                                self.suggested = navi::take_suggestion(&navi::command_file(&self.session));
                                ended(navi::status_of(status));
                            }
                            Err(e) => {
                                eprintln!("neosh: could not start navi: {e}");
                                ended(126);
                            }
                        },
                        Err(away::Failed::Spawn(e)) => {
                            eprintln!("neosh: could not start navi: {e}");
                            ended(126);
                        }
                    }
                }
            }
        }
    }

    fn get_read_buffer(&self) -> Option<(String, usize)> {
        self.inner.get_read_buffer()
    }

    fn set_read_buffer(&mut self, buffer: String, cursor: usize) {
        self.inner.set_read_buffer(buffer, cursor);
    }
}

/// NSH-40: the job `fg` (or `%n` alone) brings to the foreground.
fn brought_back<SE: brush_core::ShellExtensions>(shell: &mut brush_core::Shell<SE>, line: &str) -> Option<usize> {
    let mut words = line.split_whitespace();
    let spec = match words.next()? {
        "fg" => words.next().unwrap_or("%+").to_string(),
        w if w.starts_with('%') && words.next().is_none() => w.to_string(),
        _ => return None,
    };
    shell.jobs_mut().resolve_job_spec(&spec).map(|j| j.id)
}

/// NSH-33: zsh's hooks, as zsh and bash-preexec run them: the `name`
/// function, then each named in `name_functions`, with `args`, every one
/// seeing `$?` as it was and leaving it so. Whether any ran.
fn hooks<SE: brush_core::ShellExtensions>(shell: &mut brush_core::Shell<SE>, name: &str, args: &[&str]) -> bool {
    let mut funcs: Vec<String> = shell.func_mut(name).map(|_| name.to_string()).into_iter().collect();
    if let Some(brush_core::ShellValue::IndexedArray(listed)) = shell.env_var(&format!("{name}_functions")).map(|v| v.value().clone()) {
        funcs.extend(listed.values().filter(|f| shell.func_mut(f).is_some()).cloned());
    }
    let status = shell.last_exit_status();
    let params = shell.default_exec_params();
    for func in &funcs {
        shell.set_last_exit_status(status);
        let _ = tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(shell.invoke_function(func, args, &params)));
    }
    shell.set_last_exit_status(status);
    !funcs.is_empty()
}

/// NSH-13: neosh's own history file, unless the environment names one.
fn history_file(inherited: Option<std::ffi::OsString>, home: Option<std::ffi::OsString>) -> Option<PathBuf> {
    match inherited {
        Some(_) => None,
        None => home.map(|h| PathBuf::from(h).join(".neosh_history")),
    }
}

/// NSH-26: history put right once `line` is in it: dropped if not kept,
/// and with `erase` its earlier copies dropped.
fn settle(history: &mut brush_core::history::History, line: &str, keep: bool, erase: bool) {
    let ids: Vec<_> = history.iter().filter(|i| i.command_line == line).map(|i| i.id).collect();
    let dropped = match (keep, ids.split_last()) {
        (false, Some((last, _))) => vec![*last],
        (true, Some((_, earlier))) if erase => earlier.to_vec(),
        _ => Vec::new(),
    };
    for id in dropped {
        let _ = history.delete_item_by_id(id);
    }
}

/// NSH-29: whether `line` is only the name of a directory and no command,
/// in words the shell reads as they are (`~` aside), so `cd -- <line>` goes there.
fn autocd<SE: brush_core::ShellExtensions>(shell: &mut brush_core::Shell<SE>, line: &str, dir: &Path, home: Option<&Path>) -> bool {
    let plain = !line.is_empty() && line.chars().all(|c| c.is_alphanumeric() || "._-/~+@:,%".contains(c));
    if !plain || is_command(shell, line) {
        return false;
    }
    let path = match (line.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => home.join(rest.trim_start_matches('/')),
        _ => dir.join(line),
    };
    path.is_dir()
}

/// NSH-4: whether a word runs something: a keyword, alias, function, builtin or program.
/// NSH-54: whether every line goes to navi, until Esc.
static NAVI_MODE: AtomicBool = AtomicBool::new(false);
/// What Esc is bound to while navi mode is on.
const LEAVE_NAVI_MODE: &str = "neosh-leave-navi-mode";

/// NSH-54: navi mode on or off, Esc bound to leave it while it is on and
/// given back its own job after.
fn navi_mode<SE: brush_core::ShellExtensions>(shell: Option<&mut brush_core::Shell<SE>>, on: bool) {
    use brush_core::interfaces::{InputFunction, Key, KeyAction, KeySequence, KeyStroke};
    NAVI_MODE.store(on, Ordering::Relaxed);
    let Some(bindings) = shell.and_then(|s| s.key_bindings().cloned()) else { return };
    let esc = KeySequence::Strokes(vec![KeyStroke { alt: false, control: false, shift: false, key: Key::Escape }]);
    let action = if on { KeyAction::ShellCommand(LEAVE_NAVI_MODE.to_string()) } else { KeyAction::DoInputFunction(InputFunction::Abort) };
    let _ = tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(bindings.lock())).bind(esc, action);
}

/// NSH-54: the prompt with its `$` (or `#`) turned to `?`, or `? ` after it.
fn navi_prompt(prompt: &str) -> String {
    let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    if let Some(at) = prompt.rfind(['$', '#']) {
        if ansi.replace_all(&prompt[at + 1..], "").trim().is_empty() {
            return format!("{}?{}", &prompt[..at], &prompt[at + 1..]);
        }
    }
    format!("{prompt}? ")
}

/// NSH-53: a line for navi is complete when Enter is pressed, though a `'`
/// in it (`?what's wrong`) would leave bash waiting for the quote's end.
fn completes(line: &str, shell: &mut dyn std::any::Any) -> bool {
    if NAVI_MODE.load(Ordering::Relaxed) {
        return true;
    }
    let Some(shell) = shell.downcast_mut::<brush_core::Shell<errors::Extensions>>() else { return false };
    let ask = navi::Ask::from_setting(shell.env_str("NEOSH_ASK").as_deref());
    let dir = shell.working_dir().to_path_buf();
    !matches!(navi::classify_in(line, ask, |word| is_command(shell, word), |word| navi::globs_a_file(word, &dir)), navi::Line::Shell)
}

fn is_command<SE: brush_core::ShellExtensions>(shell: &mut brush_core::Shell<SE>, word: &str) -> bool {
    shell.is_keyword(word)
        || shell.aliases().contains_key(word)
        || shell.func_mut(word).is_some()
        || shell.builtin_mut(word).is_some()
        // A directory on PATH (`bin/..`) is no program.
        || shell.find_first_executable_in_path(word).is_some_and(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    // NSH-54
    #[test]
    fn navi_mode_s_prompt_ends_in_a_question_mark() {
        assert_eq!(navi_prompt("~/proj $ "), "~/proj ? ");
        assert_eq!(navi_prompt("\x1b[1;34m~/proj\x1b[0m 1 $\x1b[0m "), "\x1b[1;34m~/proj\x1b[0m 1 ?\x1b[0m ");
        assert_eq!(navi_prompt("❯ "), "❯ ? ");
        assert_eq!(navi_prompt("$HOME> "), "$HOME> ? ");
    }

    fn argv(s: &[&str]) -> Vec<String> {
        s.iter().map(|a| a.to_string()).collect()
    }

    // NSH-13
    #[test]
    fn history_is_kept_in_neosh_s_own_file() {
        assert_eq!(history_file(None, Some("/home/u".into())), Some(PathBuf::from("/home/u/.neosh_history")));
        assert_eq!(history_file(Some("/tmp/h".into()), Some("/home/u".into())), None);
        assert_eq!(history_file(None, None), None);
    }

    // NSH-1
    #[test]
    fn arguments_say_what_to_run() {
        assert_eq!(parse_args(&argv(&["neosh"])).unwrap(), Args::default());
        assert_eq!(parse_args(&argv(&["-neosh"])).unwrap(), Args { login: true, ..Default::default() });
        assert_eq!(
            parse_args(&argv(&["neosh", "-l", "-c", "echo $0 $1", "me", "one"])).unwrap(),
            Args { command: Some("echo $0 $1".into()), login: true, rest: argv(&["me", "one"]), ..Default::default() }
        );
        assert_eq!(parse_args(&argv(&["neosh", "run.sh", "-x"])).unwrap(), Args { rest: argv(&["run.sh", "-x"]), ..Default::default() });
        assert!(parse_args(&argv(&["neosh", "-c"])).is_err());
        assert!(parse_args(&argv(&["neosh", "--nope"])).is_err());
    }
}
