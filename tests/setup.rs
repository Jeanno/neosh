//! `neosh --setup` without a terminal (NSH-51), and a script that is not there (NSH-2).

use std::path::PathBuf;
use std::process::{Command, Output};

struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Home {
        let home = std::env::temp_dir().join(format!("neosh-setup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        Home(home)
    }

    fn neosh(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_neosh"))
            .args(args)
            .current_dir(&self.0)
            .env_clear()
            .env("HOME", &self.0)
            .env("ZDOTDIR", &self.0)
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap()
    }

    fn rc(&self) -> String {
        std::fs::read_to_string(self.0.join(".neoshrc")).unwrap_or_default()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// NSH-49 to NSH-51
#[test]
fn zsh_s_look_is_written_between_markers_and_written_again_in_place() {
    if !std::path::Path::new("/bin/zsh").exists() {
        return;
    }
    let home = Home::new("zsh");
    std::fs::write(home.0.join(".zshrc"), "echo hello from zshrc\nPROMPT='%F{green}%n@%m%f %~ %# '\n").unwrap();
    std::fs::write(home.0.join(".neoshrc"), "alias ll='ls -l'\n").unwrap();
    let out = home.neosh(&["--setup", "--look", "zsh", "--yes"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        home.rc(),
        "alias ll='ls -l'\n\n# >>> neosh setup >>>\nPS1='\\[\\e[32m\\]\\u@\\h\\[\\e[39m\\] \\w \\$ '\n# <<< neosh setup <<<\n"
    );
    // Again with nothing new: nothing changes.
    let out = home.neosh(&["--setup", "--look", "zsh", "--yes"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("nothing to change"), "{out:?}");
    // neosh's own look takes setup's lines out, and leaves the rest.
    assert!(home.neosh(&["--setup", "--look", "neosh", "--yes"]).status.success());
    assert_eq!(home.rc(), "alias ll='ls -l'\n");
}

// NSH-49
#[test]
fn a_prompt_bash_cannot_draw_keeps_neosh_s_own() {
    if !std::path::Path::new("/bin/zsh").exists() {
        return;
    }
    let home = Home::new("subst");
    std::fs::write(home.0.join(".zshrc"), "setopt prompt_subst\nPROMPT='$(date) %~ '\n").unwrap();
    let out = home.neosh(&["--setup", "--look", "zsh", "--yes"]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(said.contains("runs commands") && said.contains("neosh's own prompt is kept"), "{said}");
    assert!(!home.rc().contains("PS1"), "{}", home.rc());
}

// NSH-47, NSH-51
#[test]
fn setup_without_a_terminal_needs_every_choice() {
    let home = Home::new("noterm");
    let out = home.neosh(&["--setup"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--look zsh|neosh and --yes"), "{out:?}");
    assert_eq!(home.neosh(&["--setup", "--look", "fish"]).status.code(), Some(2));
    assert!(!home.0.join(".neoshrc").exists());
}

// NSH-2
#[test]
fn a_script_that_is_not_there_is_said_as_bash_says_it() {
    let home = Home::new("noscript");
    let out = home.neosh(&["setup"]);
    assert_eq!(out.status.code(), Some(127));
    assert_eq!(String::from_utf8_lossy(&out.stderr), "neosh: setup: No such file or directory\n");
}
