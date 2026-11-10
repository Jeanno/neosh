//! NSH-62: navi on a terminal of its own, which neosh shows while navi has
//! the screen and holds while ^Z has sent it to the background, so navi
//! works on while the shell takes commands.

use std::io::Write as _;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// neoai CLI-48: what navi writes as a question opens, and as it closes.
const QUESTION: &[u8] = b"\x1b]6974;Q\x07";
const ANSWERED: &[u8] = b"\x1b]6974;A\x07";
const CTRL_Z: u8 = 0x1a;
/// What is held beyond this loses its oldest part.
const HOLD: usize = 4 << 20;

/// What navi wrote, as neosh deals with it.
#[derive(Default)]
struct Screen {
    shown: bool,
    held: Vec<u8>,
    /// The start of a mark, split from the rest of it by a read.
    pending: Vec<u8>,
    asking: bool,
}

/// navi, answering on its own terminal.
pub struct Navi {
    child: Child,
    master: OwnedFd,
    screen: Arc<Mutex<Screen>>,
    drained: mpsc::Receiver<()>,
    status: Option<u8>,
    /// The question asked, and the file for the command it suggests (NSH-11).
    pub ask: String,
    pub handed: PathBuf,
    /// Whether the prompt has said it waits for an answer, this time.
    pub told: bool,
}

pub enum Ended {
    Done(u8),
    Away,
}

pub enum Failed {
    /// No terminal to give navi: run it as before.
    NoTerminal(Command),
    Spawn(std::io::Error),
}

/// Takes the marks out of what navi wrote, `pending` keeping the start of
/// one a read split; whether a question is open after it, if it said.
fn unmark(pending: &mut Vec<u8>, chunk: &[u8]) -> (Vec<u8>, Option<bool>) {
    let mut bytes = std::mem::take(pending);
    bytes.extend_from_slice(chunk);
    let mut out = Vec::with_capacity(bytes.len());
    let mut asking = None;
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        if let Some((mark, open)) = [(QUESTION, true), (ANSWERED, false)].into_iter().find(|(m, _)| rest.starts_with(m)) {
            asking = Some(open);
            i += mark.len();
        } else if QUESTION.starts_with(rest) || ANSWERED.starts_with(rest) {
            pending.extend_from_slice(rest);
            break;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    (out, asking)
}

fn show(bytes: &[u8]) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(bytes);
    let _ = out.flush();
}

fn size() -> Option<libc::winsize> {
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: TIOCGWINSZ fills the winsize given.
    (unsafe { libc::ioctl(0, libc::TIOCGWINSZ, &mut size) } == 0).then_some(size)
}

/// Starts navi's `command` on a terminal of its own, the size of neosh's,
/// with navi's answer shown as it comes until `attach` says otherwise.
pub fn start(mut command: Command, ask: &str, handed: PathBuf) -> Result<Navi, Failed> {
    let mut modes: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: tcgetattr fills the termios given.
    let has_modes = unsafe { libc::tcgetattr(0, &mut modes) } == 0;
    let Some(mut size) = size().filter(|_| has_modes) else {
        return Err(Failed::NoTerminal(command));
    };
    let (mut master, mut slave) = (0, 0);
    // SAFETY: openpty on valid out-pointers, the modes and size copied from ours.
    if unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), &mut modes, &mut size) } != 0 {
        return Err(Failed::NoTerminal(command));
    }
    // SAFETY: openpty gave us both; navi gets only its own side.
    let (master, slave) = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) };
    let ends = (slave.try_clone(), slave.try_clone());
    let (Ok(input), Ok(output)) = ends else {
        return Err(Failed::NoTerminal(command));
    };
    command.stdin(Stdio::from(input)).stdout(Stdio::from(output)).stderr(Stdio::from(slave));
    // SAFETY: only async-signal-safe calls between fork and exec.
    unsafe {
        command.pre_exec(|| {
            // Its terminal's own session, so ^C there reaches navi and what it runs.
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command.spawn().map_err(Failed::Spawn)?;
    // Its ends of the terminal go with the command, so navi's going ends the reads.
    drop(command);
    let reading = master.try_clone().map_err(Failed::Spawn)?;
    let screen = Arc::new(Mutex::new(Screen { shown: true, ..Screen::default() }));
    let (done, drained) = mpsc::channel();
    let writes = screen.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            // SAFETY: reading into a buffer of the length given.
            let n = unsafe { libc::read(reading.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            if n <= 0 {
                break;
            }
            let mut s = writes.lock().unwrap_or_else(|e| e.into_inner());
            let (bytes, asked) = unmark(&mut s.pending, &buf[..n as usize]);
            if let Some(open) = asked {
                s.asking = open;
            }
            if s.shown {
                show(&bytes);
            } else {
                s.held.extend_from_slice(&bytes);
                if s.held.len() > HOLD {
                    let over = s.held.len() - HOLD;
                    s.held.drain(..over);
                }
            }
        }
        let mut s = writes.lock().unwrap_or_else(|e| e.into_inner());
        let rest = std::mem::take(&mut s.pending);
        if s.shown { show(&rest) } else { s.held.extend(rest) }
        let _ = done.send(());
    });
    Ok(Navi { child, master, screen, drained, status: None, ask: ask.to_string(), handed, told: false })
}

impl Navi {
    /// How navi ended, once it has.
    pub fn finished(&mut self) -> Option<u8> {
        if self.status.is_none() {
            if let Ok(Some(ended)) = self.child.try_wait() {
                self.status = Some(crate::navi::status_of(ended));
            }
        }
        self.status
    }

    /// Whether a question is open that nothing has been typed at since.
    pub fn asking(&self) -> bool {
        self.screen.lock().map(|s| s.asking).unwrap_or(false)
    }

    /// Done, while away: how it ended and all it wrote, once read to its end.
    pub fn done(&mut self) -> Option<(u8, Vec<u8>)> {
        let status = self.finished()?;
        let _ = self.drained.recv_timeout(Duration::from_millis(500));
        let held = self.screen.lock().map(|mut s| std::mem::take(&mut s.held)).unwrap_or_default();
        Some((status, held))
    }

    /// navi has the screen: what it held is shown, keys go to it, until it
    /// is done or ^Z sends it away again.
    pub fn attach(&mut self) -> Ended {
        if let Ok(mut s) = self.screen.lock() {
            let held = std::mem::take(&mut s.held);
            show(&held);
            s.shown = true;
        }
        self.told = false;
        let mut modes: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: tcgetattr and tcsetattr on our own terminal, with a termios they filled.
        let saved = (unsafe { libc::tcgetattr(0, &mut modes) } == 0).then_some(modes);
        if let Some(mut raw) = saved {
            unsafe {
                libc::cfmakeraw(&mut raw);
                libc::tcsetattr(0, libc::TCSANOW, &raw);
            }
        }
        let mut last = size();
        let ended = loop {
            if let Some(status) = self.finished() {
                break Ended::Done(status);
            }
            // The terminal resized: navi's too, which tells it so.
            let now = size();
            if let (Some(s), true) = (now, now.map(|n| (n.ws_row, n.ws_col)) != last.map(|l| (l.ws_row, l.ws_col))) {
                unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &s) };
                last = now;
            }
            let mut ready = libc::pollfd { fd: 0, events: libc::POLLIN, revents: 0 };
            // SAFETY: one pollfd, then a read into a buffer of the length given.
            if unsafe { libc::poll(&mut ready, 1, 100) } <= 0 || ready.revents & libc::POLLIN == 0 {
                continue;
            }
            let mut keys = [0u8; 1024];
            let n = unsafe { libc::read(0, keys.as_mut_ptr().cast(), keys.len()) };
            if n <= 0 {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            let keys = &keys[..n as usize];
            let (to_navi, away) = match keys.iter().position(|&k| k == CTRL_Z) {
                Some(at) => (&keys[..at], true),
                None => (keys, false),
            };
            if !to_navi.is_empty() {
                if let Ok(mut s) = self.screen.lock() {
                    s.asking = false;
                }
                // SAFETY: writing the keys given, from a slice of their length.
                unsafe { libc::write(self.master.as_raw_fd(), to_navi.as_ptr().cast(), to_navi.len()) };
            }
            if away {
                break Ended::Away;
            }
        };
        if let Ended::Done(_) = ended {
            let _ = self.drained.recv_timeout(Duration::from_millis(500));
        }
        if let Ok(mut s) = self.screen.lock() {
            s.shown = false;
        }
        if let Some(saved) = saved {
            unsafe { libc::tcsetattr(0, libc::TCSANOW, &saved) };
        }
        ended
    }
}

// Closing neosh stops navi, and what it runs.
impl Drop for Navi {
    fn drop(&mut self) {
        if self.finished().is_none() {
            unsafe { libc::kill(-(self.child.id() as i32), libc::SIGHUP) };
            let _ = self.child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_s_mark_is_taken_out_even_split_across_reads() {
        let mut pending = Vec::new();
        assert_eq!(unmark(&mut pending, b"run? ls\x1b]69"), (b"run? ls".to_vec(), None));
        assert_eq!(pending, b"\x1b]69");
        assert_eq!(unmark(&mut pending, b"74;Q\x07[y/N] "), (b"[y/N] ".to_vec(), Some(true)));
        assert!(pending.is_empty());
        // Closed, by an answer or a judge: the last mark says.
        assert_eq!(unmark(&mut pending, b"\x1b]6974;A\x07\xe2\x94\x82 ok"), ("│ ok".as_bytes().to_vec(), Some(false)));
        assert_eq!(unmark(&mut pending, b"\x1b]6974;Q\x07\x1b]6974;A\x07"), (Vec::new(), Some(false)));
        // Other escapes go through untouched.
        assert_eq!(unmark(&mut pending, b"\x1b[2m\xe2\x94\x82\x1b[0m"), (b"\x1b[2m\xe2\x94\x82\x1b[0m".to_vec(), None));
        assert_eq!(unmark(&mut pending, b"\x1b]0;title\x07"), (b"\x1b]0;title\x07".to_vec(), None));
    }
}
