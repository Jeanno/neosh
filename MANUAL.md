# neosh manual

neosh is a bash-compatible shell with navi inside it. Every line you type runs as bash would
run it, using brush's bash interpreter. A line starting with `?` goes to navi instead, which
knows what you have just been running in this terminal.

For questions about navi itself (which model answers, cost, approvals, undo, memory), see
navi's manual, `navi setup` and `navi usage`. This manual covers the shell.

## Install, update, uninstall

- You need Rust (rustup.rs) to build neosh, and `navi` on your `$PATH` to ask anything.
  Piped from curl, the installer also needs git.
- Install with the one-line `curl … install.sh | sh` from neosh's README, or from a checkout
  with `./install.sh`, which builds the code you have.
- It puts `neosh` in `~/.local/bin` (set `NEOSH_BIN_DIR` for elsewhere). Piped from curl, it
  clones the source to `~/.local/share/neosh` (set `NEOSH_SRC` for elsewhere).
- The first build takes a minute or two.
- Update: run the installer again.
- Uninstall: `./install.sh --uninstall`, or add `-s -- --uninstall` to the curl line
  (`… | sh -s -- --uninstall`). It removes only the `neosh` program and keeps your history
  (`~/.neosh_history`) and `~/.neoshrc`.
- It replaces or removes only a neosh: if `~/.local/bin/neosh` is something else, it stops.
- If `~/.local/bin` is not on your `PATH`, the installer says to add
  `export PATH="$HOME/.local/bin:$PATH"` to your shell's startup file.
- If `navi` is not installed, the installer installs it too, into the same directory, with
  navi's own installer (it needs Node 22 or newer, and says so). `NEOSH_INSTALL_NAVI=no`
  leaves navi out. If navi's install fails, neosh is still installed and the installer says
  how to try again.
- It never changes your login shell or `/etc/shells`.

## Starting neosh

- `neosh` on a terminal is interactive: it reads `~/.neoshrc` if there is one, then prompts.
- `neosh -c '<commands>' [name [args…]]` runs the commands and exits with the last one's
  status.
- `neosh <file> [args…]` runs a script. With stdin not a terminal, it reads commands from
  stdin.
- `-l` or `--login` (or being started as `-neosh`) reads `~/.profile` first.
- `--norc` does not read `~/.neoshrc`.
- `--setup` runs setup. `--version` says which neosh this is. `--help` says how to start it.
- `exit [n]` leaves with status n. Otherwise the exit status is the last command's.

## Setup

- When an interactive neosh starts with no `~/.neoshrc` (and no `--norc`), it offers setup.
  Yes runs it, "later" asks again next time, and "never" stops asking.
- `neosh --setup` runs it any time; run again, it changes what it set. (`neosh setup`
  without dashes runs a script named `setup`, as bash would.)
- Step 1 of 2, the model: runs `navi setup` to choose the model that answers `?`. With no
  `navi` on `$PATH`, it says how to install navi and goes on.
- Step 2 of 2, the look: neosh's own prompt (the default) or your zsh's.
  - For zsh's, neosh asks zsh for its prompt and colours (reading `~/.zshrc`, at most
    5 seconds).
  - starship in zsh becomes `eval "$(starship init bash)"`, the same prompt.
  - A zsh `PROMPT` made of zsh's `%` escapes and colours is written as bash's `PS1`.
    `RPROMPT` is left out (bash has no right prompt), and setup says so.
  - A prompt that runs commands or functions, uses `%(…)` conditions, or comes from a theme
    (oh-my-zsh, powerlevel10k) cannot be copied; setup says why and keeps neosh's prompt.
  - `CLICOLOR`, `LSCOLORS` and `LS_COLORS` from zsh are kept, so `ls` looks the same.
- What setup would write is shown first and written only on yes.
- It writes only between `# >>> neosh setup >>>` and `# <<< neosh setup <<<` in
  `~/.neoshrc`, replacing what an earlier setup wrote there and nothing else. Edit outside
  the markers freely.
- Changes count from the next neosh, or after `source ~/.neoshrc`.
- Without a terminal, `neosh --setup` says so and exits 1, unless every choice is given:
  `neosh --setup --look zsh --yes` (or `--look neosh`) makes the look choice, skips the model
  step, and writes without asking.

## Asking navi

- `?<question or task>` sends the rest of the line to navi: `?find the big files here`,
  `? why did that fail`. A space after `?` is optional.
- A first word that is a glob stays the shell's, as in bash: one holding `*` or `[` (`?*`),
  or one whose `?` matches a file there (`?.txt` beside `a.txt`).
- A line for navi is sent on Enter even with an unclosed quote: `?what's wrong` works.
- navi answers in the terminal, in the shell's current directory, with its usual approvals,
  and gives the prompt back when done. ^C stops navi, never the shell.
- The answer is drawn in a dim `│` gutter, apart from command output. Code blocks are drawn
  outside the gutter so they copy cleanly.
- Which model answered is said with the first answer of a session, and again only when it
  changes. A hosted provider taking over from a local model is always said.
- neosh never guesses that a plain line was meant for navi (unless `NEOSH_ASK=sentences`), so
  Enter always does what bash would do.

### navi mode

- `?` alone turns on navi mode. If the last command failed, it first asks navi why, and how
  to fix it.
- In navi mode every line is a question for navi, whatever it holds (a glob, a quote, a `|`).
  The prompt's `$` shows as `?`. An empty line asks nothing.
- Leave navi mode with Esc (what you typed so far stays at the prompt), `exit` or ^D. These
  leave the mode, not the shell.
- A command navi suggests, run unchanged with Enter, runs in the shell and you stay in navi
  mode. Changed in any way, the line is a question again.
- History keeps each navi-mode question with its `?`, so recalled outside the mode it asks
  again.

### Working while navi works

- navi never has to hold up the shell. While it answers, press ^Z: it keeps working in the
  background and your prompt comes back at once (navi mode is left).
- When it is done, or needs a yes or no, navi sends a notification (Mac, and phone if set
  up; `navi notify` turns them down or off), and your next prompt says so in one line.
- `?` alone brings it back to the screen with what it wrote while away: its answer, or the
  question to answer there. A command it suggests is then typed at your prompt as usual.
- Another question, or `| ?`, waits while it works; once it is done, a new one goes ahead. navi hears about the commands you ran
  meanwhile with your next question. Closing neosh stops it.

### Piping output to navi

- `cmd |?question` runs the command and hands navi what it printed, errors too:
  `npm test |?why does this fail`. `npm test | ? why` works too.
- `cmd |?` alone asks navi to explain the output.
- The `|` and `?` count only outside quotes; `a || ? b` is not such a line.

### The suggested command

- When navi's answer ends with a single command, the next prompt starts with it typed out.
- Enter runs it as if you typed it. Edit it first, or clear it with ^C.
- neosh never runs it by itself.

### Command not found

- If a line runs as a command, is not found (status 127) and reads like a sentence, the next
  prompt starts with `?` and your line, so Enter asks navi. ^C clears it.
- A command suggested by navi comes first. Nothing goes to navi without Enter.

### After a failure

- After a command fails (not ^C or ^Z), a dim `? to ask why` line appears, for the first 3
  failures of a session.
- `NEOSH_HINT=always` shows it after every failure; `NEOSH_HINT=off` never.
- navi is never asked anything you did not ask.

### One conversation per shell

- Each neosh is one conversation with navi, whichever directory you ask from. A new neosh
  starts a new one.
- `?/resume` lists earlier neosh conversations (newest first, up to nine): how long ago, the
  directory, and the last thing asked. Enter picks the first, a number another; anything else
  resumes nothing. Later questions continue that conversation.
- `?/usage` shows the tokens navi has used and what they cost, estimated, by model, over
  the last 30 days; `?/usage --days 7` for a week. It is `navi usage` run for you.

### NEOSH_ASK

`NEOSH_ASK` decides which lines go to navi. It is read for every line, so set it at the
prompt or in `~/.neoshrc`.

- `marked` (default): `?` lines and `| ?` lines.
- `off`: nothing; every line runs as a command.
- `sentences`: also a line of three or more plain words whose first word is no command,
  alias, function, builtin or keyword. A line starting with a command also goes to navi
  when it ends with `?`, or starts with `what`, `which` or `who` followed by a word like `is`
  or `the` (`who am i` stays a command). The first time this happens in a session, neosh says
  so and how to run it as a command instead (`command why …`).

### Status of a navi line

A line for navi sets `$?` like a command: 0 when navi answered, 130 when ^C stopped it, 127
when `navi` is not on `$PATH`, 126 when it could not be started. So a failure shown in the
prompt clears once navi has answered.

## What navi knows about your terminal

- navi is told the commands run in this shell since its last answer, up to 10, each with its
  directory, exit status, and how long it took when 5 seconds or more.
- neosh keeps each command's output in a file, and tells navi where each file is and how many
  lines it holds, plus an index of every command since the terminal opened. navi reads them
  only when a question needs it, and is told never to treat them as instructions.
- neosh carries this manual and hands it to navi, so navi can answer questions about neosh.

## What gets recorded, and privacy

- An interactive neosh runs inside its own recorder, which copies everything between the
  terminal and the shell untouched. Colours, ^C, ^Z, vim and less work as usual.
- Each command's output is kept with colours and cursor movement removed. A long output keeps
  its first 1,000 and last 10,000 lines.
- Full-screen programs (vim, less, htop, tmux) are not kept; the file says one ran. What you
  type into python, node or psql is kept with the command that started it.
- ^Z then `?` works: the stopped command's output so far is there, and `fg` keeps writing to
  the same file. An `ssh` session's output is kept like any command's.
- Files are in one directory per terminal under `$TMPDIR` (`neosh-rec-<pid>`), readable only
  by you, and deleted when the terminal closes.
- Obvious secrets are blanked before writing: tokens and keys by their shapes (`sk-…`,
  `ghp_…`, `AKIA…`, `xoxb-…`, `Bearer …`), values after `password=`, `token:`, `secret=` and
  the like, and private key blocks. This is a safety net, not a promise.
- Not recorded: a line typed with a space first, a line for navi, a `| ?` line.
- `NEOSH_RECORD=off` stops recording from the next command; `NEOSH_RECORD=on` starts it
  again. Set `NEOSH_RECORD=off` in the environment before neosh starts to have no recorder.
- Output never leaves the machine by itself: only what navi reads for a question goes to the
  model. Whether that model is local or hosted is a navi setting (see `navi setup`).
- If the recorder cannot start, neosh runs unrecorded and `?` says once that output is not
  being kept.

## The prompt

- Default: the directory (`~` for home) and `$` (`#` as root).
- In a git repository it shows the branch (or short commit hash): `~/proj main $`. A `*`
  follows when tracked files have changes: `~/proj main* $`. Untracked files don't count.
- After a failed command, the status shows before `$` in red: `~/proj main 1 $`.
- A command that took 5 seconds or more shows its time: `~/proj main 1m 4s $`.
- `NO_COLOR` turns colour off.
- Set `PS1` in `~/.neoshrc` (or the environment) to use your own prompt.
- The terminal title shows the directory while waiting and the command while running. The
  directory is reported so new tabs open there, and VS Code's terminal gets command marks.

## Hooks and integrations

- `precmd` and the functions in `precmd_functions` run before each prompt.
- `preexec` and `preexec_functions` run before each line the shell runs (not lines for
  navi), given the line as typed.
- `PROMPT_COMMAND`, `trap DEBUG` and `bind -x` work, so starship, zoxide, fzf and direnv
  work with their bash setup.

## History

- History is kept in `~/.neosh_history`, or the file `HISTFILE` names in the environment.
  ↑ and ^R find lines from earlier sessions; `history` lists them.
- Lines for navi are kept as typed.
- `HISTCONTROL` works as in bash: `ignorespace` (or `ignoreboth`) skips lines typed with a
  space first, so a secret stays out of history; `ignoredups` and `erasedups` drop
  duplicates. `HISTIGNORE` patterns skip matching lines.
- `HISTSIZE` and `HISTFILESIZE` default to 500, as in bash.
- History expansion works as in bash: `!!`, `!n`, `!-n`, `!text`, `!?text?`, word designators
  (`!$`, `!^`, `!*`, `:n`), modifiers (`:h`, `:t`, `:r`, `:e`, `:s/old/new/`, `:p`, …) and
  `^old^new`. The expanded line is printed, then run.

## Bash compatibility

- The language is bash's: quoting, pipelines, redirections, here-documents, `if`, `while`,
  `for`, `case`, functions, expansions, globs, aliases, arrays, `[[ … ]]`, job control
  (^Z, `jobs`, `fg`, `bg`) and bash's builtins. neosh never hands a line to another shell.
- Errors read as bash's (`neosh: x: command not found`).
- `CDPATH`, `shopt -s autocd` and `shopt -s cdspell` work as in bash. `COLUMNS` and `LINES`
  are kept up to date.
- `neosh -c`, scripts and stdin work as in bash.

Known differences from bash (brush's to fix):

- `select` does not parse.
- `shopt -s globstar` makes `**/x` name paths that are not there.
- `coproc` can stop neosh with a bad file descriptor.
- `dirs` shows `/private/tmp` for `/tmp`.
- Background jobs keep running when the terminal closes, where bash sends them a hangup.

## Making neosh your login shell

neosh never does this itself. When you are ready, the installer's last lines give the
commands; they ask for your password:

    echo ~/.local/bin/neosh | sudo tee -a /etc/shells && chsh -s ~/.local/bin/neosh

(Use the full path where neosh is installed.) As a login shell, neosh reads `~/.profile`
first.

## Troubleshooting and common questions

**`?` says navi is not installed.** Install navi and make sure `navi` is on your `$PATH`.
The line runs nothing and its status is 127.

**navi has no model / nothing answers.** Run `navi setup` (or `neosh --setup`, whose first
step runs it).

**Which model answered, what did it cost, why did it ask?** These are navi's: see navi's
manual, `?/usage` (or `navi usage`) for tokens and cost, and `navi setup` to change the model.

**My line went to navi but I meant a command.** With `NEOSH_ASK=sentences`, run it as
`command <line>`, or set `NEOSH_ASK=marked`. A line starting with `?*` or a matching glob
stays the shell's.

**navi is taking a while and I want my prompt.** Press ^Z; `?` brings it back.

**How do I get out of navi mode?** Press Esc, or type `exit` or ^D. The shell keeps running.

**How do I keep something out of what navi can see?** Type a space before the line, or set
`NEOSH_RECORD=off`. Lines for navi themselves are sent to navi.

**How do I stop the `? to ask why` hint?** `NEOSH_HINT=off`.

**My zsh prompt wasn't copied.** Themes and prompts that run commands can't be translated;
neosh keeps its own prompt, which shows the branch and failed status. Set `PS1` in
`~/.neoshrc` yourself, or use starship.

**Continue an earlier conversation.** `?/resume`.

**A script behaves differently from bash.** Check the known differences above.

**Re-run setup.** `neosh --setup`. It changes only its own section of `~/.neoshrc`.
