package ui

import (
	"strings"
	"testing"

	"github.com/charmbracelet/lipgloss"
	"github.com/muesli/termenv"
)

func TestTruncateWStyledCut(t *testing.T) {
	lipgloss.SetColorProfile(termenv.TrueColor)
	in := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("220")).Render("    https://sliun.com/feed/ (0)")
	out := truncateW(in, 22)
	if !strings.Contains(out, "\x1b[0m") {
		t.Error("missing closing reset — style bleeds into padding/border")
	}
	if !strings.Contains(out, "…") {
		t.Error("expected ellipsis")
	}
	if lipgloss.Width(out) > 22 {
		t.Errorf("width %d > 22", lipgloss.Width(out))
	}
}

func TestTruncateWExactFitAndPlain(t *testing.T) {
	p := "exactly-22-chars-here!"
	if got := truncateW(p, 22); got != p {
		t.Errorf("exact fit changed: %q", got)
	}
	o := truncateW("a very long plain text line here", 12)
	if !strings.HasSuffix(o, "…") || lipgloss.Width(o) > 12 {
		t.Errorf("plain cut wrong: %q", o)
	}
	styled := lipgloss.NewStyle().Foreground(lipgloss.Color("220")).Render("short")
	if got := truncateW(styled, 30); got != styled {
		t.Errorf("styled no-cut changed: %q", got)
	}
}
