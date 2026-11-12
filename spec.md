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
  being started as `-neosh` (as a login shell is, by every terminal),
  reads `/etc/profile` and `~/.profile` first, then `~/.neoshrc` when
  interactive (where bash's login shell reads no rc), as zsh reads
  `~/.zshrc`.
- NSH-2: The exit status is the last command's, and `exit [n]` leaves with
  n. `--version` and `--help` say what neosh is and how to start it. A
  script that is not there says so as bash does (`neosh: x.sh: No such file
  or directory`, status 127).

- NSH-14: `install.sh` puts neosh in `~/.local/bin` (`NEOSH_BIN_DIR`):
  built from the checkout it is in or, piped from curl, the prebuilt one
  of the latest release (NSH-64), else built from a clone in
  `~/.local/share/neosh`. Run again, it
  updates; `--uninstall` removes it. It replaces or removes only a neosh,
  leaves history and `~/.neoshrc` alone, and changes the login shell only
  on yes (NSH-80).
- NSH-61: When there is no `navi` on `$PATH` or in the install directory,
  `install.sh` installs it too, without asking, by navi's own installer
  (neoai `neonavi/install.sh`) into the same directory. That installer says
  what it needs (Node 22 or newer) when it is missing; neosh stays installed
  either way, and the installer says how to try again.
  `NEOSH_INSTALL_NAVI=no` leaves navi out and says how to install it.
- NSH-64: Piped from curl, `install.sh` needs no Rust when there is a
  release for the machine: it downloads `neosh-<target>.tar.gz` for macOS
  (arm64, x86_64) or Linux (x86_64, arm64) from the latest GitHub release
  (`NEOSH_RELEASE_URL` for another place), checks it against the
  `.sha256` beside it, and installs the neosh in it. A download that does
  not match, or holds no neosh, installs nothing and says so. With no
  release, no asset for the machine, or `--source`, it builds from source
  as before; from a checkout it always builds the code there.
- NSH-65: Pushing a tag `v<version>`, the version in `Cargo.toml`, makes a
  GitHub Release (`.github/workflows/release.yml`) of neosh built for each
  machine of NSH-64, each a tarball of the program, README and LICENSE with
  its sha256. A tag that is not the version builds nothing; a machine whose
  build fails is left out, but not the Mac on arm.

## Setup

- NSH-47: Setup gets neosh ready in the terminal, a step at a time, each one
  skippable: how the prompt looks first, the prompt being most of what a
  shell looks like, then plugins (NSH-78), the model navi answers
  with, then the login shell (NSH-80). It is offered when an interactive neosh starts with
  no `~/.neoshrc` (and no `--norc`): yes runs it, later asks again next
  time, never writes a `~/.neoshrc` holding only a comment, so it is not
  asked again. `neosh --setup` runs it any time, and run again it changes
  what it set. `neosh setup` is bash's, the script named `setup`. Without a
  terminal `--setup` says so and exits 1, unless every choice is given
  (NSH-51).
- NSH-48: The model step runs `navi setup` (SET-1 to SET-29) and goes on
  when it is done. With no `navi` on `$PATH` it says navi is what answers
  `?` and how to install it, and goes on.
- NSH-49: The look step offers starship (the default, NSH-77), neosh's
  own look, or neosh's with the parts you pick (NSH-74).
- NSH-50: What setup would write is shown first and written only on yes,
  into `~/.neoshrc` between `# >>> neosh setup >>>` and
  `# <<< neosh setup <<<`, replacing what an earlier setup wrote there and
  nothing else. It is written when the look and plugin steps are done,
  before the model step; no to writing it still goes on to the model
  step. neosh's own look writes no prompt lines, only a comment
  naming the choice, so a first start that kept every default is not
  offered setup again; that is written without asking, unless it takes an
  earlier setup's look out. It counts from the next neosh, or after
  `source ~/.neoshrc`; offered at start, it counts at once.
- NSH-58: Setup says what it is for before asking (a line starting with
  `?` asks navi; setup is how the prompt looks, plugins, the model
  that answers `?`, and the login shell) and asks once: yes goes straight to the first step,
  with no second question; leaving navi's setup with no model, navi says
  so (SET-29). Each step is
  headed `Step N of 4` in bold, with hints dim (plain without a terminal or
  with `NO_COLOR`). At the end it says neosh is ready and shows how to
  begin: `?what's in this folder`, `?` for navi mode, `neosh --setup` to
  change it.
- NSH-51: `neosh --setup --look starship` or `--look neosh` makes the look choice
  and skips the plugin and model steps, keeping the plugins an earlier
  setup chose; with `--yes` it also writes without asking, so
  it runs without a terminal.
- NSH-73: `NEOSH_PROMPT` names the parts neosh's own prompt shows, read at
  each prompt, so setting it at the prompt shows at once: `user`, `host`
  (together `user@host`), `dir` (the directory, `~` for home) or `folder`
  (only its last folder), `branch` (NSH-12, NSH-22), `took` (NSH-15),
  `status` (a failed one), `time` (`14:05`), and `newline` (the `$` on a
  line of its own). They show in that order, whatever order they are
  named in; a word that is not a part is ignored. Unset, it is
  `dir branch took status`. `$` (`#` as root) always ends the prompt.
  It is neosh's own prompt's: a `PS1` set by the rc or the environment
  replaces that prompt, and `NEOSH_PROMPT` with it.
- NSH-74: The look step offers a choice, `my own`: the parts are
  listed, numbered and marked on or off (starting from what an earlier
  setup chose), with the prompt shown below as it would be after a
  command that failed and took 12s. Typing numbers switches those parts,
  and the list and prompt are shown again; `dir` and `folder` exclude each
  other. Enter ends it. What is written (NSH-50) is the comment for
  neosh's own look and `NEOSH_PROMPT='…'`, unless the parts are the
  default ones.
- NSH-75: `neosh --setup --prompt '<parts>'` chooses neosh's own look with
  those parts, as `--look` does (NSH-51); a word that is not a part says
  so, naming the parts, and exits 2.
- NSH-76: `NEOSH_PROMPT_COLORS` gives parts of neosh's own prompt their
  colour, as `part=style` words (`dir=green branch=bright-yellow,bold`):
  a style is a colour's name, `bright-<name>`, a number up to 255,
  `#rrggbb`, `bold`, `dim`, `italic`, `underline`, several joined by `,`,
  or `none`; `none` alone is no colours. The parts not named keep theirs
  (user and host green, the directory bold blue, the branch magenta, the
  time taken yellow, a status red, the time dim). `NO_COLOR` still means
  none. After the parts, setup's `my own` asks for colours the same way,
  showing the prompt again after each answer (a wrong word is said and
  asked again); Enter keeps them, `default` goes back to neosh's.
  `--setup --colors '<words>'` sets them without questions; a wrong word
  says so and exits 2.
- NSH-77: The look step's first choice, and its default, is starship,
  which draws the prompt, with themes and icons, as it does in bash: setup
  writes `command -v starship >/dev/null && eval "$(starship init bash)"`,
  so a neosh with no starship draws its own prompt. Its presets are listed by number, each
  shown as starship draws it (after a command that failed and took 12s);
  0 keeps the config there is, or starship's own. A preset taken is
  written to starship's config (`$STARSHIP_CONFIG`, or
  `~/.config/starship.toml`), the file there kept beside it as `.bak`,
  said before asking (NSH-50). Chosen with no starship on `$PATH`, setup
  installs it without asking: `brew install starship` when there is
  Homebrew, else starship's installer into `~/.local/bin` (`--yes`, no
  sudo), putting that folder on `PATH` in the block when `PATH` lacks it.
  If that fails it says how to install starship and takes neosh's own
  look instead. `--look starship` with no
  starship says how to install it, writes nothing and exits 1. `--setup --look starship` and
  `--preset <name>` choose it without questions; a preset starship does
  not have says so, naming them, and exits 2.
- NSH-78: The plugin step offers the plugins neosh knows by name
  (NSH-69), each with what it does, asked one at a time with yes as the
  default (`z: jump to folders you use often, as z proj`). Those taken are
  written in the setup block as `plugins=(z)`; none taken writes no
  `plugins` line. Plugins named outside the block (a `plugins=` line of
  your own) are left alone: the step says so and is skipped. Run again,
  it starts from what an earlier setup chose. `--setup --plugins '<names>'`
  chooses them without questions (`''` for none); a name neosh does not
  know says so, naming those it knows, and exits 2.
- NSH-79: (Struck: copying zsh's PATH and colours; NSH-81 finds neosh and navi.)
- NSH-80: Setup's last step, and the installer's last question, is whether
  neosh is your login shell: the system's (`dscl` on macOS, `getent`
  elsewhere), not `$SHELL`, which apps keep from when they started. When
  it already is, that is said and nothing asked. Otherwise `[y/N]`, no
  unless yes: yes adds neosh's path to `/etc/shells` when it is not there
  (`sudo tee -a`, which asks for the password) and runs `chsh -s`; if
  either fails it says the commands to run. On macOS, apps are handed
  `SHELL` as the login shell was at login, and terminals such as Ghostty
  start `$SHELL`: so then, and when neosh already is the login shell,
  `launchctl setenv SHELL <neosh>` tells launchd, and it is said that new
  Terminal windows start neosh and an app already open does once quit
  (⌘Q) and opened again. No says how to do it
  later. The installer asks on the terminal even when piped from curl,
  and without one does not ask; `NEOSH_LOGIN_SHELL=yes` or `no` answers
  it.
- NSH-81: A login or interactive neosh puts its own folder on `PATH`, at
  the end, when `PATH` lacks it, before setup or any startup file, so
  `neosh`, and navi installed beside it (NSH-61), are found in a terminal
  that starts neosh with the system's `PATH` and no startup file of yours.
  A script's or `-c`'s `PATH`, not a login shell's, is left as given.
- NSH-82: In a terminal, neosh looks as itself whatever the system's
  startup files do: the prompt macOS's `/etc/bashrc` sets (`\h:\W \u\$ `,
  read by `/etc/profile` for any shell with `$BASH`) is replaced by
  neosh's own, unless the environment brought a `PS1`; and the first prompt
  shows no failed status from the startup files (`/etc/bashrc` ends on a
  failed test in any terminal but Terminal.app).
- NSH-83: `uninstall.sh` (in the repo, and piped from curl) undoes the
  install and setup, in an order that never leaves a terminal without a
  shell: neosh as the login shell is changed back first (`chsh -s` to
  `NEOSH_SHELL_BACK`, else `/bin/zsh` on macOS and `/bin/bash` elsewhere,
  and launchd told as NSH-80 does), and if that is declined or fails
  nothing is removed. Then neosh in the install directory (only a neosh,
  as NSH-14) and the clone it was built from; then, asked, `~/.neoshrc`,
  `~/.neosh_history` and `~/.neosh` are moved to
  `~/neosh-uninstalled-<time>` (not deleted), and navi beside neosh is
  removed. Questions are asked on the terminal; without one the answer is
  no; `--yes` answers yes. `/etc/shells`, starship and `~/.neoai` are left,
  and said so.
- NSH-84: A starship look draws icons from a Nerd Font (all but
  `plain-text-symbols`, `no-nerd-font` and `pure-preset`). Chosen in
  setup, in Ghostty (`TERM_PROGRAM=ghostty`, which draws the icons itself)
  that is said and nothing done. Elsewhere, with no Nerd Font installed
  (a font file named so in the user's or system's font folders), setup
  installs JetBrainsMono Nerd Font without asking (`brew install --cask
  font-jetbrains-mono-nerd-font`); with no Homebrew it says where to get
  one. In Terminal.app it then asks `Use JetBrainsMono Nerd Font Mono in
  Terminal? [Y/n]` and on yes sets the font of Terminal's default profile
  (`osascript`), so new windows show the icons; in other terminals, and on
  no, it says which font to choose where.

## The language

- NSH-3: Everything bash's grammar has that brush runs: quoting, pipelines
  and lists, redirections and here-documents, `if`, `while`, `for`, `case`,
  functions, expansions, globs, aliases, arrays and `[[ … ]]`, job control
  (^Z, `jobs`, `fg`, `bg`), and bash's builtins. Where brush and bash differ,
  bash is right, and the difference is brush's to fix.
- NSH-63: A job brought back with `fg` is the terminal's again: when it
  ends there (^C, or done), it is not reported as `[1]+ Done` at the next
  prompt, as bash does not; stopped again with ^Z, it is reported Stopped.
  A job that ends in the background is still reported Done.
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
- NSH-53: A line for navi is sent on Enter even when bash would read it as
  unfinished: a `'` or `"` in the question (`?what's wrong`, `npm test |?it's
  failing`) opens no quote. A line for the shell still waits for its quote
  or its `done` to close, as in bash.
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
  first line of a session on. Its answer is drawn in a dim `│` gutter
  (CLI-43), so it stands apart from what commands printed, a code block
  without it so it copies clean.
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
- NSH-9: `?` alone turns navi mode on (NSH-54), and when the command just
  run failed, asks navi why, and how to fix it (NSH-20), first. With no `navi` on `$PATH`, a line
  for navi says so and how to install it, and runs nothing.
- NSH-59: neosh carries its manual, `MANUAL.md`, built in. Each time it asks
  navi it hands the manual's path over as `NAVI_SHELL_MANUAL` (neoai CLI-47),
  written first to a file of this version's in the temporary directory, so
  navi answers a question about neosh from it (neoai PRM-15).
- NSH-60: `?/usage` shows the tokens navi has used and what they cost,
  estimated, by model: it runs `navi usage` (neoai LED-19) on the terminal,
  with whatever follows it (`?/usage --days 7`). With no `navi` on `$PATH`
  it says so, as NSH-9 does.
- NSH-62: navi never holds the shell up. While it answers, ^Z sends it to
  the background: it works on and the prompt comes back at once (navi mode
  is left). What it writes meanwhile is held, and navi is told it is away,
  so it says by notification when it is done and when it waits for a yes
  or no (neoai CLI-49, as `navi notify` sets). The next prompt says the
  same in a line (neoai CLI-48 for the question), not the answer. `?`
  alone brings navi back, what it held first, so the answer shows then and
  the command it suggests is typed (NSH-11). Another question, or `| ?`,
  waits while it works and says so; once it is done, one goes ahead, the
  answer left unseen. It sees the commands run while it was away with the
  next question. Closing neosh stops it.
- NSH-57: In navi mode, the command navi offered (NSH-11), run with Enter
  as it was put there, runs in the shell, and the mode goes on after it.
  Changed in any way, the line is a question again.
- NSH-56: A line that goes to navi (one starting with `?`, any line in
  navi mode, or what `NEOSH_ASK` sends, NSH-9) is drawn as typed, in the
  terminal's own colour, not highlighted as bash: its quotes, `$` and
  words are words. A line for the shell is highlighted as before, and one
  whose highlighting would cut a wide character (`志`) in two is drawn plain
  rather than ending the shell.
- NSH-55: `?/resume` lists the conversations earlier neosh shells had with
  navi (`navi sessions --json`, SES-18), newest first and at most nine: how
  long ago, the directory, and the last thing asked. Enter takes the first,
  a number another; anything else resumes nothing. The shell's questions
  then go on in that conversation, as if asked there. With none saved, it
  says so.
- NSH-54: In navi mode every line is a question for navi, as if typed after
  `?`, whatever it holds (a glob, a quote, a `|`), and the prompt's `$`
  shows as `?` (or `? ` follows a prompt of one's own). Esc goes back to
  the shell, keeping what was typed so far at the prompt; so do `exit` and
  ^D, which leave the shell running. An empty line asks nothing. History
  keeps each question with its `?`, so outside the mode it asks again. Esc
  otherwise does what it always did.

- NSH-11: When navi's answer ends with a command to run (CLI-41), the next
  prompt starts with it typed out, the cursor at its end: Enter runs it, as
  if typed, and it can be changed first or cleared with ^C. neosh never runs
  it by itself.

## What you did

navi knows what you have been doing in this terminal: the commands you ran
and what they printed. neosh keeps each command's output in a file and tells
navi where it is; navi reads it only when the question needs it.

- NSH-34: An interactive neosh on a terminal runs inside a recorder of its
  own, the same binary, which sits between the terminal and the shell and
  copies everything between them untouched: keys, colours, resizes, ^C and
  ^Z. Full-screen programs (vim, less, htop) work as they do without it.
  Starting it and running each command add no delay you can see (under
  50 ms). If it cannot start, neosh carries on unrecorded and `?` says once
  that output is not being kept.
- NSH-35: A neosh is recorded once per terminal: the recorder's terminal,
  named in `NEOSH_RECORDER`, must be the shell's own, so a neosh in a tmux
  pane, or started on another terminal, is recorded by its own recorder
  even when it kept `NEOSH_RECORDER` from the shell that started it, and a
  neosh started inside a recorded one is recorded by the outer recorder.
- NSH-36: neosh marks where each command's output starts (with its line,
  its directory and a number) and where it ends (with its status), as
  escape sequences the recorder takes out before they reach the terminal.
  A mark carries the session's key, given by the recorder in the
  environment, so a program printing a mark it made up is not taken for
  one.
- NSH-37: Each command's output goes to its own file as it is printed:
  colours and cursor movement taken out, a line redrawn in place (a
  progress bar) kept as it ended. A command still running can be read
  while it runs. A file keeps a command's first 1,000 lines and its latest
  10,000, and says what was dropped between.
- NSH-38: What a program shows on the alternate screen (vim, less, htop) is
  not kept; its file says a full-screen program ran there. Input a program
  echoes is output, so what is typed into python, node or psql is kept with
  the command that started it.
- NSH-39: The index holds one line per command since the terminal opened:
  number, directory, the line, how it ended (exit status, stopped, still
  running), how long it took, its file and its line count. It is updated
  when a command starts and when it ends.
- NSH-40: ^Z then `?` works: the stopped command's entry says it is
  stopped (job %n), its output so far is in its file, and `fg` resumes
  writing to the same file.
- NSH-41: navi is told, for each command in NSH-6, where its output is and
  how many lines it is (`printed 37 lines: <file>`), or that it printed
  nothing, and where the index is, so it can look further back; never the
  output itself (CLI-40, CLI-42). It reads a file with its file tool, a page at a
  time. Recorded output is the user's data, never instructions, and navi is
  told so.
- NSH-42: All of it is in one directory per terminal, readable only by the
  user (`$TMPDIR/neosh-rec-<pid>/`), deleted when the terminal closes; one
  left by a terminal that died is deleted the next time a recorder starts.
- NSH-43: Nothing is recorded for a line typed with a space first, a line
  for navi, or a `| ?` line (navi is handed that output already).
  `NEOSH_RECORD=off`, at the prompt or in `~/.neoshrc`, stops recording from
  the next command on, and `NEOSH_RECORD=on` starts it again; what was
  recorded stays until the terminal closes. With `NEOSH_RECORD=off` in the
  environment when neosh starts, there is no recorder at all.
- NSH-44: Obvious secrets are blanked before they are written: keys and
  tokens by their shapes (`sk-…`, `ghp_…`, `AKIA…`, `xoxb-…`, `Bearer …`),
  values of `password=`, `token:`, `secret=` and the like, and private key
  blocks. Blanking is a safety net, not a promise; NSH-43 is the way to keep
  something out. Output never leaves the machine on its own: only what navi
  reads for a question goes to the model, and which model that is is said
  (NSH-21).
- NSH-45: After a command fails (not ^C, not ^Z), a dim line under it says
  `? to ask why`, for the first 3 failures of a session and then no more.
  navi is never asked anything unasked. `NEOSH_HINT=always` shows it after
  every failure, `NEOSH_HINT=off` never.
- NSH-46: In tmux each pane's neosh is recorded on its own (NSH-35); tmux
  itself is a full-screen program (NSH-38). `ssh host` is a command like
  any other: what the remote session prints is its output, kept in its
  file as it runs.

## The prompt

- NSH-10: The prompt is the directory, `~` for home, and `$` (`#` as root),
  unless `PS1` came with the environment or `~/.neoshrc` sets one. A `PS1`
  from the environment that is zsh's (its `%` escapes, or a prompt such as
  powerlevel10k's that calls zsh functions) is not used: bash cannot show it.
- NSH-72: A prompt (`PS0`, `PS1`, `PS2`) shows `\#` as this command's number
  and `\!` as its history number, as bash does; brush has neither. A
  prompt that still cannot be shown is said once, in a line naming it,
  and drawn plain (`$ `); the shell runs on.
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

## Plugins

- NSH-66: `plugins=(…)` in `~/.neoshrc` names what to load, in order, once
  the rc has run: an interactive neosh only, as the rc is. Each entry is
  one of:
  - a plugin known by a short name (`z`, NSH-69);
  - a path (`~/x`, `/x`, `./x`): a `.sh` or `.bash` file, or a folder,
    whose `<folder>.plugin.sh`, `<folder>.plugin.bash`, `plugin.sh`,
    `init.sh` or `<folder>.sh` is sourced;
  - a git repo: `owner/repo` for GitHub, or a URL (`https://…`, `git@…`,
    `file://…`).
  A name loaded twice loads once. `plugins` may also be one word.
- NSH-67: A repo is cloned (shallow) into `~/.neosh/plugins/<owner>/<repo>`
  the first time it is named, with a line saying so; it is then sourced
  as a folder is (NSH-66). It is never fetched again at start: `plugin
  update` pulls every cloned one. A clone that fails says so in a line and
  the shell starts without it.
- NSH-68: A plugin that cannot load (no such built-in name, no such path,
  a folder with no file to source, an error while sourcing) is said in one
  line naming the entry; the others still load and the shell starts.
  `plugin list` shows each loaded plugin and where it came from; `plugin`
  alone says how to use it.
- NSH-69: Short names neosh knows, each a repo cloned as NSH-67 says:
  `z` is rupa/z (github.com/rupa/z), installed and run as it is, so `z`
  is that `z`: it records directories in `~/.z` and `z <words>` goes to
  the best match. A short name not known says so (NSH-68), naming the
  ones that are.
- NSH-70: (Struck: z built into neosh; it is rupa/z's own, NSH-69.)
- NSH-71: An alias runs as bash runs it: its text is put in place of the
  word before the line is parsed, so a redirect, pipe, quote or `;` in it
  works (`alias z='_z 2>&1'`), only an unquoted word where a command goes
  is one, and an alias defined on a line counts from the next command.
  `alias` and `type` show it as an alias. (Fixed in brush, in neosh's fork;
  brush split an alias's text at spaces.)

## Open

- Asking in one terminal about another (a dev server in another tab), and
  seeing what a full-screen program shows: not in the first version of
  NSH-34 to NSH-46.
- When the remote shell of an `ssh` is a neosh too, an entry for each
  remote command under the `ssh` one: not in the first version.
- Where brush is not bash yet (NSH-3), found by running both: `select`
  does not parse; `shopt -s globstar` makes `**/x` name paths that are not
  there; `coproc` can stop neosh with a bad file descriptor; `dirs` shows
  `/private/tmp` for `/tmp`; jobs in the background keep running when the
  terminal closes, where bash sends them a hangup; a job started with `&`
  is said as `[1]+ <pid unknown>` and, brought back with `fg`, ^C does not
  stop it; `bg` does not say the job it resumed. These are brush's to fix.
