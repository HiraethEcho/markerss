# Bug report — DONE ✅ (archived 2026-08)

Superseded by DESIGN.md — "Flag toggles never rebuild the list":
- `L`/`S`/`u`/`a`/`A` mutate the in-memory snapshot in place; no rebuild, no reorder, selection stable.
- Resize redraw fixed separately (first non-key event was dropped).

---

the list changes when i press L, S in article pane. i hope the list pane keep still unless i mannually refresh it, or nav in nav pane, or change sort.