# AGENTS

This repository's `main` branch is DESIGN-ONLY: markdown recording intent, roadmap, and design. No code, no lightspec/. Runs sdd-lite: SPEC/PLAN/DESIGN, no lightspec/.

## Repo Structure

- `main` — design authority, language-agnostic. Five parallel specs, each a `##` section in SPEC.md / PLAN.md / DESIGN.md:
  - `## MVP` — core three-pane TUI reader
  - `## Config` — app settings (cache TTL, export dir, refresh)
  - `## Tags & Favorites` — tags strip in lower nav + favorites as special category
  - `## Article Polish` — comfortable long-form article rendering
  - `## Advanced` — OPML mapping (nested categories, tags)
- Implementation on parallel branches, one per language (full sdd, own AGENTS.md + lightspec/):
  - `rust` (Rust + ratatui + feed-rs)
  - `go` (Go + bubbletea + gofeed)
  - `cpp` (C++ + FTXUI + libcurl, newsboat source as reference)

## Role of Files in main

| File        | Role                                                                                              |
| ----------- | ------------------------------------------------------------------------------------------------- |
| `SPEC.md`   | Intent summary — 3 specs, each: goal + what + decisions                                           |
| `DESIGN.md` | Design detail — 3 specs, each: behavior + decisions. Design authority; changes land here first    |
| `AGENTS.md` | This file — repo structure + workflow for agents                                                  |
| `README.md` | Human-facing — repo layout, usage, build & run                                                    |

## Workflow

Edit design docs on main → commit → **rebase branches only when the user asks** (never auto-rebase) → implement against DESIGN.md. Each branch implements ALL five specs, in order MVP → Config → Tags & Favorites → Article Polish → Advanced. Tick the branch's checkboxes in PLAN.md as phases complete.

## Collaboration Rules

- **Never `git push` without the user's explicit approval** — local commits and rebases are fine; pushing to any remote requires a "push it" / "push" go-ahead first.
