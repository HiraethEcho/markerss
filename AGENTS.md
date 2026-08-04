# AGENTS

<!-- LIGHTSPEC:START -->
This repository's `main` branch is DESIGN-ONLY: markdown recording intent (SPEC.md), roadmap (PLAN.md), and design (DESIGN.md). No code, no lightspec/.

Implementation happens in parallel sibling worktrees, one branch each:
- `rust` → `../markerss-rust` (Rust + ratatui + feed-rs)
- `go` → `../markerss-go` (Go + bubbletea + gofeed)
- `cpp` → `../markerss-cpp` (C++ + FTXUI + libcurl, newsboat source as reference)

Workflow: edit design docs on main → commit → each branch `git rebase main` → implement against DESIGN.md.
Each branch owns its own AGENTS.md (sdd workflow) and lightspec/.
<!-- LIGHTSPEC:END -->
