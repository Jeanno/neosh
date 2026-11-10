//! neosh end to end on a terminal: a pseudo-terminal from `drive.py`, a
//! home of its own, and a stand-in navi that logs what it was given.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Logs its arguments, directory, what it was told and what was piped in; a
/// prompt asking for a suggestion hands one back (CLI-41).
const STAND_IN: &str = r#"#!/bin/sh
[ "$1" = sessions ] && { cat "$HOME/sessions.json" 2>/dev/null; exit 0; }
{ echo "ARGS: $*"; echo "PWD: $PWD"; printf 'RECENT: %s\n' "$NAVI_RECENT"; echo "INDEX: $NAVI_SHELL_INDEX"; [ -t 0 ] || echo "STDIN: $(cat)"; echo ---; } >> "$HOME/navi.log"
[ -n "$NAVI_SHELL_INDEX" ] && rm -rf "$HOME/rec" && cp -R "$(dirname "$NAVI_SHELL_INDEX")" "$HOME/rec"
case "$*" in *suggest*) echo 'echo suggested-ran' > "$NAVI_COMMAND_FILE" ;; esac
echo "navi says hi"
case "$*" in *"cannot answer"*) exit 3 ;; esac
"#;

struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Home {
        let home = std::env::temp_dir().join(format!("neosh-e2e-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("work")).unwrap();
        let navi = home.join("bin/navi");
        std::fs::write(&navi, STAND_IN).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&navi, std::fs::Permissions::from_mode(0o755)).unwrap();
        // So neosh does not offer setup (NSH-47); the tests of setup take it out.
        std::fs::write(home.join(".neoshrc"), "").unwrap();
        Home(home)
    }

    /// What the screen showed for these lines, typed into a fresh neosh.
    fn run(&self, lines: &[&str], env: &[(&str, &str)]) -> String {
        let driver = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/drive.py");
        let out = Command::new("python3")
            .arg("-I")
            .arg(driver)
            .arg(env!("CARGO_BIN_EXE_neosh"))
            .args(lines)
            .current_dir(self.0.join("work"))
            .env_clear()
            .env("HOME", &self.0)
            .env("PATH", format!("{}:/usr/bin:/bin", self.0.join("bin").display()))
            .env("TERM", "xterm-256color")
            .envs(env.iter().copied())
            .output()
            .expect("python3 to drive neosh");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.0.join("navi.log")).unwrap_or_default()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// NSH-4 to NSH-7, NSH-17, NSH-19
#[test]
fn a_sentence_and_a_question_reach_navi_with_what_was_run() {
    let home = Home::new("ask");
    let screen = home.run(&["false", "why did that fail", "? and now", "gti status", "exit"], &[("NEOSH_ASK", "sentences")]);
    let log = home.log();
    assert!(log.contains("-p why did that fail"), "{log}");
    assert!(log.contains("-p and now"), "{log}");
    assert!(log.contains("RECENT: ~/work $ false (exit 1, printed nothing)"), "{log}");
    let work = std::fs::canonicalize(home.0.join("work")).unwrap();
    assert!(log.contains(&format!("PWD: {}", work.display())), "{log}");
    // One conversation: both asked in the same session.
    let sessions: Vec<&str> = log.lines().filter(|l| l.starts_with("ARGS:")).map(|l| l.split_whitespace().nth(3).unwrap()).collect();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0], sessions[1]);
    assert!(sessions[0].starts_with("neosh-"));
    // Said once, for the sentence only; a typo is still a typo.
    assert_eq!(screen.matches("a sentence goes to navi").count(), 1, "{screen}");
    assert!(screen.contains("gti: command not found"), "{screen}");
}

// NSH-4, NSH-23
#[test]
fn a_sentence_runs_as_a_command_then_waits_as_a_question() {
    let home = Home::new("marked");
    let screen = home.run(&["why did that fail", "", "?how about now", "exit"], &[]);
    assert!(screen.contains("why: command not found"), "{screen}");
    assert!(screen.contains("$ ?why did that fail"), "{screen}");
    let log = home.log();
    assert_eq!(log.matches("ARGS:").count(), 2, "{log}");
    assert!(log.contains("-p how about now"), "{log}");
    assert!(log.contains("-p why did that fail"), "{log}");
    assert!(log.contains("RECENT: ~/work $ why did that fail (exit 127, printed 1 line: "), "{log}");
    assert!(!screen.contains("a sentence goes to navi"), "{screen}");
}

// NSH-23, NSH-17, NSH-9
#[test]
fn the_offered_question_can_be_cleared_and_nothing_is_sent() {
    let home = Home::new("cleared");
    let screen = home.run(&["show me the files", "\u{15}true", "?", "@esc", "NEOSH_ASK=off", "show me the files", "? hi", "exit"], &[]);
    assert!(screen.contains("navi mode: each line goes to navi"), "{screen}");
    assert_eq!(screen.matches("show: command not found").count(), 2, "{screen}");
    assert_eq!(home.log(), "");
}

// NSH-11
#[test]
fn the_command_navi_suggests_waits_at_the_prompt() {
    let home = Home::new("suggest");
    let screen = home.run(&["? suggest something", "", "exit"], &[]);
    assert!(screen.lines().any(|l| l.trim() == "suggested-ran"), "{screen}");
}

// NSH-16, NSH-8
#[test]
fn output_piped_to_a_question_reaches_navi_and_history_keeps_the_line() {
    let home = Home::new("piped");
    let screen = home.run(&["ls missing-folder | ? why", "history", "exit"], &[]);
    let log = home.log();
    assert!(log.contains("-a - -p why"), "{log}");
    assert!(log.contains("STDIN: ls: missing-folder"), "{log}");
    assert!(screen.contains("  ls missing-folder | ? why"), "{screen}");
}

// NSH-12, NSH-13
#[test]
fn the_prompt_shows_the_branch_and_history_outlives_the_session() {
    let home = Home::new("prompt");
    std::fs::create_dir_all(home.0.join("work/.git")).unwrap();
    std::fs::write(home.0.join("work/.git/HEAD"), "ref: refs/heads/main\n").unwrap();
    let screen = home.run(&["false", "echo from-the-first", "exit"], &[]);
    assert!(screen.contains("~/work main 1 $"), "{screen}");
    let again = home.run(&["history", "exit"], &[]);
    assert!(again.contains("echo from-the-first"), "{again}");
    assert!(home.0.join(".neosh_history").exists());
}

// NSH-5
#[test]
fn ctrl_c_stops_navi_and_never_the_shell() {
    let home = Home::new("interrupt");
    std::fs::write(home.0.join("bin/navi"), "#!/bin/sh\necho thinking\nsleep 30\necho never\n").unwrap();
    let screen = home.run(&["? a slow one", "\x03", "echo still-here", "? again", "\x03", "echo here-too", "exit"], &[]);
    assert!(screen.contains("still-here"), "{screen}");
    assert!(screen.contains("here-too"), "{screen}");
    assert!(!screen.contains("never"), "{screen}");
}

// NSH-3, NSH-6
#[test]
fn job_control_and_a_command_of_several_lines_work_through_neosh() {
    let home = Home::new("jobs");
    let screen = home.run(
        &["sleep 20", "@wait 1", "\x1a", "jobs", "fg", "@wait 1", "\x03", "for f in a b; do", "echo got-$f", "done", "? what ran", "exit"],
        &[],
    );
    assert!(screen.contains("Stopped\tsleep 20"), "{screen}");
    assert!(screen.contains("got-a") && screen.contains("got-b"), "{screen}");
    let log = home.log();
    assert!(log.contains("~/work $ for f in a b; do ↵ echo got-$f ↵ done (ok, printed 2 lines: "), "{log}");
}

// NSH-53
#[test]
fn a_quote_in_a_question_does_not_hold_it_open() {
    let home = Home::new("quote");
    let screen = home.run(&["?what's wrong", "echo ok |?why's that", "echo 'held", "closed'", "exit"], &[]);
    let log = home.log();
    assert!(log.contains("-p what's wrong"), "{log}");
    assert!(log.contains("STDIN: ok"), "{log}");
    assert!(log.contains("why's that"), "{log}");
    // The shell's own line still waits for its quote.
    assert!(screen.contains("held\nclosed"), "{screen}");
}

// NSH-9, NSH-54
#[test]
fn a_question_mark_alone_after_a_failure_asks_why() {
    let home = Home::new("why");
    let screen = home.run(&["ls missing-folder", "?", "@esc", "true", "?", "@esc", "exit"], &[]);
    let log = home.log();
    assert_eq!(log.matches("ARGS:").count(), 1, "{log}");
    assert!(log.contains("-p why did my last command fail"), "{log}");
    assert!(log.contains("~/work $ ls missing-folder (exit 1, printed 1 line: "), "{log}");
    // Each time, the mode begins; after a command that worked, nothing is asked.
    assert_eq!(screen.matches("navi mode: each line goes to navi").count(), 2, "{screen}");
}

// NSH-55
#[test]
fn resume_takes_up_an_earlier_conversation() {
    let home = Home::new("resume");
    let screen = home.run(&["?/resume", "? hi", "exit"], &[]);
    assert!(screen.contains("No earlier conversation with navi to resume."), "{screen}");
    std::fs::write(
        home.0.join("sessions.json"),
        concat!(
            r#"{"name":"neosh-7-1","cwd":"/tmp/a","messages":4,"used":"x","ago":"5 min ago","last":"why is it slow"}"#, "\n",
            r#"{"name":"neosh-6-1","cwd":"/tmp/b","messages":9,"used":"x","ago":"2 h ago","last":"set up a venv"}"#, "\n",
        ),
    )
    .unwrap();
    let screen = home.run(&["?/resume", "2", "? and now", "exit"], &[]);
    assert!(screen.contains("1  5 min ago"), "{screen}");
    assert!(screen.contains("set up a venv"), "{screen}");
    assert!(screen.contains("· continuing (9 messages from /tmp/b)"), "{screen}");
    let log = home.log();
    assert!(log.contains("--session neosh-6-1 -p and now"), "{log}");
}

// NSH-54
#[test]
fn navi_mode_sends_every_line_until_esc() {
    let home = Home::new("mode");
    let screen = home.run(
        &["?", "what is here", "*.txt and it's |", "", "@type abc", "@esc", "", "?", "exit", "echo back-in-the-shell", "exit"],
        &[],
    );
    let log = home.log();
    assert!(log.contains("-p what is here"), "{log}");
    assert!(log.contains("-p *.txt and it's |"), "{log}");
    // The third: `?` right after `abc` failed asks why, then the mode begins.
    assert_eq!(log.matches("ARGS:").count(), 3, "{log}");
    assert!(screen.contains("~/work ? what is here"), "{screen}");
    // Esc kept what was typed, for the shell to run.
    assert!(screen.contains("abc: command not found"), "{screen}");
    // `exit` left the mode, not the shell.
    assert!(screen.contains("back-in-the-shell"), "{screen}");
    let history = std::fs::read_to_string(home.0.join(".neosh_history")).unwrap_or_default();
    assert!(history.contains("?what is here"), "{history}");
}

// NSH-56
#[test]
fn a_line_for_navi_is_not_highlighted_as_bash() {
    let home = Home::new("plain");
    let raw = home.run(&["echo \"shellside\"", "?what is \"quoted\" here", "?", "echo \"naviside\" $HOME", "exit", "exit"], &[("DRIVE_RAW", "1")]);
    // The shell's line is coloured; navi's are drawn as typed.
    assert!(raw.contains("ESC[33mshellside"), "{raw}");
    assert!(raw.contains("?what is \"quoted\" here"), "{raw}");
    assert!(raw.contains("echo \"naviside\" $HOME"), "{raw}");
    assert!(!raw.contains("mquoted") && !raw.contains("mnaviside"), "{raw}");
}

// NSH-56: brush could end a span inside a character and panic.
#[test]
fn a_line_with_wide_characters_is_highlighted_without_a_panic() {
    let home = Home::new("wide");
    let screen = home.run(&["echo 同志 \"x\"", "echo \"同志\" $HOME", "echo $(echo 志) ${HOME}志", "echo still-here", "exit"], &[]);
    assert!(!screen.contains("panicked"), "{screen}");
    assert!(screen.contains("同志 x"), "{screen}");
    assert!(screen.lines().any(|l| l.trim() == "still-here"), "{screen}");
}

// NSH-57
#[test]
fn in_navi_mode_the_offered_command_runs_in_the_shell() {
    let home = Home::new("offered");
    let screen = home.run(&["?", "suggest something", "", "?changed it", "@esc", "exit"], &[]);
    let log = home.log();
    // Enter on what navi offered ran it, and the mode went on.
    assert!(screen.lines().any(|l| l.trim() == "suggested-ran"), "{screen}");
    assert_eq!(log.matches("ARGS:").count(), 2, "{log}");
    assert!(log.contains("-p ?changed it") || log.contains("-p changed it"), "{log}");
}

// NSH-24
#[test]
fn history_expands_as_in_bash() {
    let home = Home::new("bang");
    let screen = home.run(&["echo one two", "echo !$", "!!", "^two^three", "!nope", "set +H", "echo !!", "history", "exit"], &[]);
    let lines: Vec<&str> = screen.lines().map(str::trim).collect();
    // Printed, then run.
    assert!(lines.contains(&"echo two"), "{screen}");
    assert_eq!(lines.iter().filter(|l| **l == "two").count(), 2, "{screen}");
    assert!(lines.contains(&"echo three") && lines.contains(&"three"), "{screen}");
    assert!(screen.contains("neosh: !nope: event not found"), "{screen}");
    // Off with `set +H`.
    assert!(lines.contains(&"!!"), "{screen}");
    // History keeps what ran.
    assert!(lines.iter().any(|l| l.ends_with("echo two") && l.starts_with(char::is_numeric)), "{screen}");
    assert!(!lines.iter().any(|l| l.ends_with("echo !$") && l.starts_with(char::is_numeric)), "{screen}");
}

// NSH-32
#[test]
fn history_modifiers_work_as_in_bash() {
    let home = Home::new("modifiers");
    let screen = home.run(&["echo /usr/lib/x.tar.gz", "ls -d !$:h", "!echo:s/x/y/:p", "\x1b[A", "echo !!:t:r", "exit"], &[]);
    let lines: Vec<&str> = screen.lines().map(str::trim).collect();
    assert!(lines.contains(&"ls -d /usr/lib") && lines.contains(&"/usr/lib"), "{screen}");
    // `:p` prints, runs nothing, and ↑ finds it.
    assert!(lines.contains(&"echo /usr/lib/y.tar.gz"), "{screen}");
    assert_eq!(lines.iter().filter(|l| **l == "/usr/lib/y.tar.gz").count(), 1, "{screen}");
    assert!(lines.contains(&"echo y.tar") && lines.contains(&"y.tar"), "{screen}");
}

// NSH-33
#[test]
fn precmd_and_preexec_run_as_in_zsh_and_leave_the_status() {
    let home = Home::new("hooks");
    let screen = home.run(
        &[
            "pc() { echo \"pc saw $?\"; return 0; }; pe() { echo \"pe: $1\"; return 3; }",
            "precmd_functions=(pc missing); preexec_functions=(pe)",
            "false",
            "echo status $?",
            "unset precmd_functions preexec_functions",
            "precmd() { PS1='hooked $ '; }",
            "exit",
        ],
        &[],
    );
    let lines: Vec<&str> = screen.lines().map(str::trim).collect();
    assert!(lines.contains(&"pe: false") && lines.contains(&"pc saw 1"), "{screen}");
    // The status is the command's, for the prompt and for `$?`.
    assert!(screen.contains("~/work 1 $ echo status"), "{screen}");
    assert!(lines.contains(&"status 1"), "{screen}");
    assert!(screen.contains("hooked $ exit"), "{screen}");
}

// NSH-26
#[test]
fn history_keeps_what_bash_would() {
    let home = Home::new("histctl");
    let screen = home.run(
        &["HISTCONTROL=ignoreboth", "echo a", "echo a", " echo secret", " ? secret question", "HISTIGNORE='ls*'", "ls -d /", "history", "exit"],
        &[],
    );
    // Not kept is still run, or asked.
    assert!(screen.lines().any(|l| l.trim() == "secret"), "{screen}");
    assert!(home.log().contains("-p secret question"), "{}", home.log());
    let saved = std::fs::read_to_string(home.0.join(".neosh_history")).unwrap();
    let lines: Vec<&str> = saved.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(lines, ["HISTCONTROL=ignoreboth", "echo a", "HISTIGNORE='ls*'", "history", "exit"], "{saved}");
}

// NSH-27
#[test]
fn history_is_as_long_as_histsize_and_histfilesize_say() {
    let home = Home::new("histsize");
    let earlier: String = (1..=20).map(|n| format!("echo {n}\n")).collect();
    std::fs::write(home.0.join(".neosh_history"), &earlier).unwrap();
    let screen = home.run(&["echo sizes $HISTSIZE $HISTFILESIZE", "exit"], &[]);
    assert!(screen.lines().any(|l| l.trim() == "sizes 500 500"), "bash's defaults: {screen}");
    std::fs::write(home.0.join(".neosh_history"), &earlier).unwrap();
    let screen = home.run(&["history | wc -l", "exit"], &[("HISTSIZE", "5"), ("HISTFILESIZE", "3")]);
    assert!(screen.lines().any(|l| l.trim() == "5"), "{screen}");
    let saved = std::fs::read_to_string(home.0.join(".neosh_history")).unwrap();
    assert_eq!(saved, "echo 20\nhistory | wc -l\nexit\n");
    std::fs::write(home.0.join(".neosh_history"), &earlier).unwrap();
    home.run(&["exit"], &[("HISTFILESIZE", "-1")]);
    assert_eq!(std::fs::read_to_string(home.0.join(".neosh_history")).unwrap().lines().count(), 21, "below zero keeps all");
}

// NSH-28
#[test]
fn a_line_for_navi_sets_the_status_as_a_command_does() {
    let home = Home::new("status");
    let screen = home.run(
        &["false", "? why", "echo after-answer $?", "? you cannot answer", "echo after-failure $?", "false", "?", "@esc", "echo after-why $?", "true", "?", "@esc", "echo after-help $?", "exit"],
        &[],
    );
    let lines: Vec<&str> = screen.lines().map(str::trim).collect();
    for want in ["after-answer 0", "after-failure 3", "after-why 0", "after-help 0"] {
        assert!(lines.contains(&want), "{want}: {screen}");
    }
    // The prompt after an answer shows no failure from before it.
    assert!(screen.contains("~/work 1 $ ? why"), "{screen}");
    assert!(screen.contains("~/work $ echo after-answer"), "{screen}");
    // With no navi to ask: 127, as a command not found.
    let screen = home.run(&["PATH=/usr/bin:/bin", "? anyone", "echo none $?", "exit"], &[]);
    assert!(screen.lines().any(|l| l.trim() == "none 127"), "{screen}");
}

// NSH-29, NSH-30, NSH-31
#[test]
fn autocd_cdspell_and_the_terminal_size_work_as_in_bash() {
    let home = Home::new("shopts");
    std::fs::create_dir_all(home.0.join("work/alpha/beta")).unwrap();
    let screen = home.run(
        &["alpha", "shopt -s autocd cdspell", "alpha", "pwd", "..", "cd alpah/bta", "pwd", "echo size $COLUMNS $LINES", "history", "exit"],
        &[],
    );
    let lines: Vec<&str> = screen.lines().map(str::trim).collect();
    let work = std::fs::canonicalize(home.0.join("work")).unwrap();
    // Off by default: a directory's name is no command.
    assert!(screen.contains("alpha: command not found"), "{screen}");
    assert!(lines.contains(&"cd -- alpha"), "{screen}");
    assert!(lines.contains(&format!("{}", work.join("alpha").display()).as_str()), "{screen}");
    assert!(lines.contains(&"cd -- .."), "{screen}");
    assert!(lines.contains(&"alpha/beta"), "cdspell says what it went to: {screen}");
    assert!(lines.contains(&format!("{}", work.join("alpha/beta").display()).as_str()), "{screen}");
    assert!(lines.contains(&"size 120 40"), "{screen}");
    // History keeps the line as typed.
    assert!(lines.iter().any(|l| l.ends_with(" ..") && l.starts_with(char::is_numeric)), "{screen}");
    assert!(!lines.iter().any(|l| l.ends_with("cd -- ..") && l.starts_with(char::is_numeric)), "{screen}");
}

// NSH-47
#[test]
fn a_first_start_offers_setup_and_never_means_never() {
    let home = Home::new("never");
    std::fs::remove_file(home.0.join(".neoshrc")).unwrap();
    let screen = home.run(&["n", "echo still-here", "exit"], &[]);
    assert!(screen.contains("Set neosh up now? [Y]es, [l]ater, [n]ever"), "{screen}");
    assert!(screen.contains("still-here"), "{screen}");
    let rc = std::fs::read_to_string(home.0.join(".neoshrc")).unwrap();
    assert!(rc.starts_with('#') && rc.contains("neosh --setup"), "{rc}");
    let screen = home.run(&["exit"], &[]);
    assert!(!screen.contains("Set neosh up now?"), "{screen}");
}

// NSH-47
#[test]
fn later_writes_nothing_and_asks_again() {
    let home = Home::new("later");
    std::fs::remove_file(home.0.join(".neoshrc")).unwrap();
    let screen = home.run(&["l", "exit"], &[]);
    assert!(screen.contains("offered next time"), "{screen}");
    assert!(!home.0.join(".neoshrc").exists());
}

// NSH-47 to NSH-50
#[test]
fn setup_at_first_start_runs_navi_s_and_copies_zsh_s_look_at_once() {
    let home = Home::new("firstsetup");
    std::fs::remove_file(home.0.join(".neoshrc")).unwrap();
    std::fs::write(home.0.join(".zshrc"), "PROMPT='%n-zsh-look %# '\nRPROMPT='%T'\nexport LSCOLORS=Gxfxcxdxbxegedabagacad\n").unwrap();
    let zdotdir = home.0.display().to_string();
    let screen = home.run(&["y", "y", "2", "y", "echo $LSCOLORS", "exit"], &[("ZDOTDIR", &zdotdir)]);
    assert!(home.log().contains("ARGS: setup"), "{}", home.log());
    assert!(screen.contains("RPROMPT"), "{screen}");
    let rc = std::fs::read_to_string(home.0.join(".neoshrc")).unwrap();
    assert!(rc.contains("PS1='\\u-zsh-look \\$ '"), "{rc}");
    assert!(rc.contains("export LSCOLORS='Gxfxcxdxbxegedabagacad'"), "{rc}");
    // It counts at once: the prompt is zsh's, and the colours are there.
    assert!(screen.contains("-zsh-look $ echo"), "{screen}");
    assert!(screen.contains("\nGxfxcxdxbxegedabagacad"), "{screen}");
}

// NSH-34, NSH-36, NSH-37, NSH-39, NSH-41, NSH-42, NSH-43
#[test]
fn what_commands_print_is_kept_for_navi_and_gone_with_the_terminal() {
    let home = Home::new("record");
    let screen = home.run(
        &["printf '\\033[31mred\\033[0m\\nsecond\\n'", " echo private-one", "NEOSH_RECORD=off", "echo private-two", "NEOSH_RECORD=on", "echo token=abc123", "? what printed", "exit"],
        &[],
    );
    // The terminal saw it all, colours and all, and no marks.
    assert!(screen.contains("red") && screen.contains("private-one") && screen.contains("private-two"), "{screen}");
    assert!(!screen.contains("6973"), "{screen}");
    let log = home.log();
    let index = log.lines().find_map(|l| l.strip_prefix("INDEX: ")).filter(|i| !i.is_empty()).unwrap_or_else(|| panic!("{log}")).to_string();
    // Told where, never what.
    assert!(log.contains("~/work $ printf '\\033[31mred\\033[0m\\nsecond\\n' (ok, printed 2 lines: "), "{log}");
    assert!(!log.lines().any(|l| l == "second" || l.contains("STDIN")), "{log}");
    let rec = home.0.join("rec");
    let listed = std::fs::read_to_string(rec.join("index.txt")).unwrap();
    assert!(listed.contains("[exit 0, ") && listed.contains("2 lines] ~/work $ printf"), "{listed}");
    assert!(!listed.contains("private"), "{listed}");
    assert!(listed.contains("$ echo token=abc123"), "{listed}");
    let kept: Vec<String> = std::fs::read_dir(&rec).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".txt") && e.file_name() != "index.txt").map(|e| std::fs::read_to_string(e.path()).unwrap()).collect();
    assert!(kept.contains(&"red\nsecond\n".to_string()), "{kept:?}");
    assert!(kept.contains(&"token=[blanked]\n".to_string()), "{kept:?}");
    assert!(!kept.iter().any(|k| k.contains("private")), "{kept:?}");
    // NSH-42: the terminal closed, and the directory went with it.
    assert!(!std::path::Path::new(&index).exists(), "{index}");
}

// NSH-43
#[test]
fn with_recording_off_from_the_start_there_is_no_recorder() {
    let home = Home::new("norecord");
    home.run(&["echo hi", "? what printed", "exit"], &[("NEOSH_RECORD", "off")]);
    let log = home.log();
    assert!(log.contains("INDEX: \n") && log.contains("~/work $ echo hi (ok)"), "{log}");
}

// NSH-45
#[test]
fn a_failure_hints_at_asking_for_the_first_three() {
    let home = Home::new("hint");
    let screen = home.run(&["false", "true", "false", "sleep 5", "@wait 0.5", "\x03", "false", "false", "exit"], &[]);
    assert_eq!(screen.matches("? to ask why").count(), 3, "{screen}");
    let screen = home.run(&["NEOSH_HINT=off", "false", "exit"], &[]);
    assert!(!screen.contains("? to ask why"), "{screen}");
}
