# Feature request — DONE ✅ (archived 2026-08)

Superseded by DESIGN.md / SPEC.md:
- `E` (nav) appends saved-article list (`title url summary`) to `export_saved_path` → DESIGN.md Export / MVP
- Lazy feeds (`!lazy`, `L` nav toggle, Lazy section, auto-refresh skip) → DESIGN.md Storage / Tags & Favorites

---

# export
export saved articles as a list like
```plain text
title url summary
title2 url2 summary2
```
to stdout

# lazy load

add lazy load list. do not refresh feeds in that list at start. only pull them when manually use `r` to refresh
maybe add !lazy like !favourite. use `L` in nav pane to tag as lazy