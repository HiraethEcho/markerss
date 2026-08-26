# AGENTS

## What this branch is

`rust-comment` is a **learning fork** of the `rust` implementation branch.
Purpose: teach Rust to a human who knows a little C, using this real codebase
(a TUI RSS reader) as the textbook. There is no feature development here.

- `main` — design authority (SPEC/PLAN/DESIGN). Not used on this branch; its
  spec files were removed here on purpose. Do not re-create them.
- `rust` — upstream implementation. **This branch must always rebase onto
  `rust`** (`git rebase rust`) when pulling upstream changes — never merge.
  Teaching comments are the only intended divergence; conflicts should be
  resolved in favor of `rust`'s code plus re-applied comments.
- This branch diverges from `rust` by carrying teaching comments.

## Comment convention (the whole point of this branch)

Every file in `src/` carries **bilingual (English + 中文) teaching comments**:

- `//!` module header: what the file does + which Rust concepts it demonstrates
- `///` doc comment on every struct / enum / fn: purpose + data flow
- `//` inline at teaching moments: ownership/move, `&`/`&mut`, `Result`/`?`,
  `Option`, traits, closures, lifetimes, `String` vs `&str`, iterators,
  crate-specific idioms
- C analogies where apt (`Option<T>` ≈ NULL-check enforced by the compiler,
  `String` ≈ `malloc`'d `char*` with auto-free, `match` ≈ exhaustive switch)
- 中文 mirror line directly under each EN line, terse
- For repetitive blocks: comment the pattern once, not every arm

### Hard rules for agents touching src/

1. **Comments only unless asked otherwise.** Never reformat, rename, reorder,
   or "improve" working code. The user reads this code to learn; churn is harm.
2. After editing, verify zero behavior change (e.g. diff with comments
   stripped must be identical to before) and run `cargo check`.
3. Match the existing comment style exactly — read `src/model.rs` first as
   the reference example.
4. Do NOT add tests, features, deps, or refactor. If you spot a bug or
   improvement, report it — don't fix it unasked.

## Build & verify

```sh
cargo check          # fast type-check
cargo build --release
cargo test           # only if tests already exist; do not write new ones
```

## Collaboration rules

- Never `git push` without the user's explicit approval. Local commits are fine.
- Explain risky edits and destructive commands before executing.
