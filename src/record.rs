//! What you did (NSH-34 to NSH-46): a recorder between the terminal and the
//! shell, the same binary, copying everything untouched and keeping each
//! command's output in a file of its own, with an index, for navi to read.
//!
//! neosh marks where a command's output starts and ends with an OSC the
//! recorder takes out: `ESC ] 6973 ; S ; key ; id ; dir ; line [; resumes] BEL`
//! and `ESC ] 6973 ; E ; key ; id ; how BEL`, the dir and line in hex, the id
//! `<pid>-<n>` so neoshes started inside a recorded one have their own.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{IsTerminal as _, Write as _};
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The terminal a recorder records, its session key and its directory.
pub const RECORDER: &str = "NEOSH_RECORDER";
pub const KEY: &str = "NEOSH_RECORD_KEY";
pub const DIR: &str = "NEOSH_RECORD_DIR";
/// Why the recorder could not start, for `?` to say once (NSH-34).
pub const FAILED: &str = "NEOSH_RECORD_FAILED";
/// The index, for navi (NSH-41).
pub const INDEX: &str = "NAVI_SHELL_INDEX";
const MARK: &str = "6973";
/// NSH-37
pub const HEAD: u64 = 1_000;
pub const TAIL: usize = 10_000;
/// A file whose middle is being dropped is written again no more often.
const REWRITE: Duration = Duration::from_millis(500);

/// The terminal on fd 0, by name.
fn tty_name() -> Option<String> {
    // SAFETY: ttyname returns a pointer to a static buffer or null.
    let p = unsafe { libc::ttyname(0) };
    (!p.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

/// NSH-35: whether a recorder already records this terminal.
pub fn recorded() -> bool {
    let wanted = std::env::var(RECORDER).ok();
    wanted.is_some() && wanted == tty_name() && std::env::var_os(KEY).is_some()
}

/// NSH-34, NSH-43: for an interactive neosh on a terminal not yet recorded,
/// become the recorder and run neosh inside it; its status when it ends. None
/// to carry on as the shell, recorded already, unwanted, or not possible.
pub fn wrap(argv: &[String]) -> Option<i32> {
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) || recorded() {
        return None;
    }
    if std::env::var("NEOSH_RECORD").is_ok_and(|v| v == "off") {
        return None;
    }
    match Recorder::start(argv) {
        Ok(recorder) => Some(recorder.relay()),
        Err(e) => {
            // SAFETY: before any thread is started.
            unsafe { std::env::set_var(FAILED, e.to_string()) };
            None
        }
    }
}

struct Recorder {
    master: OwnedFd,
    child: std::process::Child,
    tape: Tape,
    saved: Option<libc::termios>,
}

static mut WAKE: RawFd = -1;

extern "C" fn woken(signal: libc::c_int) {
    // SAFETY: write is async-signal-safe; WAKE is set before the handler is.
    unsafe {
        let _ = libc::write(WAKE, [signal as u8].as_ptr().cast(), 1);
    }
}

fn last_error(what: &str) -> std::io::Error {
    let e = std::io::Error::last_os_error();
    std::io::Error::new(e.kind(), format!("{what}: {e}"))
}

impl Recorder {
    fn start(argv: &[String]) -> std::io::Result<Recorder> {
        let dir = make_dir()?;
        let key = random_key();
        let (mut master, mut slave) = (0, 0);
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        // SAFETY: plain ioctl and openpty on valid out-pointers.
        unsafe {
            libc::ioctl(0, libc::TIOCGWINSZ, &mut size);
            // Opening one can fail now and then while many open at once.
            let mut tries = 0;
            while libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) != 0 {
                tries += 1;
                if tries == 5 {
                    let _ = std::fs::remove_dir_all(&dir);
                    return Err(last_error("no pseudo-terminal"));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        // SAFETY: openpty gave us both; the shell gets only its own side.
        let (master, slave) = unsafe {
            libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC);
            (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave))
        };
        let name = {
            let p = unsafe { libc::ttyname(slave.as_raw_fd()) };
            if p.is_null() {
                return Err(last_error("no name for the pseudo-terminal"));
            }
            unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned()
        };
        let exe = std::env::current_exe()?;
        let mut command = std::process::Command::new(exe);
        command
            .arg0(&argv[0])
            .args(&argv[1..])
            .env(RECORDER, &name)
            .env(KEY, &key)
            .env(DIR, &dir)
            .env(INDEX, dir.join("index.txt"))
            .stdin(slave.try_clone()?)
            .stdout(slave.try_clone()?)
            .stderr(slave);
        use std::os::unix::process::CommandExt as _;
        // SAFETY: only async-signal-safe calls between fork and exec.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().map_err(|e| {
            let _ = std::fs::remove_dir_all(&dir);
            std::io::Error::new(e.kind(), format!("neosh could not start inside it: {e}"))
        })?;
        // Once the shell is starting, so it does not wait for it.
        std::thread::spawn(|| sweep(&std::env::temp_dir()));
        Ok(Recorder { master, child, tape: Tape::new(dir, key), saved: None })
    }

    /// Copy between the terminal and the shell until the shell ends.
    fn relay(mut self) -> i32 {
        let mut pipe = [0; 2];
        // SAFETY: the signal handlers only write to the pipe.
        unsafe {
            libc::pipe(pipe.as_mut_ptr());
            for fd in pipe {
                libc::fcntl(fd, libc::F_SETFL, libc::fcntl(fd, libc::F_GETFL) | libc::O_NONBLOCK);
                libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            }
            WAKE = pipe[1];
            for signal in [libc::SIGWINCH, libc::SIGCHLD, libc::SIGHUP, libc::SIGTERM] {
                libc::signal(signal, woken as *const () as libc::sighandler_t);
            }
            for signal in [libc::SIGINT, libc::SIGQUIT, libc::SIGTSTP, libc::SIGTTOU, libc::SIGTTIN] {
                libc::signal(signal, libc::SIG_IGN);
            }
            // Keys go to the shell as they are typed: ^C and ^Z included.
            let mut raw: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut raw) == 0 {
                self.saved = Some(raw);
                libc::cfmakeraw(&mut raw);
                libc::tcsetattr(0, libc::TCSANOW, &raw);
            }
        }
        let master = self.master.as_raw_fd();
        let mut buf = vec![0u8; 65536];
        let mut stdout = std::io::stdout().lock();
        let mut hung_up = false;
        let mut told = false;
        let status = loop {
            let mut fds = [
                libc::pollfd { fd: 0, events: if hung_up { 0 } else { libc::POLLIN }, revents: 0 },
                libc::pollfd { fd: master, events: libc::POLLIN, revents: 0 },
                libc::pollfd { fd: pipe[0], events: libc::POLLIN, revents: 0 },
            ];
            // SAFETY: three valid pollfds.
            let ready = unsafe { libc::poll(fds.as_mut_ptr(), 3, 250) };
            if ready < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                break self.finish();
            }
            if fds[2].revents & libc::POLLIN != 0 {
                loop {
                    let n = unsafe { libc::read(pipe[0], buf.as_mut_ptr().cast(), buf.len()) };
                    if n <= 0 {
                        break;
                    }
                    for &signal in &buf[..n as usize] {
                        match signal as libc::c_int {
                            libc::SIGWINCH => self.resize(),
                            libc::SIGHUP => hung_up = true,
                            libc::SIGTERM => self.tell(libc::SIGTERM),
                            _ => {}
                        }
                    }
                }
            }
            if fds[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                let n = unsafe { libc::read(master, buf.as_mut_ptr().cast(), buf.len()) };
                if n <= 0 {
                    break self.finish();
                }
                let shown = self.tape.feed(&buf[..n as usize]);
                if stdout.write_all(&shown).and_then(|()| stdout.flush()).is_err() {
                    hung_up = true;
                }
            }
            if fds[0].revents & libc::POLLIN != 0 {
                let n = unsafe { libc::read(0, buf.as_mut_ptr().cast(), buf.len()) };
                if n <= 0 {
                    hung_up = true;
                } else {
                    let _ = write_all(master, &buf[..n as usize]);
                }
            } else if fds[0].revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                hung_up = true;
            }
            if hung_up && !told {
                // NSH-42: the terminal closed; the shell is told as it would be.
                told = true;
                self.tell(libc::SIGHUP);
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                self.drain(&mut stdout);
                break code(status);
            }
            self.tape.tick();
        };
        drop(stdout);
        status
    }

    /// A signal for the shell and what it runs, as the terminal would send it.
    fn tell(&self, signal: libc::c_int) {
        let group = self.child.id() as libc::pid_t;
        // SAFETY: the shell leads its own session and process group.
        unsafe {
            libc::kill(-group, signal);
            libc::kill(group, signal);
        }
    }

    fn resize(&self) {
        // SAFETY: the window's size, from the terminal to the shell's.
        unsafe {
            let mut size: libc::winsize = std::mem::zeroed();
            if libc::ioctl(0, libc::TIOCGWINSZ, &mut size) == 0 {
                libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &size);
            }
        }
    }

    /// What the shell printed last, before it ended.
    fn drain(&mut self, out: &mut impl std::io::Write) {
        let fd = self.master.as_raw_fd();
        unsafe { libc::fcntl(fd, libc::F_SETFL, libc::fcntl(fd, libc::F_GETFL) | libc::O_NONBLOCK) };
        let mut buf = [0u8; 65536];
        loop {
            let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
            if n <= 0 {
                break;
            }
            let shown = self.tape.feed(&buf[..n as usize]);
            let _ = out.write_all(&shown);
        }
        let _ = out.flush();
    }

    fn finish(&mut self) -> i32 {
        self.child.wait().map(code).unwrap_or(1)
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if let Some(saved) = self.saved {
            unsafe { libc::tcsetattr(0, libc::TCSANOW, &saved) };
        }
        // NSH-42: gone with the terminal.
        let _ = std::fs::remove_dir_all(&self.tape.dir);
    }
}

fn write_all(fd: RawFd, mut bytes: &[u8]) -> std::io::Result<()> {
    while !bytes.is_empty() {
        let n = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
        if n < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        bytes = &bytes[n as usize..];
    }
    Ok(())
}

fn code(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt as _;
    status.code().unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}

/// NSH-42: the directories recorders that died left behind, taken away.
/// A temporary directory can hold many thousands, so names are looked at first.
fn sweep(tmp: &Path) {
    if let Ok(entries) = std::fs::read_dir(tmp) {
        for entry in entries.flatten() {
            if !entry.file_name().to_str().is_some_and(|n| n.starts_with("neosh-rec-")) {
                continue;
            }
            let path = entry.path();
            let pid = std::fs::read_to_string(path.join("recorder.pid")).ok().and_then(|p| p.trim().parse::<i32>().ok());
            if let Some(pid) = pid {
                // SAFETY: signal 0 only asks whether the process is there.
                let gone = unsafe { libc::kill(pid, 0) } != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
                if gone {
                    let _ = std::fs::remove_dir_all(&path);
                }
            }
        }
    }
}

/// NSH-42: `$TMPDIR/neosh-rec-<pid>/`, the user's only.
fn make_dir() -> std::io::Result<PathBuf> {
    let tmp = std::env::temp_dir();
    let dir = tmp.join(format!("neosh-rec-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new().mode(0o700).create(&dir).map_err(|e| std::io::Error::new(e.kind(), format!("{}: {e}", dir.display())))?;
    std::fs::write(dir.join("recorder.pid"), std::process::id().to_string())?;
    std::fs::write(dir.join("index.txt"), "")?;
    Ok(dir)
}

fn random_key() -> String {
    let mut bytes = [0u8; 12];
    if let Ok(mut f) = File::open("/dev/urandom") {
        use std::io::Read as _;
        let _ = f.read_exact(&mut bytes);
    }
    hex(&bytes)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<String> {
    let bytes: Option<Vec<u8>> = (0..s.len()).step_by(2).map(|i| s.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
    String::from_utf8(bytes?).ok()
}

/// A command's output file, by the id neosh gave it.
pub fn file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.txt"))
}

/// The mark neosh prints as a command starts (NSH-36), `resumes` naming the
/// command `fg` brings back (NSH-40), whose id the end mark then gives.
pub fn start_mark(key: &str, id: &str, dir: &str, line: &str, resumes: Option<&str>) -> String {
    let resumes = resumes.map(|r| format!(";{r}")).unwrap_or_default();
    format!("\x1b]{MARK};S;{key};{id};{};{}{resumes}\x07", hex(dir.as_bytes()), hex(line.as_bytes()))
}

/// The mark neosh prints as a command ends: `exit N`, or `stopped %N`.
pub fn end_mark(key: &str, id: &str, how: &str) -> String {
    format!("\x1b]{MARK};E;{key};{id};{}\x07", hex(how.as_bytes()))
}

#[derive(Debug, Clone, PartialEq)]
enum State {
    Running,
    Ended(String),
}

struct Entry {
    id: String,
    dir: String,
    line: String,
    state: State,
    started: Instant,
    took: Duration,
    text: Text,
}

/// The stream from the shell: what the terminal is shown, and what is kept.
pub struct Tape {
    dir: PathBuf,
    key: String,
    parse: Parse,
    entries: Vec<Entry>,
    /// Entries being printed to, innermost last.
    open: Vec<usize>,
}

#[derive(Default)]
enum Parse {
    #[default]
    Ground,
    Esc,
    /// ESC ( and the like: one byte more.
    Charset,
    Csi(Vec<u8>),
    Osc(Vec<u8>),
    OscEsc(Vec<u8>),
}

impl Tape {
    pub fn new(dir: PathBuf, key: String) -> Tape {
        Tape { dir, key, parse: Parse::Ground, entries: Vec::new(), open: Vec::new() }
    }

    /// Takes bytes from the shell; what the terminal is shown, marks taken out.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut shown = Vec::with_capacity(bytes.len());
        for &b in bytes {
            match std::mem::take(&mut self.parse) {
                Parse::Ground => match b {
                    0x1b => self.parse = Parse::Esc,
                    _ => {
                        shown.push(b);
                        self.text(|t| t.byte(b));
                    }
                },
                Parse::Esc => match b {
                    b'[' => self.parse = Parse::Csi(Vec::new()),
                    b']' => self.parse = Parse::Osc(Vec::new()),
                    b'(' | b')' | b'*' | b'+' | b'#' => {
                        shown.extend([0x1b, b]);
                        self.parse = Parse::Charset;
                    }
                    _ => shown.extend([0x1b, b]),
                },
                Parse::Charset => shown.push(b),
                Parse::Csi(mut params) => {
                    if (0x40..=0x7e).contains(&b) {
                        shown.extend([0x1b, b'[']);
                        shown.extend(&params);
                        shown.push(b);
                        self.text(|t| t.csi(&params, b));
                    } else if params.len() < 64 {
                        params.push(b);
                        self.parse = Parse::Csi(params);
                    } else {
                        shown.extend([0x1b, b'[']);
                        shown.extend(&params);
                        shown.push(b);
                    }
                }
                Parse::Osc(mut body) => match b {
                    0x07 => self.osc(body, &[0x07], &mut shown),
                    0x1b => self.parse = Parse::OscEsc(body),
                    _ if body.len() < 4096 => {
                        body.push(b);
                        self.parse = Parse::Osc(body);
                    }
                    _ => {
                        shown.extend([0x1b, b']']);
                        shown.extend(&body);
                        shown.push(b);
                    }
                },
                Parse::OscEsc(body) => {
                    if b == b'\\' {
                        self.osc(body, &[0x1b, b'\\'], &mut shown);
                    } else {
                        shown.extend([0x1b, b']']);
                        shown.extend(&body);
                        shown.push(0x1b);
                        self.parse = Parse::Esc;
                        // The ESC starts something new.
                        let again = self.feed(&[b]);
                        shown.extend(again);
                    }
                }
            }
        }
        shown
    }

    fn text(&mut self, f: impl FnOnce(&mut Text)) {
        if let Some(&i) = self.open.last() {
            f(&mut self.entries[i].text);
        }
    }

    fn osc(&mut self, body: Vec<u8>, end: &[u8], shown: &mut Vec<u8>) {
        let text = String::from_utf8_lossy(&body);
        let mut parts = text.split(';');
        if parts.next() == Some(MARK) {
            let kind = parts.next();
            // NSH-36: a mark without the key is not one, and not shown either.
            if parts.next() == Some(self.key.as_str()) {
                let rest: Vec<String> = parts.map(str::to_string).collect();
                match kind {
                    Some("S") if rest.len() >= 3 => self.started(&rest[0], &unhex(&rest[1]).unwrap_or_default(), &unhex(&rest[2]).unwrap_or_default(), rest.get(3).map(String::as_str)),
                    Some("E") if rest.len() >= 2 => self.ended(&rest[0], &unhex(&rest[1]).unwrap_or_default()),
                    _ => {}
                }
            }
            return;
        }
        shown.extend([0x1b, b']']);
        shown.extend(&body);
        shown.extend(end);
    }

    fn started(&mut self, id: &str, dir: &str, line: &str, resumes: Option<&str>) {
        // NSH-40: `fg` writes on in the stopped command's file.
        let resumed = resumes.and_then(|r| self.entries.iter().position(|e| e.id == r && matches!(&e.state, State::Ended(h) if h.starts_with("stopped"))));
        let i = match resumed {
            Some(i) => {
                let e = &mut self.entries[i];
                e.state = State::Running;
                e.started = Instant::now() - e.took;
                i
            }
            None => {
                // A command `fg` brings back that was not recorded is kept by its name.
                let id = resumes.unwrap_or(id);
                let text = Text::new(file(&self.dir, id));
                self.entries.push(Entry { id: id.to_string(), dir: dir.to_string(), line: line.to_string(), state: State::Running, started: Instant::now(), took: Duration::ZERO, text });
                self.entries.len() - 1
            }
        };
        self.open.retain(|&o| o != i);
        self.open.push(i);
        self.write_index();
    }

    fn ended(&mut self, id: &str, how: &str) {
        let Some(i) = self.entries.iter().rposition(|e| e.id == id) else { return };
        let e = &mut self.entries[i];
        e.text.end();
        e.took = e.started.elapsed();
        e.state = State::Ended(how.to_string());
        self.open.retain(|&o| o != i);
        self.write_index();
    }

    /// Now and then: a running command's latest lines, and its file whole again.
    pub fn tick(&mut self) {
        for &i in &self.open {
            self.entries[i].text.flush(false);
        }
    }

    /// NSH-39: one line a command, rewritten as each starts and ends.
    fn write_index(&self) {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let mut out = String::from("# Commands in this terminal, oldest first: number, how it ended, how long, lines printed, directory and line, file.\n");
        for (n, e) in self.entries.iter().enumerate() {
            let how = match &e.state {
                State::Running => "running".to_string(),
                State::Ended(h) => h.clone(),
            };
            let took = if e.state == State::Running { e.started.elapsed() } else { e.took };
            let dir = crate::prompt::tilde(Path::new(&e.dir), home.as_deref());
            let line = e.line.lines().map(str::trim_end).collect::<Vec<_>>().join(" ↵ ");
            let lines = if e.text.total == 1 { "1 line".to_string() } else { format!("{} lines", e.text.total) };
            out += &format!("{} [{how}, {}, {lines}] {dir} $ {line} -> {}\n", n + 1, seconds(took), e.text.path.display());
        }
        let tmp = self.dir.join("index.txt.new");
        if std::fs::write(&tmp, out).is_ok() {
            let _ = std::fs::rename(tmp, self.dir.join("index.txt"));
        }
    }
}

fn seconds(d: Duration) -> String {
    if d < Duration::from_secs(60) { format!("{:.1}s", d.as_secs_f64()) } else { format!("{}m {}s", d.as_secs() / 60, d.as_secs() % 60) }
}

/// One command's output as text: colours and cursor movement out, a line
/// drawn again in place kept as it ended (NSH-37), the alternate screen not
/// kept (NSH-38), secrets blanked (NSH-44).
struct Text {
    path: PathBuf,
    file: Option<File>,
    line: Vec<char>,
    col: usize,
    utf8: Vec<u8>,
    alt: bool,
    in_key: bool,
    /// Lines written, of all printed.
    total: u64,
    tail: VecDeque<String>,
    head: Vec<String>,
    dropped: u64,
    stale: bool,
    rewritten: Instant,
}

impl Text {
    fn new(path: PathBuf) -> Text {
        use std::os::unix::fs::OpenOptionsExt as _;
        let file = std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(&path).ok();
        Text { path, file, line: Vec::new(), col: 0, utf8: Vec::new(), alt: false, in_key: false, total: 0, tail: VecDeque::new(), head: Vec::new(), dropped: 0, stale: false, rewritten: Instant::now() }
    }

    fn byte(&mut self, b: u8) {
        if self.alt {
            return;
        }
        match b {
            b'\n' => self.commit(),
            b'\r' => self.col = 0,
            0x08 => self.col = self.col.saturating_sub(1),
            b'\t' => {
                let to = (self.col / 8 + 1) * 8;
                while self.col < to {
                    self.put(' ');
                }
            }
            0..=0x1f | 0x7f => {}
            _ => {
                self.utf8.push(b);
                match std::str::from_utf8(&self.utf8) {
                    Ok(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        self.utf8.clear();
                        for c in chars {
                            self.put(c);
                        }
                    }
                    Err(e) if e.error_len().is_some() || self.utf8.len() >= 4 => {
                        self.utf8.clear();
                        self.put('\u{fffd}');
                    }
                    Err(_) => {}
                }
            }
        }
    }

    fn put(&mut self, c: char) {
        if self.col < self.line.len() {
            self.line[self.col] = c;
        } else {
            self.line.resize(self.col, ' ');
            self.line.push(c);
        }
        self.col += 1;
    }

    fn csi(&mut self, params: &[u8], end: u8) {
        let params = String::from_utf8_lossy(params);
        // NSH-38
        if let Some(modes) = params.strip_prefix('?') {
            if modes.split(';').any(|m| matches!(m, "1049" | "1047" | "47")) && matches!(end, b'h' | b'l') {
                let entering = end == b'h';
                if entering && !self.alt {
                    if !self.line.is_empty() {
                        self.commit();
                    }
                    self.write_line("[a full-screen program ran here; what it showed is not kept]".into());
                }
                self.alt = entering;
                if !entering {
                    self.line.clear();
                    self.col = 0;
                }
            }
            return;
        }
        if self.alt {
            return;
        }
        let n = params.split(';').next().and_then(|p| p.parse::<usize>().ok()).unwrap_or(1).max(1);
        match end {
            b'K' if matches!(params.as_ref(), "" | "0") => self.line.truncate(self.col),
            b'K' if params == "2" => {
                self.line.clear();
            }
            b'C' => self.col += n,
            b'D' => self.col = self.col.saturating_sub(n),
            b'G' => self.col = n - 1,
            _ => {}
        }
    }

    fn commit(&mut self) {
        let line: String = self.line.iter().collect::<String>().trim_end().to_string();
        self.line.clear();
        self.col = 0;
        let line = self.blank(line);
        self.write_line(line);
    }

    fn write_line(&mut self, line: String) {
        self.total += 1;
        if self.total <= HEAD {
            self.head.push(line.clone());
            self.append(&line);
            return;
        }
        self.tail.push_back(line.clone());
        if self.tail.len() > TAIL {
            self.tail.pop_front();
            self.dropped += 1;
            self.stale = true;
        } else {
            self.append(&line);
        }
        if self.stale && self.rewritten.elapsed() > REWRITE {
            self.flush(true);
        }
    }

    fn append(&mut self, line: &str) {
        if let Some(f) = self.file.as_mut() {
            let _ = writeln!(f, "{line}");
        }
    }

    /// The file whole again when lines were dropped from its middle; at the
    /// end, the line still being drawn too.
    fn flush(&mut self, force: bool) {
        if self.stale && (force || self.rewritten.elapsed() > REWRITE) {
            let mut out = self.head.join("\n");
            out += &format!("\n[… {} lines not kept …]\n", self.dropped);
            for line in &self.tail {
                out += line;
                out.push('\n');
            }
            let tmp = self.path.with_extension("new");
            if std::fs::write(&tmp, out).is_ok() && std::fs::rename(&tmp, &self.path).is_ok() {
                self.file = std::fs::OpenOptions::new().append(true).open(&self.path).ok();
            }
            self.stale = false;
            self.rewritten = Instant::now();
        }
    }

    fn end(&mut self) {
        if !self.line.is_empty() {
            self.commit();
        }
        self.alt = false;
        self.flush(true);
    }

    /// NSH-44: obvious secrets blanked; private keys line by line.
    fn blank(&mut self, line: String) -> String {
        if line.contains("-----BEGIN") && line.contains("PRIVATE KEY") {
            self.in_key = true;
            return line;
        }
        if self.in_key {
            if line.contains("-----END") {
                self.in_key = false;
                return line;
            }
            return "[private key line blanked]".into();
        }
        blank_secrets(&line)
    }
}

/// NSH-44: keys and tokens by their shapes, and the values of `password=`,
/// `token:` and the like, as `[blanked]`.
pub fn blank_secrets(line: &str) -> String {
    static SHAPES: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let shapes = SHAPES.get_or_init(|| {
        regex::Regex::new(concat!(
            r"(?i)(?P<named>(?:password|passwd|pwd|token|secret|api[_-]?key|access[_-]?key)\s*[=:]\s*)[^\s'\x22]+",
            r"|(?P<bearer>bearer\s+)[A-Za-z0-9._~+/=-]{8,}",
            r"|\bsk-[A-Za-z0-9_-]{16,}",
            r"|\bgh[pousr]_[A-Za-z0-9]{20,}",
            r"|\bgithub_pat_[A-Za-z0-9_]{20,}",
            r"|\bAKIA[0-9A-Z]{16}\b",
            r"|\bxox[abprs]-[A-Za-z0-9-]{10,}",
        ))
        .expect("a valid pattern")
    });
    shapes
        .replace_all(line, |c: &regex::Captures| {
            let kept = c.name("named").or_else(|| c.name("bearer")).map_or("", |m| m.as_str());
            format!("{kept}[blanked]")
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tape(name: &str) -> Tape {
        let dir = std::env::temp_dir().join(format!("neosh-tape-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tape::new(dir, "k".into())
    }

    fn read(t: &Tape, id: &str) -> String {
        std::fs::read_to_string(file(&t.dir, id)).unwrap()
    }

    // NSH-36, NSH-37
    #[test]
    fn marks_are_taken_out_and_each_command_s_output_kept() {
        let mut t = tape("marks");
        let mut shown = t.feed(b"prompt $ ls\r\n");
        shown.extend(t.feed(start_mark("k", "1-1", "/tmp", "ls", None).as_bytes()));
        // A mark split across reads, colours, a progress bar, a title.
        let split = b"\x1b[32mgreen\x1b[0m\r\n\x1b]0;title\x07 10%\r 50%\r100%\r\nlast";
        shown.extend(t.feed(&split[..9]));
        shown.extend(t.feed(&split[9..]));
        let end = end_mark("k", "1-1", "exit 0");
        shown.extend(t.feed(&end.as_bytes()[..5]));
        shown.extend(t.feed(&end.as_bytes()[5..]));
        shown.extend(t.feed(b"after\r\n"));
        let shown = String::from_utf8(shown).unwrap();
        assert_eq!(shown, "prompt $ ls\r\n\x1b[32mgreen\x1b[0m\r\n\x1b]0;title\x07 10%\r 50%\r100%\r\nlastafter\r\n");
        assert_eq!(read(&t, "1-1"), "green\n100%\nlast\n");
        let index = std::fs::read_to_string(t.dir.join("index.txt")).unwrap();
        assert!(index.contains("1 [exit 0, ") && index.contains(", 3 lines] /tmp $ ls -> "), "{index}");
    }

    // NSH-36
    #[test]
    fn a_mark_without_the_key_is_not_one() {
        let mut t = tape("key");
        let shown = t.feed(start_mark("forged", "1-1", "/", "x", None).as_bytes());
        assert!(shown.is_empty());
        assert!(t.entries.is_empty());
        assert_eq!(t.feed(b"\x1b]7;file://h/tmp\x07"), b"\x1b]7;file://h/tmp\x07");
    }

    // NSH-38, NSH-40
    #[test]
    fn the_alternate_screen_is_not_kept_and_fg_writes_on() {
        let mut t = tape("alt");
        t.feed(start_mark("k", "1-1", "/", "vim x", None).as_bytes());
        t.feed(b"before\r\n\x1b[?1049hlots of screen\x1b[?1049lafter\r\n");
        t.feed(end_mark("k", "1-1", "stopped %1").as_bytes());
        t.feed(b"not kept\r\n");
        t.feed(start_mark("k", "1-2", "/", "fg", Some("1-1")).as_bytes());
        t.feed(b"again\r\n");
        t.feed(end_mark("k", "1-1", "exit 0").as_bytes());
        assert_eq!(read(&t, "1-1"), "before\n[a full-screen program ran here; what it showed is not kept]\nafter\nagain\n");
        assert_eq!(t.entries.len(), 1);
    }

    // NSH-37
    #[test]
    fn a_long_output_keeps_its_first_and_latest_lines() {
        let mut t = tape("long");
        t.feed(start_mark("k", "1-1", "/", "seq", None).as_bytes());
        let total = HEAD as usize + TAIL + 5;
        let all: String = (1..=total).map(|i| format!("{i}\r\n")).collect();
        t.feed(all.as_bytes());
        t.feed(end_mark("k", "1-1", "exit 0").as_bytes());
        let kept = read(&t, "1-1");
        let lines: Vec<&str> = kept.lines().collect();
        assert_eq!(lines.len(), HEAD as usize + 1 + TAIL);
        assert_eq!(lines[HEAD as usize - 1], HEAD.to_string());
        assert_eq!(lines[HEAD as usize], "[… 5 lines not kept …]");
        assert_eq!(*lines.last().unwrap(), total.to_string());
    }

    // NSH-44
    #[test]
    fn obvious_secrets_are_blanked() {
        assert_eq!(blank_secrets("export TOKEN=abc123 and password: hunter2"), "export TOKEN=[blanked] and password: [blanked]");
        assert_eq!(blank_secrets("Authorization: Bearer abcdefghijkl"), "Authorization: Bearer [blanked]");
        assert_eq!(blank_secrets("key sk-abcdefghijklmnopqrstu end"), "key [blanked] end");
        assert_eq!(blank_secrets("AKIAABCDEFGHIJKLMNOP"), "[blanked]");
        assert_eq!(blank_secrets("nothing here"), "nothing here");
        let mut t = Text::new(std::env::temp_dir().join(format!("neosh-blank-{}", std::process::id())));
        assert_eq!(t.blank("-----BEGIN OPENSSH PRIVATE KEY-----".into()), "-----BEGIN OPENSSH PRIVATE KEY-----");
        assert_eq!(t.blank("b3BlbnNzaC1rZXk".into()), "[private key line blanked]");
        assert_eq!(t.blank("-----END OPENSSH PRIVATE KEY-----".into()), "-----END OPENSSH PRIVATE KEY-----");
        let _ = std::fs::remove_file(&t.path);
    }
}
