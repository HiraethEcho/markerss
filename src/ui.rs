//! Pane rendering: nav / list / article / status / help / input.
//! Child module of main — may touch App private fields.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::model::Item;
use crate::util::{display_width, fmt_date};
use crate::{App, InputMode, InputPrompt, Scope, TreeRow};

pub(crate) fn render(frame: &mut Frame, app: &mut App) {
    let [main, status_bar] = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
    if app.fullscreen {
        // full-screen focus on the article pane
        app.article_area = main;
        draw_article(frame, main, app);
        draw_status(frame, status_bar, app);
        if app.show_help {
            draw_help(frame, frame.area(), app);
        }
        if let Some(prompt) = &app.input {
            draw_input(frame, frame.area(), prompt);
        }
        return;
    }
    let r = [
        app.cfg.pane_ratio[0].max(0.0),
        app.cfg.pane_ratio[1].max(0.0),
        app.cfg.pane_ratio[2].max(0.0),
    ];
    let [nav, list, article] = Layout::horizontal([
        Constraint::Percentage((r[0] * 100.0) as u16),
        Constraint::Percentage((r[1] * 100.0) as u16),
        Constraint::Percentage((r[2] * 100.0) as u16),
    ])
    .areas(main);
    app.article_area = article;

    draw_nav(frame, nav, app);
    draw_list(frame, list, app);
    draw_article(frame, article, app);
    draw_status(frame, status_bar, app);

    if app.show_help {
        draw_help(frame, frame.area(), app);
    }
    if let Some(prompt) = &app.input {
        draw_input(frame, frame.area(), prompt);
    }
}

fn draw_nav(frame: &mut Frame, area: Rect, app: &App) {
    let mut items: Vec<ListItem> = Vec::new();
    let visible = (area.height as usize).saturating_sub(2).max(1);
    let offset = app.tree_sel.saturating_sub(visible.saturating_sub(1));
    let window: Vec<(usize, &TreeRow)> = app
        .tree_rows
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .collect();
    for (i, row) in window {
        let (text, style) = match row {
            TreeRow::Section(name) => {
                let prefix = if app.collapsed.contains(name) { "▸" } else { "▾" };
                (
                    format!("{prefix} {name}"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            }
            TreeRow::AllUnread => {
                let n = app.db.total_unread().unwrap_or(0);
                (format!("Unread ({n})"), Style::default().add_modifier(Modifier::BOLD))
            }
            TreeRow::ReadLater => {
                let n = app.db.items_with_flag("read_later").unwrap_or_default().len();
                (format!("Read Later ({n})"), Style::default().add_modifier(Modifier::BOLD))
            }
            TreeRow::Saved => {
                let n = app.db.items_with_flag("saved").unwrap_or_default().len();
                (format!("Saved ({n})"), Style::default().add_modifier(Modifier::BOLD))
            }
            TreeRow::Favourite => {
                let n: usize = app
                    .feeds
                    .feeds
                    .iter()
                    .filter(|f| f.favourite)
                    .map(|f| app.db.unread_count(&f.url).unwrap_or(0))
                    .sum();
                let prefix = if app.fav_expanded { "▾" } else { "▸" };
                (format!("{prefix} Favourite ({n})"), Style::default().add_modifier(Modifier::BOLD))
            }
            TreeRow::Uncategorized => {
                let n: usize = app
                    .feeds
                    .uncategorized()
                    .iter()
                    .map(|f| app.db.unread_count(&f.url).unwrap_or(0))
                    .sum();
                let prefix = if app.uncat_expanded { "▾" } else { "▸" };
                (
                    format!("{prefix} No Category ({n})"),
                    Style::default().add_modifier(Modifier::BOLD),
                )
            }
            TreeRow::FavouriteFeed(_, name) | TreeRow::UncategorizedFeed(_, name) => {
                let f = app
                    .feeds
                    .feeds
                    .iter()
                    .find(|x| x.display_name() == name.as_str());
                let n = f
                    .map(|x| app.db.unread_count(&x.url).unwrap_or(0))
                    .unwrap_or(0);
                let mark = f
                    .filter(|x| app.feed_errors.contains_key(&x.url))
                    .map(|_| " !")
                    .unwrap_or("");
                (format!("  {name} ({n}){mark}"), Style::default())
            }
            TreeRow::Category(cat) => {
                let n: usize = app
                    .feeds
                    .by_category(cat)
                    .iter()
                    .map(|f| app.db.unread_count(&f.url).unwrap_or(0))
                    .sum();
                let prefix = if app.collapsed.contains(cat) { "▸" } else { "▾" };
                let indent = cat.matches('/').count() * 2 + 2;
                (format!("{}{prefix} {cat} ({n})", " ".repeat(indent)), Style::default())
            }
            TreeRow::Feed(url, name, indent) => {
                let n = app.db.unread_count(url).unwrap_or(0);
                let mark = if app.feed_errors.contains_key(url.as_str()) { " !" } else { "" };
                (
                    format!("{}{} ({n}){mark}", " ".repeat(*indent as usize), name),
                    Style::default(),
                )
            }
            TreeRow::Tag(t) => {
                let n: usize = app
                    .feeds
                    .feeds
                    .iter()
                    .filter(|f| f.has_tag(t))
                    .map(|f| app.db.unread_count(&f.url).unwrap_or(0))
                    .sum();
                let prefix = if app.collapsed.contains(&format!("tag:{t}")) {
                    "▸"
                } else {
                    "▾"
                };
                (format!("  {prefix} {t} ({n})"), Style::default())
            }
        };
        let item = ListItem::new(text);
        // top-level entries get a highlight fg (yellow) on top of their base style
        let is_top = matches!(
            row,
            TreeRow::Section(_)
                | TreeRow::AllUnread
                | TreeRow::ReadLater
                | TreeRow::Saved
                | TreeRow::Favourite
                | TreeRow::Uncategorized
        );
        let base = if is_top {
            style.patch(Style::default().fg(Color::Yellow))
        } else {
            style
        };
        // patch selection into the row style so base styles (bold etc.) survive
        let row_style = if i == app.tree_sel {
            if app.focus == 0 {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::Yellow)
            }
        } else {
            Style::default()
        };
        items.push(item.style(base.patch(row_style)));
    }
    frame.render_widget(
        List::new(items).block(pane_block("Nav", app.focus == 0)),
        area,
    );
}

fn draw_list(frame: &mut Frame, area: Rect, app: &App) {
    let mut items: Vec<ListItem> = Vec::new();
    if app.scoped_items.is_empty() {
        items.push(ListItem::new("no items — r to refresh"));
    }
    // window around the selection so long lists scroll with the cursor
    let visible = (area.height as usize).saturating_sub(2).max(1);
    let offset = app.list_sel.saturating_sub(visible.saturating_sub(1));
    let window: Vec<(usize, &(String, Item))> = app
        .scoped_items
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .collect();
    for (i, (url, item)) in window {
        let read = app.db.is_read(url, &item.guid).unwrap_or(false);
        let marker = if read { " " } else { "•" };
        let flags = if item.saved && item.read_later {
            " [SL]"
        } else if item.saved {
            " [S]"
        } else if item.read_later {
            " [L]"
        } else {
            ""
        };
        let text = format!("{marker} {}{flags}", item.display_title());
        let mut li = ListItem::new(text);
        if i == app.list_sel {
            let style = if app.focus == 1 {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::Yellow)
            };
            li = li.style(style);
        } else if read {
            li = li.style(Style::default().fg(Color::DarkGray));
        }
        items.push(li);
    }
    let title = match &app.scope {
        Scope::AllUnread => "All Unread".to_string(),
        Scope::ReadLater => "Read Later".to_string(),
        Scope::Saved => "Saved".to_string(),
        Scope::Category(c) => c.clone(),
        Scope::Feed(u) => app
            .feeds
            .feeds
            .iter()
            .find(|f| &f.url == u)
            .map(|f| f.display_name().to_string())
            .unwrap_or_else(|| u.clone()),
        Scope::Tag(t) => format!("#{t}"),
    };
    frame.render_widget(
        List::new(items).block(pane_block(&title, app.focus == 1)),
        area,
    );
}


/// Build the styled article body Text (HTML → markdown → tui-markdown).
/// Images render as `[img] desc (url)` fallback text so the draw pass can
/// locate rows and fetch/overlay real pictures.
fn render_article_text(app: &App, item: &Item) -> Text<'static> {
    let md = app.article_markdown_display(item);
    if md.trim().is_empty() {
        return Text::from(""); // caller falls back to the fetch hint
    }
    let options = tui_markdown::Options::new(app.theme.styles.clone())
        .image_fallback(tui_markdown::ImageFallback::AltTextAndUrl);
    let text = tui_markdown::from_str_with_options(&md, &options);
    let lines: Vec<Line<'static>> = text
        .lines
        .into_iter()
        .map(|l| {
            let spans = l
                .spans
                .into_iter()
                .map(|s| Span::styled(s.content.into_owned(), s.style))
                .collect();
            Line { spans, style: l.style, alignment: l.alignment }
        })
        .collect();
    Text::from(lines)
}

/// Header text: title / meta (feed · date · read · flags) / url / summary.
fn article_header<'a>(app: &App, url: &str, item: &'a Item, feed_name: &'a str) -> Text<'a> {
    let title_style = Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD);
    let meta_style = Style::default().fg(app.theme.dim);
    let dim_style = Style::default().fg(app.theme.dim);
    let read_mark = if app.db.is_read(url, &item.guid).unwrap_or(false) {
        "read"
    } else {
        "unread"
    };
    let flags_mark = if item.saved && item.read_later {
        " [SL]"
    } else if item.saved {
        " [S]"
    } else if item.read_later {
        " [L]"
    } else {
        ""
    };
    Text::from(vec![
        Line::from(Span::styled(item.display_title().to_string(), title_style)),
        Line::from(Span::styled(
            format!("{feed_name}  ·  {}  ·  {read_mark}{flags_mark}", fmt_date(&item.date)),
            meta_style,
        )),
        Line::from(Span::styled(item.url.clone(), dim_style)),
        Line::from(""),
        Line::from(Span::styled(item.summary.trim(), Style::default())),
    ])
}

fn draw_article(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some((url, item)) = app.current_item() else {
        frame.render_widget(
            Paragraph::new("select an article")
                .block(pane_block("Article", app.focus == 2)),
            area,
        );
        return;
    };
    let feed_name = app
        .feeds
        .feeds
        .iter()
        .find(|f| f.url.as_str() == url.as_str())
        .map(|f| f.display_name().to_string())
        .unwrap_or_default();

    let in_article = app.focus == 2;
    // list mode: summary only; article mode: feed content, then fetch hint
    let content_ready = in_article && !item.content.trim().is_empty();

    // Fixed header (title/meta/summary), content scrolls below a separator.
    let block = pane_block("Article", app.focus == 2);
    let inner = block.inner(area);
    // dynamic header height so a long summary is fully visible
    let summary = item.summary.trim();
    let summary_w = (inner.width.saturating_sub(2)).max(1) as usize;
    let summary_lines = if summary.is_empty() {
        1
    } else {
        display_width(summary).div_ceil(summary_w).clamp(1, 8)
    };
    let head_h = (4 + summary_lines) as u16;
    let [head, sep, body] = Layout::vertical([
        Constraint::Length(head_h),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(inner);

    let header_text = article_header(app, &url, &item, feed_name.as_str());
    frame.render_widget(Paragraph::new(header_text).wrap(Wrap { trim: true }), head);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(sep.width as usize),
            Style::default().fg(app.theme.dim),
        )),
        sep,
    );

    // Body: feed/fetched HTML → markdown → styled ratatui Text
    // (h2md → tui-markdown pipeline).
    // Links render as underlined alt text (no URL); images as [img].
    let body_text = if content_ready {
        // render once per guid; reuse until the item's content changes
        if !matches!(&app.article_render, Some((g, _)) if g == &item.guid) {
            let t = render_article_text(app, &item);
            app.article_render = Some((item.guid.clone(), t.clone()));
        }
        app.article_render.as_ref().map(|(_, t)| t.clone()).unwrap_or_default()
    } else if app.fetching {
        Text::from("fetching…")
    } else if in_article {
        // in the article pane but the feed has no content — blank until fetched
        Text::from("")
    } else {
        Text::from("l/enter to read")
    };
    // reading width: cap (and center) only when configured (>0 = unlimited)
    let content_w = if app.cfg.reading_width > 0 {
        body.width.min(app.cfg.reading_width as u16)
    } else {
        body.width
    };
    let x_off = (body.width - content_w) / 2;

    let content_w_est = content_w.max(1) as usize;

    frame.render_widget(block, area);

    // ── TUI images ─────────────────────────────────────────────────────────
    // Replace `[img]` rows with image-height blank rows (exact layout) and
    // overlay decoded pictures on the blanks. `scroll` is clamped to the
    // expanded text so images never push the bottom out of reach.
    // clamp scroll to the estimated wrapped content height
    let est_lines: u16 = body_text
        .lines
        .iter()
        .map(|l| {
            let t: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
            display_width(&t).div_ceil(content_w_est).max(1) as u16
        })
        .sum();
    let max_scroll = est_lines.saturating_sub(body.height);
    let scroll = app.article_scroll.min(max_scroll);
    // write back only in article mode — in list mode the body is a one-line
    // hint and clamping would zero the saved article position
    if in_article {
        app.article_scroll = scroll;
    }
    let total_lines = body_text.lines.len() as u16;
    let para = Paragraph::new(body_text)
        .wrap(Wrap { trim: true })
        .scroll((scroll, 0));
    let content_area = Rect {
        x: body.x + x_off,
        y: body.y,
        width: content_w,
        height: body.height,
    };
    frame.render_widget(para, content_area);

    // scrollbar on the pane's right edge
    let total = total_lines;
    if total > body.height && scroll > 0 {
        let bar_h = ((body.height as f32 * body.height as f32 / total as f32).max(1.0)) as u16;
        let bar_y = (scroll as f32 * (body.height - bar_h) as f32 / max_scroll as f32) as u16;
        let sb_area = Rect {
            x: body.x + body.width - 1,
            y: body.y + bar_y,
            width: 1,
            height: bar_h,
        };
        frame.render_widget(
            Paragraph::new(" ").style(Style::default().bg(app.theme.dim)),
            sb_area,
        );
    }

}

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let line = format!(
        "{}  |  ? help  Q quit  F favourite  J/K unread  n/p parent  l/enter open  r fetch  R refresh",
        app.status
    );
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_help(frame: &mut Frame, area: Rect, app: &App) {
    let text = Text::from(
        "Keys\n\
         ─────\n\
         nav:   j/k move · h/l expand+descend · N new feed · D delete · M rename · F favourite\n\
         list:  j/k move · l/enter open · / search (enter keep, left stop) · J/K next/prev unread\n\
         article: j/k scroll · n/p parent (move list) · ctrl+u/d half page · ctrl+f/b full page\n\
         left:  h/q/esc — article→list→nav→parent\n\
         right: l/enter — expand→list→article→fetch\n\
         jump:  gg/G top/bottom (nav+list+article)\n\
         sort:  st/sn/sf/su forward · sT/sN/sF/sU reversed — time/title/feed/unread\n\
         copy:  yy url · yn title · yp feed url\n\
         global: o browser · e export · a read · A all-read · L/S flags · r/R refresh\n\
         i/x OPML · t preset · tab focus · Q quit · ? help\n\n\
         export → $XDG_DATA_HOME/markerss/<category>/<slug>.md",
    );
    // floating opaque window, default colors, scrollable with j/k
    let w = (area.width * 3 / 4).max(40);
    let h = (area.height * 3 / 4).max(10);
    let rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    };
    let block = Block::default().borders(Borders::ALL).title("Help");
    frame.render_widget(ratatui::widgets::Clear, rect);
    frame.render_widget(
        Paragraph::new(text).block(block).scroll((app.help_scroll, 0)),
        rect,
    );
}

fn draw_input(frame: &mut Frame, area: Rect, prompt: &InputPrompt) {
    let text = Text::from(format!("{} {}", prompt.prompt, prompt.buf));
    let block = Block::default()
        .borders(Borders::ALL)
        .title("input (esc cancel)")
        .style(Style::default().bg(Color::Blue));
    if prompt.mode == InputMode::Search {
        // search box floats under the list pane
        let box_rect = Rect {
            x: area.x + area.width / 6,
            y: area.y + 1,
            width: area.width * 2 / 3,
            height: 3,
        };
        frame.render_widget(ratatui::widgets::Clear, box_rect);
        frame.render_widget(
            Paragraph::new(text).block(block.title("search (enter keep · esc restore)")),
            box_rect,
        );
        return;
    }
    let box_rect = Rect {
        x: area.x + area.width / 4,
        y: area.y + area.height / 2,
        width: area.width / 2,
        height: 3,
    };
    frame.render_widget(ratatui::widgets::Clear, box_rect);
    frame.render_widget(Paragraph::new(text).block(block), box_rect);
}

fn pane_block<'a>(title: &'a str, focused: bool) -> Block<'a> {
    let color = if focused { Color::Yellow } else { Color::DarkGray };
    Block::bordered().title(title).border_style(Style::default().fg(color))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_roundtrip() {
        let html = "<h2>Title</h2><p>Hello <b>bold</b> <a href=\"https://e.com\">link</a></p>";
        let md = crate::fetch::html_to_markdown(html);
        assert!(md.contains("Title"), "got: {md}");
        assert!(md.contains("**bold**"), "got: {md}");
        assert!(md.contains("[link](https://e.com)"), "got: {md}");
    }
}
