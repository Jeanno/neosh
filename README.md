# neosh

**A bash shell with AI help built in: start a line with `?` and navi
answers, already knowing what you just ran and what it printed.**

```
~/proj $ python3 app.py
json.decoder.JSONDecodeError: Expecting property name … (char 1)
? to ask why
~/proj 1 $ ?
│ · read what `python3 app.py` printed
│ The JSON in app.py is malformed: `"{bad"` is not an object …
```

Every other line is plain bash, run by [brush](https://github.com/reubeno/brush),
so your commands and scripts keep working and Enter never does anything bash
would not.

## Install

You need [Rust](https://rustup.rs) to build it and
[navi](https://github.com/Jeanno/neoai#navi-in-the-terminal) on your `$PATH`.

```bash
curl -fsSL https://raw.githubusercontent.com/Jeanno/neosh/main/install.sh | sh
```

This puts `neosh` in `~/.local/bin`; run it again to update, or add
`-s -- --uninstall` to remove it. From a checkout, `./install.sh` builds the
code you have. Then type `neosh`. It doesn't change your login shell; the
installer says how, for when you are ready.

The first start offers setup (`neosh --setup` any time): the model navi
answers with, and whether to keep your zsh prompt and colours. It writes
only between the `# >>> neosh setup >>>` markers in `~/.neoshrc`.

## Asking navi

- `?why did that fail` asks about your recent commands. `?` alone, right
  after a failure, asks why and how to fix it.
- `cmd |?question` hands navi the output too: `npm test |?why does this fail`.
- If the answer ends with a command, it is typed at your next prompt: Enter
  runs it, ^C clears it.
- Forgot the `?` and got `command not found`? The next prompt offers your
  sentence to navi.
- The answer is drawn in a dim `│` gutter, with code blocks outside it so
  they copy clean. navi asks before it changes anything.
- One conversation per shell. `NEOSH_ASK=sentences` also sends lines that
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
