# Tasks — reading-advanced

## 1. Reading polish
- [x] 80-col reading width, left-aligned, centered
- [x] Dynamic header height (summary fully visible, CJK-aware width estimate)
- [x] Scrollbar (right edge) + scroll clamp
- [x] Theme file → markdown Theme + accent/dim; named colors; defaults on error
- [x] Element styling via the_other_tui_markdown Theme (headings/code/quote/list/table/hr)

## 2. Efficiency keys
- [x] gg/G top/bottom (nav+list+article)
- [x] Ctrl+f/Ctrl+b full page (list+article)
- [x] / live search (enter keep, esc restore)
- [x] Sort stack st/sn/sf/su + sT/sN/sF/sU reversed (per-level), keep 3, `sort` config seeds
- [x] yy/yn/yf OSC52 copy (hand-rolled base64)
- [x] s/y status-bar combo help
- [x] foldlevel config (initial fold depth; no fold keys)

## 3. Verify
- [x] cargo build + cargo test green (43 tests)
- [x] cargo clippy clean
- [x] lightspec validate reading-advanced --strict
