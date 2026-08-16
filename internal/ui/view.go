package ui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// paneWidths computes nav/list/article widths from config pane_ratio.
func (m *Model) paneWidths(w int) (navW, listW, artW int) {
	r := m.cfg.PaneRatio
	navW = int(float64(w) * r[0])
	listW = int(float64(w) * r[1])
	if navW < 12 {
		navW = 12
	}
	if listW < 20 {
		listW = 20
	}
	artW = w - navW - listW
	if artW < 12 {
		artW = 12
	}
	return
}

func (m *Model) articleWidth() int {
	_, _, artW := m.paneWidths(m.width)
	if m.fullscreen {
		return m.width
	}
	return artW
}

func (m *Model) View() string {
	if m.width == 0 {
		return "loading…"
	}
	navW, listW, artW := m.paneWidths(m.width)
	h := m.height - 1 // footer

	var body string
	if m.fullscreen {
		body = m.articleView(m.width, m.height-1)
	} else {
		nav := m.navView(navW, h)
		list := m.listView(listW, h)
		art := m.articleView(artW, h)
		body = lipgloss.JoinHorizontal(lipgloss.Top, nav, list, art)
	}

	footer := m.footerView()
	out := body + "\n" + footer
	if m.helpOpen {
		out = m.overlayOn(out, m.helpBox())
	}
	if m.inputMode != inputNone {
		out = m.overlayOn(out, m.inputBox())
	}
	return out
}

// overlayOn paints a centered floating box over the base view, keeping
// the background visible. Escape-safe: composes left/base/right instead
// of splatting runes (which corrupts ANSI under color profiles).
func (m *Model) overlayOn(base, box string) string {
	lines := strings.Split(base, "\n")
	boxLines := strings.Split(box, "\n")
	boxW := 0
	for _, bl := range boxLines {
		boxW = max(boxW, lipgloss.Width(bl))
	}
	padX := max(0, (m.width-boxW)/2)
	padY := max(0, (m.height-len(boxLines))/2)
	for i, bl := range boxLines {
		y := padY + i
		if y < 0 || y >= len(lines) {
			continue
		}
		base := lines[y]
		left := truncateW(base, padX)
		tail := cutPrefixW(cutPrefixW(base, lipgloss.Width(left)), boxW)
		lines[y] = left + bl + tail
	}
	return strings.Join(lines, "\n")
}

// cutPrefixW drops the first n display columns of s (ANSI-aware).
func cutPrefixW(s string, n int) string {
	if n <= 0 {
		return s
	}
	var b strings.Builder
	w := 0
	inEsc := false
	keep := false
	for _, r := range s {
		if r == '\x1b' {
			inEsc = true
			if keep {
				b.WriteRune(r)
			}
			continue
		}
		if inEsc {
			if keep {
				b.WriteRune(r)
			}
			if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') {
				inEsc = false
			}
			continue
		}
		if keep {
			b.WriteRune(r)
			continue
		}
		rw := runeWidth(r)
		if w+rw > n {
			keep = true
			b.WriteRune(r)
		} else {
			w += rw
		}
	}
	return b.String()
}

// inputBox builds the floating input prompt box.
func (m *Model) inputBox() string {
	label := m.inputLabel()
	runes := []rune(m.inputVal)
	cur := m.inputCur
	if cur > len(runes) {
		cur = len(runes)
	}
	shown := ""
	if len(runes) > 0 {
		if cur < len(runes) {
			shown = string(runes[:cur]) + stySel.Render(string(runes[cur])) + string(runes[cur+1:])
		} else {
			shown = string(runes) + stySel.Render(" ")
		}
	} else {
		shown = stySel.Render(" ")
	}
	content := styInput.Render(label) + shown
	w := lipgloss.Width(content) + 6
	if w > m.width-4 {
		w = m.width - 4
	}
	s := lipgloss.NewStyle().
		Width(w).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("240")).
		Background(lipgloss.Color("235"))
	body := content + "\n" + styDim.Render("enter commit · esc cancel")
	return s.Render(body)
}

func (m *Model) footerView() string {
	var hints []string
	if m.refreshing {
		hints = append(hints, spinnerGlyph(m.spinner)+" refreshing")
	}
	switch m.focus {
	case paneNav:
		hints = append(hints, "j/k move · N new · d delete · M rename · t preset")
	case paneList:
		hints = append(hints, "j/k move · enter open · a toggle · A all · n/p unread")
	case paneArticle:
		hints = append(hints, "j/k scroll · enter fetch · n/p item · a/o/e")
	}
	hints = append(hints, "? help · Q quit")
	left := strings.Join(hints, "  ·  ")
	right := m.status
	if right != "" {
		budget := m.width - lipgloss.Width(left) - 3
		right = truncateW(right, max(budget, 0))
		if right != "" {
			left = left + " | " + right // plain sep: ANSI inside Width() mis-wraps
		}
	}
	left = truncateW(left, m.width)
	return lipgloss.NewStyle().Width(m.width).Render(left)
}

func spinnerGlyph(n int) string {
	frames := []string{"⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"}
	return frames[n%len(frames)]
}

func paneStyle(w, h int, focused bool, title string) lipgloss.Style {
	s := lipgloss.NewStyle().
		Width(w).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("240"))
	if h > 2 {
		s = s.Height(h - 2) // border adds 2 → total h
	}
	if focused {
		s = s.BorderForeground(lipgloss.Color("39"))
	}
	return s
}
