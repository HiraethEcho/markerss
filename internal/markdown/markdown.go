// Package markdown converts article HTML to markdown and renders
// markdown as styled terminal lines. Rendering rules per DESIGN.md:
// links = underlined alt text (URL hidden), images = [img], sub/sup as
// ~x~/^x^ markers. Export uses the markdown itself (URLs intact).
package markdown

import (
	"html"
	"regexp"
	"strings"
	"unicode"

	"github.com/JohannesKaufmann/html-to-markdown/v2/converter"
	"github.com/JohannesKaufmann/html-to-markdown/v2/plugin/base"
	"github.com/JohannesKaufmann/html-to-markdown/v2/plugin/commonmark"
	"github.com/charmbracelet/lipgloss"
)

var conv = converter.NewConverter(converter.WithPlugins(
	base.NewBasePlugin(),
	commonmark.NewCommonmarkPlugin(),
))

// HTMLToMD converts article/summary HTML to markdown. Falls back to
// stripped plain text on conversion error.
func HTMLToMD(src string) string {
	if strings.TrimSpace(src) == "" {
		return ""
	}
	// <sub>/<sup> → ~x~/^x^ markers (DESIGN: rendered sub/superscript).
	src = subRe.ReplaceAllString(src, "~$1~")
	src = supRe.ReplaceAllString(src, "^$1^")
	md, err := conv.ConvertString(src)
	if err != nil {
		return html.UnescapeString(tagRe.ReplaceAllString(src, " "))
	}
	return strings.TrimSpace(md)
}

var tagRe = regexp.MustCompile(`(?s)<[^>]+>`)
var subRe = regexp.MustCompile(`(?is)<sub[^>]*>(.*?)</sub>`)
var supRe = regexp.MustCompile(`(?is)<sup[^>]*>(.*?)</sup>`)

var (
	styTitle = lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("39"))
	styBold  = lipgloss.NewStyle().Bold(true)
	styItal  = lipgloss.NewStyle().Italic(true)
	styCode  = lipgloss.NewStyle().Foreground(lipgloss.Color("220"))
	styLink  = lipgloss.NewStyle().Underline(true).Foreground(lipgloss.Color("39"))
	styDim   = lipgloss.NewStyle().Foreground(lipgloss.Color("240"))
	styQuote = lipgloss.NewStyle().Foreground(lipgloss.Color("240"))
	styImg   = lipgloss.NewStyle().Faint(true)
	stySub   = lipgloss.NewStyle().Faint(true)
	stySup   = lipgloss.NewStyle().Faint(true)
)

// Render converts markdown to styled terminal lines, wrapped to width.
func Render(md string, width int) []string {
	var out []string
	inCode := false
	for _, raw := range strings.Split(md, "\n") {
		line := strings.TrimRight(raw, " \t")
		if strings.HasPrefix(line, "```") {
			inCode = !inCode
			if !inCode {
				out = append(out, "")
			}
			continue
		}
		if inCode {
			out = append(out, styCode.Render(line))
			continue
		}
		out = append(out, renderBlock(line, width)...)
	}
	return out
}

func renderBlock(line string, width int) []string {
	switch {
	case line == "":
		return []string{""}
	case strings.HasPrefix(line, "#"):
		n := 0
		for n < len(line) && line[n] == '#' {
			n++
		}
		text := strings.TrimSpace(line[n:])
		s := styBold
		if n == 1 {
			s = styTitle
		}
		return []string{s.Render(text)}
	case strings.HasPrefix(line, ">"):
		return []string{styQuote.Render("│ " + renderInline(strings.TrimPrefix(strings.TrimSpace(line), ">")))}
	case strings.HasPrefix(line, "-") || strings.HasPrefix(line, "*"):
		return []string{"  " + renderInline(line)}
	case regexp.MustCompile(`^\d+\.`).MatchString(line):
		return []string{"  " + renderInline(line)}
	case strings.Trim(line, "-_") == "" && strings.Contains(line, "---"):
		return []string{styDim.Render(strings.Repeat("─", width))}
	default:
		return strings.Split(wrap(renderInline(line), width), "\n")
	}
}

// inline tokenizer: images, links, bold, italic, code, sub, sup.
var reInline = regexp.MustCompile(
	`!\[([^\]]*)\]\([^)]*\)|` +
		`\[([^\]]+)\]\([^)]*\)|` +
		`\*\*([^*]+)\*\*|` +
		`\*([^*]+)\*|` +
		"`([^`]+)`|" +
		`~([^~]+)~|` +
		`\^([^^]+)\^`)

func renderInline(s string) string {
	if !strings.ContainsAny(s, "*[]`~^") {
		return s
	}
	var b strings.Builder
	last := 0
	for _, m := range reInline.FindAllStringSubmatchIndex(s, -1) {
		b.WriteString(s[last:m[0]])
		seg := s[m[0]:m[1]]
		switch {
		case strings.HasPrefix(seg, "!["):
			b.WriteString(styImg.Render("[img]"))
		case strings.HasPrefix(seg, "["):
			b.WriteString(styLink.Render(s[m[4]:m[5]]))
		case strings.HasPrefix(seg, "**"):
			b.WriteString(styBold.Render(s[m[6]:m[7]]))
		case strings.HasPrefix(seg, "*"):
			b.WriteString(styItal.Render(s[m[8]:m[9]]))
		case strings.HasPrefix(seg, "`"):
			b.WriteString(styCode.Render(s[m[10]:m[11]]))
		case strings.HasPrefix(seg, "~"):
			b.WriteString(stySub.Render(s[m[12]:m[13]]))
		case strings.HasPrefix(seg, "^"):
			b.WriteString(stySup.Render(s[m[14]:m[15]]))
		}
		last = m[1]
	}
	b.WriteString(s[last:])
	return b.String()
}

// wrap word-wraps styled text to width, preserving ANSI segments.
func wrap(s string, width int) string {
	if width <= 4 || lipgloss.Width(s) <= width {
		return s
	}
	var out []string
	var cur strings.Builder
	curW := 0
	for _, f := range strings.Fields(s) {
		fw := lipgloss.Width(f)
		if curW > 0 && curW+1+fw > width {
			out = append(out, cur.String())
			cur.Reset()
			curW = 0
		}
		if curW > 0 {
			cur.WriteRune(' ')
			curW++
		}
		cur.WriteString(f)
		curW += fw
	}
	if cur.Len() > 0 {
		out = append(out, cur.String())
	}
	return strings.Join(out, "\n")
}

// Slug turns a title into a filesystem-safe slug.
func Slug(s string) string {
	var b strings.Builder
	dash := false
	for _, r := range strings.ToLower(strings.TrimSpace(s)) {
		if unicode.IsLetter(r) || unicode.IsDigit(r) {
			b.WriteRune(r)
			dash = false
		} else if !dash {
			b.WriteRune('-')
			dash = true
		}
	}
	out := strings.Trim(b.String(), "-")
	if out == "" {
		return "untitled"
	}
	if len(out) > 80 {
		out = strings.TrimRight(out[:80], "-")
	}
	return out
}
