# Tasks — keys-render-perf

## 1. Keybinding system
- [x] keybindings map (config.toml + standalone keybindings.toml override)
- [x] Multi-key arrays + angle-bracket special keys (<enter> <esc> <tab> …)
- [x] Unified combo map (Action enum, DEFAULT_KEYS, prefix-buffer matching in on_key)
- [x] New default map (F/D/n/p/J/K/gg/yp/sorts) + arrow keys restored
- [x] keybindings.toml reference written to user config

## 2. Render pipeline
- [x] h2md replaces html2md (no residual tags, aligned tables)
- [x] tui-markdown replaces the_other_tui_markdown (GFM, code, StyleSheet→theme)
- [x] Reading width config (0 = fill)

## 3. Performance
- [x] Redraw only on input/messages
- [x] No per-frame article Text clone; scroll clamp writeback

## 4. Cleanup
- [x] Remove image system + proxy (+2 deps)
- [x] Simplify: 8 sort variants → Action::Sort{level,reverse}; Favourite dual-guard merge

## 5. Verify
- [x] cargo build + cargo test green (49)
- [x] cargo clippy clean
- [x] lightspec validate keys-render-perf --strict
