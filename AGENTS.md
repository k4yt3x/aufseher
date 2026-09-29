# AGENTS.md

Guidance for AI agents working in this repository.

# Rust practice

## Safety

- Avoid panicking calls (`unwrap()`, `expect()`, unchecked indexing). Propagate with `?`. The lints
  are `warn` in `Cargo.toml` and relaxed under `cfg(test)`, where panicking on failure is the point.
- Never discard errors with `let _ =`. Propagate with `?`, log explicitly when ignoring is correct, or
  handle with `match` / `if let Err(..)`.
- Errors from fallible async work must reach the UI layer so the user gets real feedback.

## Layout and style

- No `mod.rs`. Use `src/some_module.rs`.
- New crates set `[lib] path = "..."` in `Cargo.toml` for a descriptive root name.
- Never hand-wrap comments. One line per paragraph; `cargo +nightly fmt` wraps them
  (`.rustfmt.toml` sets `wrap_comments = true`). If a wrap lands awkwardly, reword rather than
  inserting a manual break.
- Shadow a binding to scope a clone in async contexts:

  ```rust
  executor.spawn({
      let task_ran = task_ran.clone();
      async move { *task_ran.borrow_mut() = true; }
  });
  ```

## Names and visibility

- Test names read as sentences: `a_fork_keeps_its_images_when_the_source_is_deleted`, never a
  `test_` prefix. A test-only constructor is `for_test()` or ends in `_for_test`.
- `new` is infallible; `open`, `from_*` and `resolve` are fallible. No `fresh`, `try_new` or
  `from_connection`. Predicates are `is_*`; the feature-on question is `is_enabled()`.
- `pub(crate)` for anything another module reads, `pub(super)` for a parent alone, never bare `pub`
  in this binary crate; child modules are `pub(crate) mod`.
- Every public item has a `///` comment, except an axum handler (its route documents it) and a
  tool's struct (its `definition()` does).
- Time is a `Duration` constant; `_MILLIS`/`_SECONDS` only for an integer that goes on a wire. Sizes
  are `_BYTES` or `_CHARS`, never `_LEN`, `_LIMIT` or `_CAP`; a MiB literal goes through the named
  `MIB`. Terminal display goes through `text::format_timestamp` and `text::format_size`; the wire
  carries RFC 3339 and raw byte counts.
- A value enum with a wire spelling follows `Backend`: one `const fn name()`, `Display` and
  `FromStr` derived from an `ALL` table plus `name()`, serde through `try_from = "String"` and
  `into = "String"`, clap parsers `.parse()`. One spelling per value; no aliases, no alternates.
  Values a model emits (todo status words) are the one exception.

## Build gate

Run after editing: `cargo +nightly fmt` and `cargo sort -w`.

CI denies warnings on clippy and rustdoc, so the bare commands can pass locally and fail CI.
Reproduce the exact gate before declaring done:

```
cargo +nightly fmt --check
cargo sort -w --check
cargo clippy --locked --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --document-private-items
cargo test --locked                  # CI adds --features mock-provider; debug builds carry it anyway
cargo check --locked --all-targets   # on the MSRV in Cargo.toml's rust-version
mdbook build docs/book
```

The `mock-provider` feature exists so a release-profile build can run the suite; debug builds carry
it regardless. A shipped artifact must never have it, because `MEKA_MOCK_PROVIDER=1` then stands in
for every profile.

`--all-targets` matters: plain clippy skips tests and benches. In rustdoc, watch
`rustdoc::invalid_html_tags`: a bare `<word>` parses as an unclosed tag. Backtick it, or rephrase if
the comment is also a clap help string, where backticks render literally.

`fmt --check` does not enforce `comment_width`: `wrap_comments` silently declines some comments (in
a macro body, in a method chain) and still exits 0, so a paragraph left for rustfmt to wrap can ship
at 400 columns. After `fmt`, `awk 'length>100 && /^[[:space:]]*\/\//'` the changed files; reword
what it prints, or break it by hand.

## Clap help text

`///` doc comments must render within 120 columns under `-h`, and stay as short as they can: no
examples, no tautology, one line where one line says it. Verify by running the binary with
`COLUMNS=120` for every changed subcommand: source length ignores clap's indent, value-name width,
and auto-appended hints. Adding flags widens the whole column, so a new flag can push existing lines
over.

Long-form prose goes after a blank `///` line so it appears only under `--help`. When that prose is
multi-line or indented, add `#[command(verbatim_doc_comment)]`.

Command summaries take no trailing period; multi-sentence prose is punctuated normally.
