# neosh

**A bash shell with an agent living inside it.** navi sees your terminal as
you do: what you ran, where, how it ended and what it printed. Start a line
with `?` to hand it any task. It can read and edit files, run commands and
search the web, and it asks before it changes anything.

```
~/proj $ ?set up a venv and install what app.py needs
│ · read app.py
│ · run python3 -m venv .venv && .venv/bin/pip install requests
│ Done: **requests** is installed in `.venv`. Activate it with:

    source .venv/bin/activate

~/proj $ source .venv/bin/activate      ← typed for you; Enter runs it
```

It picks up where you are. After a command fails, `?` alone asks why. To
point it at a command's output, ask about it (`?what was the exact error`)
or pipe it in (`npm test |?`).

Every other line is plain bash, run by [brush](https://github.com/reubeno/brush),
so your commands and scripts keep working and Enter never does anything bash
would not.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/Jeanno/neosh/main/install.sh | sh
```

This puts `neosh` in `~/.local/bin`: the prebuilt one from the latest
[release](https://github.com/Jeanno/neosh/releases) for macOS (arm64 or
Intel) or Linux (x86_64 or arm64), its sha256 checked. With no release for
your machine, or with `-s -- --source`, it builds neosh from source, which
needs [Rust](https://rustup.rs) and git. From a checkout, `./install.sh`
builds the code you have.

[navi](https://github.com/Jeanno/neoai#navi-in-the-terminal), which needs
Node 22 or newer, is put beside it when it isn't there yet
(`NEOSH_INSTALL_NAVI=no` leaves it out). Run the installer again to update,
or add `-s -- --uninstall` to remove neosh. Then type `neosh`. It doesn't
change your login shell; the installer says how, for when you are ready.

The first start offers setup (`neosh --setup` any time): the model navi
answers with, and whether to keep your zsh prompt and colours. It writes
only between the `# >>> neosh setup >>>` markers in `~/.neoshrc`.

## Asking navi

- `?<anything>` is a task or a question: `?find the big files here`,
  `?rename these photos by date`, `?why did that fail`.
- `?` alone starts **navi mode**: every line goes to navi, no `?` needed,
  until Esc (or `exit`) takes you back to the shell. Right after a failure,
  it first asks why.
- `cmd |?question` hands navi the output too: `npm test |?why does this fail`.
- If the answer ends with a command, it is typed at your next prompt: Enter
  runs it, ^C clears it.
- Forgot the `?` and got `command not found`? The next prompt offers your
  sentence to navi.
- The answer is drawn in a dim `│` gutter, with code blocks outside it so
  they copy clean. navi asks before it changes anything.
- navi never holds you up: ^Z while it works sends it to the background and
  gives you the prompt. It notifies you when it's done or needs a yes or
  no, and `?` alone brings it back with its answer.
- One conversation per shell; `?/resume` picks up one an earlier shell had.
  `NEOSH_ASK=sentences` also sends lines that
  start with no known command; `NEOSH_ASK=off` sends nothing.

## What gets recorded

neosh keeps each command's output in a file, so navi can read it when a
question needs it, without you pasting it. navi is told where the files are,
not what they hold.

- Your terminal works as always (colours, ^C, ^Z, vim, less).
- Files are readable by you only and are deleted when the terminal closes.
  Obvious secrets (tokens, `password=…`, private keys) are blanked.
- Full-screen programs aren't kept, and a long output keeps its first 1,000
  and last 10,000 lines.
- To leave a line out, type a space first. `NEOSH_RECORD=off` pauses
  recording; set it before neosh starts to turn the recorder off.

## Shell

- bash's language and builtins: job control, history expansion (`!!`,
  `^old^new`), `CDPATH`, `shopt -s autocd cdspell`.
- The prompt shows the directory, git branch, slow-command time and failed
  status. Set `PS1` in `~/.neoshrc` to change it.
- Plugins named in `~/.neoshrc`: `plugins=(z ~/my/plugin owner/repo)`, with `z`
  built in.
- `precmd`/`preexec` hooks, `PROMPT_COMMAND`, `trap DEBUG` and `bind -x`
  work, so starship, zoxide, fzf and direnv use their bash setup.
- History lives in `~/.neosh_history`; `HISTCONTROL`, `HISTIGNORE` and
  `HISTSIZE` work as in bash.
- `neosh -c`, scripts and stdin work as in bash. `-l` also reads `~/.profile`.

All the details are in [spec.md](spec.md).

## Development

```bash
cargo test
```

This includes end-to-end tests that drive neosh on a pseudo-terminal
(`tests/drive.py`, which needs python3) with a stand-in navi.

### Cutting a release

Set `version` in `Cargo.toml` (and run `cargo build` so `Cargo.lock`
follows), commit, then push a tag of that version:

```bash
git tag v0.1.0 && git push origin v0.1.0
```

`.github/workflows/release.yml` builds neosh for macOS (arm64, x86_64) and
Linux (x86_64, arm64), and makes a GitHub Release of
`neosh-<target>.tar.gz` and its `.sha256`, which `install.sh` downloads.
A tag that is not `Cargo.toml`'s version builds nothing.
