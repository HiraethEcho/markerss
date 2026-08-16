package ui

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

var helpKeys = [][2]string{
	{"h/q/esc/←", "left: article → list → nav; fold → fold parent"},
	{"l/enter/→", "right: expand (cursor → first child) → list → article+read → fetch full"},
	{"j/k", "move / scroll (arrows too)"},
	{"n/p", "list: mark read + jump unread · article: next/prev item"},
	{"a / A", "toggle read / mark all in view read"},
	{"ctrl+u / ctrl+d", "article: half-page scroll (pgup/pgdown)"},
	{"o / e", "open in browser / export (path prompt prefilled)"},
	{"N / d / M / T", "nav: new feed / delete (x2) / rename (context) / edit tags"},
	{"F", "nav: favourite feed · article: fullscreen"},
	{"t / r / R", "cycle nav preset / partial refresh / full refresh"},
	{"i / x", "import / export OPML"},
	{"tab / shift+tab", "focus next / prev pane"},
	{"?", "this help"},
	{"Q", "quit"},
}

func (m *Model) updateHelp(msg tea.KeyMsg) {
	switch msg.String() {
	case "j", "down":
		m.helpScroll++
	case "k", "up":
		m.helpScroll--
	case "q", "esc", "?", "enter":
		m.helpOpen = false
	}
	if m.helpScroll < 0 {
		m.helpScroll = 0
	}
}

// helpBox builds the floating help window content.
func (m *Model) helpBox() string {
	lines := []string{styTitle.Render("markerss — keys")}
	lines = append(lines, "")
	for _, kv := range helpKeys {
		lines = append(lines, "  "+styAccent.Render(kv[0])+"  "+kv[1])
	}
	lines = append(lines, "", styDim.Render("j/k scroll · q close"))

	boxW := 72
	if m.width < boxW+4 {
		boxW = m.width - 4
	}
	if boxW < 24 {
		boxW = 24
	}
	// cap lines to the box width so it stays compact
	inner := boxW - 4
	for i, l := range lines {
		lines[i] = truncateW(l, inner)
	}
	boxH := 24
	if m.height < boxH+2 {
		boxH = m.height - 2
	}
	innerH := boxH - 2
	start := m.helpScroll
	if start > len(lines) {
		start = len(lines)
	}
	end := min(start+innerH, len(lines))
	body := strings.Join(lines[start:end], "\n")
	if end-start < innerH {
		body += strings.Repeat("\n", inner-(end-start))
	}
	return lipgloss.NewStyle().
		Width(boxW).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("240")).
		Background(lipgloss.Color("235")).
		Render(body)
}

func padRight(s string, w int) string {
	if lipgloss.Width(s) >= w {
		return s
	}
	return s + strings.Repeat(" ", w-lipgloss.Width(s))
}
