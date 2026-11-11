# TODO

Claude's working list for neosh, and for navi where neosh needs it.
Newest asks first. Done items move to the log at the bottom.

## Now

## Next

- A job started with `&` has no pid in brush (`[1]+ <pid unknown>`), so `fg` cannot hand it the terminal and ^C does not stop it; `bg` says nothing (brush, NSH-3).
- Where the "? to ask why" hint sits.

## Waiting on the owner

- Filing upstream with brush: background jobs survive the terminal closing,
  a hook in the reedline validator (vendored here, NSH-53), and `fg` leaving
  a job that ended in the foreground in the job list, so it is reported
  `Done` (worked around in neosh's `fg`, NSH-63), and an alias's text split at
  spaces and never parsed, so `alias e='echo hi 2>&1'; e` prints `hi 2>&1`
  (brush-core 0.5.0 `interp.rs`, its TODO #57; worked around in neosh, NSH-71).
- A possible rename of the pm skill.
- Low priority (deprioritized 2026-11-09): a Raycast release tag (neoai `raycast-v*`).

## Done
- 2026-11-11: release v0.1.1 (plugins, rupa/z, aliases as in bash).
- 2026-11-11: `plugins=(z)` installs rupa/z's own z in place of neosh's (NSH-69, NSH-70 struck), and an alias with a redirect, pipe or quote runs as in bash (NSH-71).
- 2026-11-11: plugins named in `~/.neoshrc` (`plugins=(…)`: built in, a path, or a git repo cloned once; `plugin list`, `plugin update`), and `z` built in (NSH-66 to NSH-70).
- 2026-11-10: navi hands neosh only a command that runs as it stands, never a placeholder or a command's output (neoai CLI-60); `~` is home in navi's file tools (TL-48); every tool call is said in words (CLI-61).
- 2026-11-10: the judge eval on Darkbloom (Qwen3.8 27B, 3 runs): 91%, no unsafe approval, 9 needless asks, about 1.6s a verdict; good enough to judge when it is the model.

- 2026-11-10: a prebuilt release: `release.yml` builds macOS and Linux tarballs on a `v*` tag; piped from curl, `install.sh` installs the latest one after checking its sha256, and builds from source with none or `--source` (NSH-64, NSH-65).
- 2026-11-10: no `[1]+ Done` after ^C on a job brought back with `fg`, as in bash (NSH-63).
- 2026-11-10: navi away notifies when it is done or waits for you, instead of its answer showing at the next prompt (NSH-62, neoai CLI-49).
- 2026-11-09: ^Z sends navi to the background while it works; `?` brings it back, the next prompt shows its answer or that it waits for you (NSH-62, neoai CLI-48).
- 2026-11-09: `install.sh` installs navi too when it is missing, without asking (NSH-61); `?/usage` runs `navi usage` (NSH-60).
- 2026-11-09: neosh carries a manual navi reads to answer questions about neosh (NSH-59); navi counts tokens and `navi usage` (`?/usage` here) estimates the cost (neoai LED-18 to LED-20).
- 2026-11-09: decided: `navi setup` stays as it is, with no `claude -p` and no pointer to API credit.
- 2026-11-09: setup asks once, says what it is for, styles its steps, saves neosh's own look so it is not offered again, and ends with how to begin (NSH-58).
- 2026-11-09: navi mode runs the command navi offered (NSH-57); wide characters no longer panic the highlighter; navi's spinner no longer hides a y/n question (neoai CLI-45).
- 2026-11-09: a line for navi is drawn as typed, not highlighted as bash (NSH-56).
- 2026-11-09: live-tested on Sonnet (Darkbloom was at capacity): an image
  read, a command's recorded output read, `?/resume`, tool calls in words.
- 2026-11-09: `?/resume` picks up an earlier shell's conversation (NSH-55, SES-18).
- 2026-11-09: navi looks at an image given as a path, and reads a PDF (TL-45).
- 2026-11-09: navi mode, `?` alone until Esc (NSH-54).
- 2026-11-09: a quote in a question no longer holds the line open (NSH-53),
  with brush-interactive vendored for the validator hook.
- 2026-11-09: README leads with navi as an agent in the terminal.
- 2026-11-09: markdown drawn and tool calls in words in the gutter (CLI-44).
- 2026-11-09: navi's answer in a dim gutter (NSH-5, CLI-43).
- 2026-11-09: what each command prints is recorded for navi (NSH-34 to NSH-46).
- 2026-11-08: setup at first start or with `--setup` (NSH-47 to NSH-51).
- 2026-11-08: errors said as bash says them (NSH-52).
