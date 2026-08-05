# AGENTS

This repository's `main` branch is DESIGN-ONLY: markdown recording intent, roadmap, and design. No code, no lightspec/. Runs sdd-lite: SPEC/PLAN/DESIGN, no lightspec/.

## Repo Structure

- `main` — design authority, language-agnostic. Five parallel specs, each a `##` section in SPEC.md / PLAN.md / DESIGN.md:
  - `## MVP` — core three-pane TUI reader
  - `## Config` — app settings (cache TTL, export dir, refresh)
  - `## Tags & Favorites` — tags strip in lower nav + favorites as special category
  - `## Article Polish` — comfortable long-form article rendering
  - `## Advanced` — OPML mapping (nested categories, tags)
- Implementation in parallel sibling worktrees, one branch each (full sdd, own AGENTS.md + lightspec/):
  - `rust` → `../markerss-rust` (Rust + ratatui + feed-rs)
  - `go` → `../markerss-go` (Go + bubbletea + gofeed)
  - `cpp` → `../markerss-cpp` (C++ + FTXUI + libcurl, newsboat source as reference)

## Role of Files in main

| File        | Role                                                                                              |
| ----------- | ------------------------------------------------------------------------------------------------- |
| `SPEC.md`   | Intent summary — 3 specs, each: goal + what + decisions                                           |
| `PLAN.md`   | Roadmap + per-branch progress — `##` = spec, `###` = branch (rust/go/cpp), phases with checkboxes |
| `DESIGN.md` | Design detail — 3 specs, each: behavior + decisions. Design authority; changes land here first    |
| `AGENTS.md` | This file — repo structure + workflow for agents                                                  |
| `README.md` | Human-facing — repo layout, usage, build & run                                                    |

## Workflow

### main

Edit design docs on main → commit → **rebase branches only when the user asks** (never auto-rebase) → implement against DESIGN.md. Each branch implements ALL five specs, in order MVP → Config → Tags & Favorites → Article Polish → Advanced. Tick the branch's checkboxes in PLAN.md as phases complete.

### dev branches

SDD Workflow (default tier)

| Phase           | Command             |
| --------------- | ------------------- |
| Init (default)  | `/sdd-init`         |
| Proposal        | `/spec`             |
| Plan            | `/plan`             |
| Apply           | `/build`            |
| Test            | `/test`             |
| Review          | `/review`           |
| Ship            | `/ship`             |
| Pause / handoff | `/rest`             |
| Pickup          | `/pickup`           |
| Archive         | `lightspec archive` |

- New session / interrupted → `/pickup`
- Vague request → discovery (`discovery` + `idea-refine`) before spec
- Coding → `building` (thin slices, ponytail); testing → `test-driven-development`
- Task self-review → `reviewing`; security-sensitive → `security-and-hardening`
- Design updates land on `main` → `git rebase main` here before implementing
