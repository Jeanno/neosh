# TODO

Claude's working list for neosh, and for navi where neosh needs it.
Newest asks first. Done items move to the log at the bottom.

## Now

- Run the judge eval (neoai TL-46) on Darkbloom once it answers; it was at
  capacity (429) for the first run. Then decide whether it should judge.

## Next

- `[1]+ Done` printed after ^C on a job brought back with `fg` (brush).
- Where the "? to ask why" hint sits.
- navi's example commands with placeholders (`your-command-here`) get typed at the prompt; only a runnable command should be.
- navi, away and done, shows its answer at the next prompt, not while you sit at an empty one (reedline's external printer could).
- navi offered the output of a command (`Mon Nov  9 …`) as the command to type; with the placeholder item above, only a runnable command should be (neoai).
- navi's write tool takes `~/x` literally, making a `~` directory; the model then guessed a home from the path (neoai).
- Tool calls with no words yet (most `browser_*`): give them some.

## Waiting on the owner

- A prebuilt neosh release.
- Filing upstream with brush: background jobs survive the terminal closing,
  and a hook in the reedline validator (vendored here, NSH-53).
- A possible rename of the pm skill.
- Low priority (deprioritized 2026-11-09): a Raycast release tag (neoai `raycast-v*`).

## Done

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
