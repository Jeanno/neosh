# TODO

Claude's working list for neosh, and for navi where neosh needs it.
Newest asks first. Done items move to the log at the bottom.

## Now

- Run the judge eval (neoai TL-46) on Darkbloom once it answers; it was at
  capacity (429) for the first run. Then decide whether it should judge.

## Next

- `[1]+ Done` printed after ^C on a job brought back with `fg` (brush).
- Where the "? to ask why" hint sits, and the setup prompts' styling.
- Tool calls with no words yet (most `browser_*`): give them some.

## Waiting on the owner

- A Raycast release tag (neoai `raycast-v*`).
- A prebuilt neosh release.
- Filing upstream with brush: background jobs survive the terminal closing,
  and a hook in the reedline validator (vendored here, NSH-53).
- A possible rename of the pm skill.
- Whether `navi setup` should offer `claude -p` or point at API credit.

## Done

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
