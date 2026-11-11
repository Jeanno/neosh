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
        self.neosh_with_path(args, "/usr/bin:/bin")
    }

    fn neosh_with_path(&self, args: &[&str], path: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_neosh"))
            .args(args)
            .current_dir(&self.0)
            .env_clear()
            .env("HOME", &self.0)
            .env("ZDOTDIR", &self.0)
            .env("PATH", path)
            .output()
            .unwrap()
    }

    /// A stand-in starship on a PATH of its own: two presets, and a prompt.
    fn fake_starship(&self) -> String {
        let bin = self.0.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = "#!/bin/sh\ncase \"$1 $2\" in\n  'preset --list') printf 'pure-preset\\ntokyo-night\\n' ;;\n  'preset pure-preset') echo 'format = \"$directory$character\"' ;;\n  'preset tokyo-night') echo 'format = \"tokyo\"' ;;\n  prompt*) echo '> ' ;;\n  *) exit 1 ;;\nesac\n";
        let path = bin.join("starship");
        std::fs::write(&path, script).unwrap();
        std::os::unix::fs::PermissionsExt::set_mode(&mut std::fs::metadata(&path).unwrap().permissions(), 0o755);
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        format!("{}:/usr/bin:/bin", bin.display())
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
    assert!(String::from_utf8_lossy(&out.stdout).contains("already has this look"), "{out:?}");
    // neosh's own look takes setup's lines out, keeps only the choice, and leaves the rest.
    assert!(home.neosh(&["--setup", "--look", "neosh", "--yes"]).status.success());
    assert_eq!(home.rc(), "alias ll='ls -l'\n\n# >>> neosh setup >>>\n# neosh's own look; `neosh --setup` changes it.\n# <<< neosh setup <<<\n");
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
    assert!(String::from_utf8_lossy(&out.stderr).contains("--look zsh|neosh|starship (or --prompt '<parts>') and --yes"), "{out:?}");
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

// NSH-73, NSH-75
#[test]
fn the_prompt_s_parts_are_chosen_without_questions() {
    let home = Home::new("parts");
    let out = home.neosh(&["--setup", "--prompt", "time folder", "--yes"]);
    assert!(out.status.success(), "{out:?}");
    assert!(home.rc().contains("NEOSH_PROMPT='folder time'"), "{}", home.rc());
    let bad = home.neosh(&["--setup", "--prompt", "folder sparkles", "--yes"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("sparkles is not a part"), "{bad:?}");
    // The default parts write only the choice.
    assert!(home.neosh(&["--setup", "--prompt", "dir branch took status", "--yes"]).status.success());
    assert!(!home.rc().contains("NEOSH_PROMPT"), "{}", home.rc());
}

// NSH-76
#[test]
fn the_prompt_s_colours_are_chosen_without_questions() {
    let home = Home::new("colors");
    let out = home.neosh(&["--setup", "--prompt", "dir time", "--colors", "dir=green time=#888888", "--yes"]);
    assert!(out.status.success(), "{out:?}");
    assert!(home.rc().contains("NEOSH_PROMPT_COLORS='dir=green time=#888888'"), "{}", home.rc());
    let bad = home.neosh(&["--setup", "--colors", "dir=sparkly", "--yes"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("dir=sparkly is not part=colour"), "{bad:?}");
}

// NSH-77
#[test]
fn starship_and_a_preset_are_written_the_config_before_kept() {
    let home = Home::new("starship");
    let path = home.fake_starship();
    std::fs::create_dir_all(home.0.join(".config")).unwrap();
    std::fs::write(home.0.join(".config/starship.toml"), "# mine\n").unwrap();
    let out = home.neosh_with_path(&["--setup", "--preset", "tokyo-night", "--yes"], &path);
    assert!(out.status.success(), "{out:?}");
    assert!(home.rc().contains("command -v starship >/dev/null && eval \"$(starship init bash)\""), "{}", home.rc());
    assert_eq!(std::fs::read_to_string(home.0.join(".config/starship.toml")).unwrap(), "format = \"tokyo\"\n");
    assert_eq!(std::fs::read_to_string(home.0.join(".config/starship.toml.bak")).unwrap(), "# mine\n");
    let bad = home.neosh_with_path(&["--setup", "--preset", "nope", "--yes"], &path);
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("nope is not one of starship's presets (pure-preset, tokyo-night)"), "{bad:?}");
    // Without starship, it says how to install it.
    let none = home.neosh(&["--setup", "--look", "starship", "--yes"]);
    assert_eq!(none.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&none.stdout).contains("brew install starship"), "{none:?}");
}

// NSH-78
#[test]
fn plugins_are_chosen_without_questions_and_kept_when_not_asked() {
    let home = Home::new("plugins");
    let out = home.neosh(&["--setup", "--look", "neosh", "--plugins", "z", "--yes"]);
    assert!(out.status.success(), "{out:?}");
    assert!(home.rc().contains("plugins=(z)"), "{}", home.rc());
    // Not asked: an earlier setup's plugins stay.
    assert!(home.neosh(&["--setup", "--prompt", "dir time", "--yes"]).status.success());
    assert!(home.rc().contains("plugins=(z)") && home.rc().contains("NEOSH_PROMPT='dir time'"), "{}", home.rc());
    assert!(home.neosh(&["--setup", "--look", "neosh", "--plugins", "", "--yes"]).status.success());
    assert!(!home.rc().contains("plugins="), "{}", home.rc());
    let bad = home.neosh(&["--setup", "--look", "neosh", "--plugins", "nosuch", "--yes"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("nosuch is not a plugin neosh knows (known: z)"), "{bad:?}");
    // Plugins of your own, outside the block, are left alone.
    std::fs::write(home.0.join(".neoshrc"), "plugins=(~/mine)\n").unwrap();
    let own = home.neosh(&["--setup", "--look", "neosh", "--plugins", "z", "--yes"]);
    assert!(String::from_utf8_lossy(&own.stdout).contains("names its own plugins"), "{own:?}");
    assert!(!home.rc().contains("plugins=(z)"), "{}", home.rc());
}

// NSH-79: a login neosh gets the folders zsh puts on PATH, each once.
#[test]
fn the_folders_zsh_adds_to_path_are_written_for_neosh() {
    if !std::path::Path::new("/bin/zsh").exists() {
        return;
    }
    let home = Home::new("zshpath");
    std::fs::create_dir_all(home.0.join("tools/bin")).unwrap();
    std::fs::write(home.0.join("tools/bin/mytool"), "#!/bin/sh\necho mytool-ran\n").unwrap();
    let mut perms = std::fs::metadata(home.0.join("tools/bin/mytool")).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(home.0.join("tools/bin/mytool"), perms).unwrap();
    std::fs::write(home.0.join(".zprofile"), "path+=($HOME/tools/bin /not/there)\n").unwrap();
    let out = home.neosh(&["--setup", "--look", "neosh", "--yes"]);
    assert!(out.status.success(), "{out:?}");
    let rc = home.rc();
    assert!(rc.contains("for d in \"$HOME\"'/tools/bin'; do"), "{rc}");
    assert!(!rc.contains("/not/there"), "{rc}");
    // Read twice, as a nested neosh would: on PATH once, and it runs.
    let run = home.neosh(&["-c", "source ~/.neoshrc; source ~/.neoshrc; mytool; echo \"$PATH\" | tr : '\\n' | grep -c tools/bin"]);
    assert_eq!(String::from_utf8_lossy(&run.stdout), "mytool-ran\n1\n", "{run:?}");
}
