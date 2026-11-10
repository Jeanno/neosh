# brush-interactive, as neosh uses it

This is [brush-interactive](https://github.com/reubeno/brush) 0.4.0 from
crates.io (MIT, see LICENSE), with three changes. The first: `src/reedline/validator.rs`
takes a hook, `set_completes`, that neosh uses to say a line for navi is
complete even when bash would read it as unfinished (`?what's wrong`, whose
`'` opens a quote). Without it, Enter on such a line waits for more input. The same check
tells `src/reedline/highlighter.rs` to draw a line for navi as typed,
not highlighted as bash (NSH-56); and a line whose highlighting would cut
a character in two (brush can, on `志`) is drawn plain rather than panic.

A third, one line in `src/reedline/edit_mode.rs`: readline's `abort` is
bound as reedline's Esc, so neosh can give Esc back its own job after navi
mode (NSH-54) has used it.

To move to a newer brush, copy the new crate here and make the same changes.
