# neosh

A shell, as bash, zsh and fish are, with navi inside it. Its language is
bash's, so your commands and scripts keep working; a line starting with `?`
goes to navi instead, which knows what you have just been running. A space
after the `?` is up to you; a glob such as `?*`, or `?.txt` naming a file,
stays a glob.

```
~/proj $ npm run build
… error TS2304: Cannot find name 'Foo'.
~/proj $ ?why did that fail
```

navi is told the commands you ran since it last answered, with where they ran
and how they ended, and answers in your terminal, in the shell's directory,
asking before it changes anything. To show navi what a command printed, its errors too, pipe it to `?`:

```
~/proj $ npm test |?why does this fail
~/proj $ git log -5 |?
```

When the answer is a command to run,
the next prompt starts with it typed out: press Enter to run it, change it
first, or clear it with ^C. Right after a command fails, `?` alone asks
navi why and how to fix it, and once navi has answered the prompt shows no
failure: `$?` after a `?` line is navi's (130 if you stopped it with ^C); otherwise it says how to ask. Every line
without a `?` is the shell's and runs as bash runs it, so Enter never does
anything bash would not. If you forget the `?` and a sentence comes back
`command not found`, the next prompt starts with `?` and your sentence:
Enter asks navi, ^C clears it. All of a shell's conversation with navi is
one session, whichever directory you ask from; which model answers is said
with its first answer, and again only when it changes.

### What you did

navi knows what your commands printed, without you pasting it. neosh runs
inside a recorder of its own that copies everything between your terminal
and the shell untouched (colours, ^C, ^Z, vim and less work as always) and
keeps each command's output in a file, with an index of every command in
the terminal. navi is told where they are, not what they say, and reads
one only when your question needs it:

```
~/proj $ python3 app.py
Traceback (most recent call last):
…
? to ask why
~/proj 1 $ ?what is the exact error
· read_file({"path":"/…/neosh-rec-62946/62947-1.txt"})
```

A long output keeps its first 1,000 and latest 10,000 lines; what a
full-screen program shows is not kept; what you type into python or psql
is. Each tmux pane is recorded on its own, and `ssh host` is a command like
any other. The files are yours only and go when the terminal closes.
Obvious secrets (tokens, `password=…`, private keys) are blanked as they
are written, and nothing leaves your machine unless navi reads it for a
question. A line typed with a space first is not recorded;
`NEOSH_RECORD=off` stops recording until `NEOSH_RECORD=on`, and set in the
environment before neosh starts, there is no recorder at all. The first
three failures of a session say `? to ask why`; `NEOSH_HINT=always` or
`off` changes that.

`NEOSH_ASK=sentences` (in `~/.neoshrc` or at the prompt) also sends a
sentence whose first word is no command straight to navi (`show me the big
files`); `NEOSH_ASK=off` sends nothing.

The interpreter is [brush](https://github.com/reubeno/brush)'s, a
bash-compatible shell in Rust: pipelines, redirections, here-documents,
functions, arrays, `[[ … ]]`, aliases, job control, bash's builtins and its
history expansion (`sudo !!`, `!$`, `^old^new`, and modifiers such as `!$:h` and `!!:gs/a/b/`); `cd` looks in `CDPATH`,
and `shopt -s autocd cdspell` lets you type `..` or a folder's name to go
there and puts small typos in a `cd` right. `COLUMNS` and `LINES` follow the
terminal.
What neosh adds is specified in [spec.md](spec.md).

The prompt shows where you are, the git branch (with `*` when tracked files
have changes), how long a command took when
it was 5 seconds or more, and the status of one that failed, in red
(`~/proj main* 1m 4s 1 $`); navi is told the time too. Set `PS1` in `~/.neoshrc` for your
own, and `NO_COLOR` for none. zsh's hooks work too: a `precmd` or
`preexec` function, or ones named in `precmd_functions` and
`preexec_functions` (as `add-zsh-hook` and bash-preexec tools use), run before
each prompt and each command and never change `$?`; `PROMPT_COMMAND`,
`trap … DEBUG` and `bind -x` work as in bash, so tools such as starship,
zoxide, fzf and direnv can use their bash setup. History is kept in `~/.neosh_history`
(or the file `HISTFILE` names), so ↑ and ^R reach earlier sessions;
`HISTCONTROL=ignoreboth` and `HISTIGNORE` work as in bash, so a line typed
with a space first stays out of it. Like bash's, it holds the last 500
lines unless `HISTSIZE` and `HISTFILESIZE` say otherwise (`-1` keeps
everything). The
window title shows the directory, or the command running; Terminal and iTerm
open a new tab in the same directory; and VS Code's terminal marks each
command.

## Install

You need [Rust](https://rustup.rs) to build it and [navi](https://github.com/Jeanno/neoai#navi-in-the-terminal)
on your `$PATH` to ask anything. One line, no checkout needed:

```bash
curl -fsSL https://raw.githubusercontent.com/Jeanno/neosh/main/install.sh | sh
```

From a checkout, `./install.sh` builds the code you have. Either way
you get `neosh` in `~/.local/bin`; run the installer again to update, or pass
`--uninstall` to remove it (`… | sh -s -- --uninstall`). Then type `neosh`.

## Setup

The first time neosh starts with no `~/.neoshrc`, it offers setup: yes,
later (asked again next time) or never. `neosh --setup` runs it any time.
Two steps, each skippable:

1. **The model** navi answers with: it runs `navi setup`.
2. **The look**: neosh's own, or your zsh's. neosh asks your zsh for its
   prompt and translates it to `PS1` (colours, `%n@%m`, `%~`, the time and
   the rest); a starship prompt becomes starship's bash prompt. A prompt
   bash cannot draw (powerlevel10k, or one that runs commands as it is
   drawn) is named and neosh's own prompt kept. `CLICOLOR`, `LSCOLORS` and
   `LS_COLORS` come too.

It shows what it will write, then writes only between
`# >>> neosh setup >>>` and `# <<< neosh setup <<<` in `~/.neoshrc`,
leaving the rest as it was. Without a terminal:
`neosh --setup --look zsh --yes`.

`neosh -c '…'`, `neosh script.sh` and commands on stdin work as in bash.
An interactive neosh reads `~/.neoshrc`; `-l` (or starting as `-neosh`)
also reads `~/.profile`. neosh does not add itself to `/etc/shells` or
change your login shell: try it from the shell you use now first. The
installer ends by saying how, for when you are ready.

```bash
cargo test
```

`cargo test` also drives neosh on a pseudo-terminal (`tests/drive.py`, which
needs python3) with a stand-in navi, end to end.
