# neosh

A shell, as zsh, bash and fish are: it reads commands, parses them and runs
programs, pipelines and jobs itself, in one process. Its language is bash's,
so what you know and the scripts you have keep working. navi lives inside
it: a line starting with `?` goes to navi, which knows what you have just
been running.

neosh is written in Rust, one binary. navi, and the specs its ids name
(`CLI-40`, `MOD-28`), are in [neoai](https://github.com/Jeanno/neoai). Its interpreter is
[brush](https://github.com/reubeno/brush)'s (`brush-core`, MIT), a
bash-compatible shell checked against bash by its own tests; neosh never
hands a line to another shell. What neosh adds is below.

## Starting

- NSH-1: `neosh` on a terminal is interactive: it reads `~/.neoshrc` if there
  is one, then prompts. `neosh -c '<commands>' [name [args…]]` runs the
  commands and exits with the last one's status; `neosh <file> [args…]` runs
  a script; with stdin not a terminal it reads commands from stdin. `-l`, or
  being started as `-neosh` (as a login shell is), reads `~/.profile` first.
- NSH-2: The exit status is the last command's, and `exit [n]` leaves with
  n. `--version` and `--help` say what neosh is and how to start it. A
  script that is not there says so as bash does (`neosh: x.sh: No such file
  or directory`, status 127).

- NSH-14: `install.sh` builds neosh and puts it in `~/.local/bin`
  (`NEOSH_BIN_DIR`), from the checkout it is in or, piped from curl, a clone
  in `~/.local/share/neosh`. Run again, it
  updates; `--uninstall` removes it. It replaces or removes only a neosh,
  leaves history and `~/.neoshrc` alone, and never changes the login shell
  or `/etc/shells`: it says how.

## Setup

- NSH-47: Setup gets neosh ready in the terminal, a step at a time, each one
  skippable: the model navi answers with, then neosh's look. As zsh's
  first-run setup does, it is offered when an interactive neosh starts with
  no `~/.neoshrc` (and no `--norc`): yes runs it, later asks again next
  time, never writes a `~/.neoshrc` holding only a comment, so it is not
  asked again. `neosh --setup` runs it any time, and run again it changes
  what it set. `neosh setup` is bash's, the script named `setup`. Without a
  terminal `--setup` says so and exits 1, unless every choice is given
  (NSH-51).
- NSH-48: The model step runs `navi setup` (SET-1 to SET-27) and goes on
  when it is done. With no `navi` on `$PATH` it says navi is what answers
  `?` and how to install it, and goes on.
- NSH-49: The look step offers neosh's own look (the default) or your
  zsh's. For zsh's, neosh asks zsh for its prompt and colours as your zsh
  starts (reading `~/.zshrc`, with no terminal, at most 5 seconds):
  - starship in zsh gives `eval "$(starship init bash)"`, the same prompt;
  - a `PROMPT` of zsh's escapes is written as bash's `PS1`: `%n` `%m` `%M`
    `%~` `%d` `%/` `%c` `%.` `%1~` `%#` `%?` `%j` `%h` `%!` `%l` `%T` `%t`
    `%@` `%*` `%D` `%D{…}` `%%` `%)`, colours and styles (`%F{…}` `%f`
    `%K{…}` `%k` `%B` `%b` `%U` `%u` `%S` `%s`, by name, number or
    `#rrggbb`) and `%{…%}`; `RPROMPT` is left out, bash having no right
    prompt, and said so;
  - a prompt it cannot write as bash's (one that runs commands or
    functions, `%(…)` conditions, or a theme: oh-my-zsh's, powerlevel10k)
    is named, with why, and neosh's own prompt is kept, which shows the
    branch and a failed status as most themes do;
  - `CLICOLOR`, `LSCOLORS` and `LS_COLORS`, when zsh sets them, are kept,
    so `ls` is coloured as in zsh.
- NSH-50: What setup would write is shown first and written only on yes,
  into `~/.neoshrc` between `# >>> neosh setup >>>` and
  `# <<< neosh setup <<<`, replacing what an earlier setup wrote there and
  nothing else; neosh's own look writes no prompt lines. It counts from
  the next neosh, or after `source ~/.neoshrc`; offered at start, it counts
  at once.
- NSH-51: `neosh --setup --look zsh` or `--look neosh` makes the look choice
  and skips the model step; with `--yes` it also writes without asking, so
  it runs without a terminal.

## The language

- NSH-3: Everything bash's grammar has that brush runs: quoting, pipelines
  and lists, redirections and here-documents, `if`, `while`, `for`, `case`,
  functions, expansions, globs, aliases, arrays and `[[ … ]]`, job control
  (^Z, `jobs`, `fg`, `bg`), and bash's builtins. Where brush and bash differ,
  bash is right, and the difference is brush's to fix.
- NSH-52: An error is said as bash says it: a script names itself and the
  line (`s.sh: line 2: x: command not found`, a sourced file names that
  file), `-c` and commands on stdin give the line (`neosh: line 1: …`),
  and a line typed at the prompt names neosh (`neosh: x: command not
  found`). What follows is brush's wording, except command not found,
  which is bash's.

- NSH-25: `cd` looks a relative directory up in `CDPATH` as bash does:
  each entry in turn (an empty one is `.`), the first holding the directory
  wins and its path is printed, and with none the directory is the current
  one's. A path starting `/`, `./` or `../`, or `.`, `..` and `-`, is never
  looked up. A directory that is not there says so as bash does
  (`cd: share: No such file or directory`, status 1).
- NSH-29: With `shopt -s autocd`, a line that is only the name of a
  directory and no command (`src`, `..`, `~/work`) runs `cd -- <it>`, said
  on stderr as bash says it; history keeps the line as typed.
- NSH-30: With `shopt -s cdspell` in an interactive neosh, a `cd` to a
  directory that is not there tries each part of the path with one
  character swapped, missing, extra or wrong (`cd ../bta` for `../beta`);
  a single such directory found is printed and gone to, as bash does.
- NSH-31: An interactive neosh sets `COLUMNS` and `LINES` to the terminal's
  size before each prompt while `checkwinsize` is on (as it is by default,
  as in bash 5), so scripts and `select` see the width.
- NSH-24: An interactive neosh expands history as bash does, before
  anything else reads the line, while `set -H` (`histexpand`, on by
  default) is: `!!` the last command, `!n` the n-th in `history`, `!-n` the
  n-th from last, `!text` the last starting with text, `!?text?` the last
  holding it; after one, `:0`, `:n`, `:n-m`, `:n*`, `:^`, `:$` or `:*`
  picks its words, and `!$`, `!^` and `!*` are the last command's.
  `^old^new` runs the last command with the first `old` made `new`. A `!`
  is left alone in single quotes, after `\`, before a space, `=`, `(` or a
  closing `"`, and in `$!` and `${!…}`. The expanded line is printed, then
  run, and is what history keeps; one naming no command says `event not
  found` (or `bad word specifier`, `substitution failed`) and runs nothing.

- NSH-32: After a history reference and its words, modifiers each after a
  `:` change what it names, as bash's do and in the order written: `h`
  drops the last `/` part, `t` keeps only it, `r` drops a `.suffix`, `e`
  keeps only it; `q` single-quotes it, `x` quotes each word; `s/old/new/`
  changes the first `old` (any delimiter, `\` quoting it, `&` in new
  standing for old, the last delimiter optional at the end of the line, an
  empty old the last one used or `!?text?`'s), `gs` or `as` every one; `&`
  repeats the last substitution, `^old^new` too, and `g&` everywhere. `p`
  prints the line and keeps it in history without running it. A
  substitution that finds nothing says so (`:s/a/b/: substitution failed`),
  an unknown modifier too, and runs nothing.

## navi

- NSH-4: A line starting with `?` goes to navi with the rest as its prompt,
  the question right after the `?` or after a space (`?why did that fail`,
  `? why did that fail`). A first word that is a glob stays the shell's, as
  in bash: one holding `*` or `[` (`?*`), or one whose `?` names a file
  there (`?.txt` beside `a.txt`). Every other line is the shell's and runs
  as bash would run it: neosh never guesses that a command was meant for
  navi, so Enter always does what it does in bash.
- NSH-23: When a line typed as a command is not found (status 127) and reads
  as a sentence (NSH-17), the next prompt starts with `?` and the line,
  so Enter asks navi instead, and ^C clears it. A suggestion from navi
  comes first. Nothing goes to navi without Enter.
- NSH-17: `NEOSH_ASK` says which lines go to navi, read for every line so
  setting it at the prompt or in `~/.neoshrc` counts at once: `marked` (the
  default) a `?` line and `| ?`; `off` none, so every line runs as a
  command; `sentences` also a line that reads as a sentence and whose first
  word is no command, alias, function, builtin or keyword. A sentence is
  three words or more, none starting with `-` and none holding `|`, `&`,
  `;`, `<`, `>`, `$`, `` ` ``, `=`, `(`, `)`, `*`, `[` or a path's `/`. With
  `sentences`, one starting with a command still goes to navi when it ends
  with `?`, unless that last word is a glob naming a file there (`ls a?`),
  or when its first word is `what`, `which` or `who` followed by a word
  such as `is`, `are`, `does` or `the` (`who am i` stays a command). Any
  other value is the default.
- NSH-19: With `NEOSH_ASK=sentences`, the first time in a session that a
  line goes to navi for reading as a sentence, without a `?`, neosh says so
  on stderr before navi answers, with how to run it as a command instead
  (`command why …`).
- NSH-5: navi answers in the terminal as the `navi` CLI does (`navi -p`), in
  the shell's directory, with its approvals, and gives the prompt back when
  it has answered (CLI-38). ^C stops it and never the shell, from the
  first line of a session on.
- NSH-16: A command line ending in `| ?`, with a question or without
  (`npm test |?why does this fail`, `npm test | ? why`), runs the command and hands navi what
  it printed, its errors too, with the question (or, alone, a request to
  explain it). The `|` and `?` count only outside quotes; `a || ? b` is no
  such line. navi answers as for any question (NSH-5 to NSH-7, NSH-11), and
  the line is kept in history as typed.
- NSH-6: navi is told the commands run in this shell since its last answer,
  up to the last 10, each with its directory and exit status (CLI-40), so
  "why did that fail" is about the right command. A command typed over
  several lines is told on one, each line break shown as ` ↵ `.
- NSH-7: One shell session is one conversation with navi, whichever
  directory it is asked from (CLI-39); a new neosh starts a new one.
- NSH-28: A line for navi is a command like any other for `$?` and the
  prompt: its status is navi's, 0 when navi answered, 130 when ^C stopped
  it, 127 with no `navi` on `$PATH`, 126 when it could not be started, and 0 for `?` alone saying how to ask.
  So a failure shown before is cleared once navi has answered it, and
  `$?` after `? …` says how the asking went.
- NSH-21: Which backend and model answered is said with the first answer
  of a session and again only when it changes (MOD-28): a hosted provider
  taking over from a local model is always said.
- NSH-8: A line sent to navi is kept in the shell's history as typed.
- NSH-13: History is kept between sessions in `~/.neosh_history`, or in the
  file `HISTFILE` names when it comes with the environment; ↑ and ^R find
  lines from earlier sessions, and `history` lists them as bash's does.
- NSH-26: History keeps what bash's would, a line for navi too: with
  `HISTCONTROL` holding `ignorespace` (or `ignoreboth`) a line typed with a
  space first is not kept, so a secret can be kept out of
  `~/.neosh_history`; `ignoredups` (or `ignoreboth`) drops a line the same
  as the one before, and `erasedups` drops its earlier copies from what ↑
  goes through (the history file keeps what it already has).
  A line matching one of `HISTIGNORE`'s `:`-separated patterns (`*`, `?`,
  `[…]`, and `&` for the line before) is not kept either. A line not kept
  still runs, and is never written to the history file.
- NSH-27: History is as long as bash's: `HISTSIZE` lines in the shell
  (↑, `history`, `!n`) and `HISTFILESIZE` in the history file, oldest
  dropped first, both 500 when neither the environment nor `~/.neoshrc`
  sets them, as bash's are. A value below zero, or not a number, keeps
  everything; `HISTFILESIZE` is applied when the shell exits, a timestamp
  line (`HISTTIMEFORMAT`) going with its command.
- NSH-9: `?` alone says how to ask navi, unless the command just run
  failed: then it asks navi why, and how to fix it (NSH-20). With no `navi` on `$PATH`, a line
  for navi says so and how to install it, and runs nothing.

- NSH-11: When navi's answer ends with a command to run (CLI-41), the next
  prompt starts with it typed out, the cursor at its end: Enter runs it, as
  if typed, and it can be changed first or cleared with ^C. neosh never runs
  it by itself.

## The prompt

- NSH-10: The prompt is the directory, `~` for home, and `$` (`#` as root),
  unless `PS1` came with the environment or `~/.neoshrc` sets one.
- NSH-12: In a git repository the prompt also shows its branch, or the
  commit's short hash when none is checked out (`~/proj main $`), read from
  `.git` without running git. After a command that failed, it shows the
  status before the `$` (`~/proj main 1 $`), in red. Colour is left out
  when `NO_COLOR` is set.
- NSH-22: When files git tracks have changes, staged or not, a `*` follows
  the branch (`~/proj main* $`). neosh asks git (`git status`, taking no
  locks, so it never gets in the way of a git command running beside it);
  with no git on `$PATH`, or no answer within a fifth of a second, the `*`
  is left out and the prompt is not held up. Untracked files are no change.
- NSH-15: A command that took 5 seconds or more shows how long, in the next
  prompt before any status (`~/proj main 1m 4s 1 $`), and in what navi is
  told of it (`~/proj $ cargo build (exit 101, took 1m 4s)`).

- NSH-33: zsh's hooks, as zsh and bash-preexec run them: before each prompt
  the `precmd` function, then each function named in the `precmd_functions`
  array; before each line the shell runs (a `| ?` line too, not a line for
  navi), `preexec` and `preexec_functions`, given the line as typed. A name
  with no function is skipped. Each sees `$?` as the command left it and
  leaves it so, for the prompt, for navi (NSH-6) and for the next line; a
  `PS1` that `precmd` sets shows in that prompt.

## The terminal

- NSH-18: An interactive neosh tells the terminal, unless `TERM` is unset or
  `dumb`: the title is the directory while it waits and the command while
  one runs; the directory is reported (OSC 7), so Terminal, iTerm and others
  open a new tab there; and in VS Code's terminal, commands are marked with
  their status as its shell integration expects.

## Open

- Telling navi a failed command's output without `| ?`: commands write
  straight to the terminal, so neosh sees only the status.
- A prebuilt binary, so installing needs no Rust.
- Where brush is not bash yet (NSH-3), found by running both: `select`
  does not parse; `shopt -s globstar` makes `**/x` name paths that are not
  there; `coproc` can stop neosh with a bad file descriptor; `dirs` shows
  `/private/tmp` for `/tmp`. These are brush's to fix.
