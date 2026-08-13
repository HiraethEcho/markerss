# Handoff — 2026-08-06

## State
- [ ] change: state-unread-counts (rust) — 0/13 tasks — next: user approval → build Phase 3
- [x] change: scaffold-parse (rust) — done, committed e413f05, NOT archived
- [x] change: scaffold-parse (go) — done, committed b33b495, frozen
- [x] change: scaffold-parse (cpp) — done, committed 3d8be0c, frozen

## Pickup commands
- rust Phase 3 → approve `lightspec/changes/state-unread-counts/proposal.md`, then /pickup on `rust` branch
- scaffold-parse changes → `lightspec archive scaffold-parse` (each branch) when ready

## Notes
- Design lives on main — DESIGN.md is authority; branches rebase on main
- rust = chosen language (best code quality); go/cpp frozen, kept for reference
- Phase 2 wording fix: `~` = custom display name (NOT hidden) — corrected in DESIGN.md
- rust skeleton quirk: panics on non-tty (`ratatui::init`) — fix when doing error handling (Phase 10 polish)
- go test bug fixed: nil vs empty slice for Tags
- cpp: cmake absent → Makefile; FTXUI absent → ncursesw
