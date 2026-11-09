//! Errors as bash says them (NSH-52).

use std::process::Command;

// NSH-52
#[test]
fn a_script_s_errors_name_it_and_the_line() {
    let dir = std::env::temp_dir().join(format!("neosh-errors-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("s.sh"), "echo a\n?what is this\nf() {\n  nope2\n}\nf\n. ./lib.sh\n").unwrap();
    std::fs::write(dir.join("lib.sh"), "\nnope3\n").unwrap();
    let run = |args: &[&str], stdin: &str| {
        use std::io::Write as _;
        let mut child = Command::new(env!("CARGO_BIN_EXE_neosh"))
            .args(args)
            .current_dir(&dir)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
        String::from_utf8_lossy(&child.wait_with_output().unwrap().stderr).into_owned()
    };
    assert_eq!(
        run(&["s.sh"], ""),
        "s.sh: line 2: ?what: command not found\ns.sh: line 4: nope2: command not found\n./lib.sh: line 2: nope3: command not found\n"
    );
    assert_eq!(run(&["-c", "echo; nope4"], ""), "neosh: line 1: nope4: command not found\n");
    assert_eq!(run(&[], "echo\nnope5\n"), "neosh: line 2: nope5: command not found\n");
    let _ = std::fs::remove_dir_all(&dir);
}
