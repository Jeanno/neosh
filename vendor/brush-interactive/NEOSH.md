# brush-interactive, as neosh uses it

This is [brush-interactive](https://github.com/reubeno/brush) 0.4.0 from
crates.io (MIT, see LICENSE), with one change: `src/reedline/validator.rs`
takes a hook, `set_completes`, that neosh uses to say a line for navi is
complete even when bash would read it as unfinished (`?what's wrong`, whose
`'` opens a quote). Without it, Enter on such a line waits for more input.

To move to a newer brush, copy the new crate here and make the same change.
