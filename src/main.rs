//! markerss — TUI RSS reader.
//!
//! Three panes: nav tree (categories → feeds) | item list | article.
//! Storage: SQLite (items + read state). Content: feed HTML → markdown →
//! styled Text (eilmeldung-style pipeline). Design authority: DESIGN.md.

mod config;
mod db;
mod feedlist;
mod fetch;
mod model;
mod opml;
mod xdg;

use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use model::Item;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::config::Config;
use crate::db::Db;
use crate::feedlist::{Feed, File};

// ─── worker messages ────────────────────────────────────────────────────────

enum Msg {
    FeedRefreshed {
        url: String,
        result: Result<Vec<Item>, String>,
        full: bool,
    },
    ArticleFetched { url: String, guid: String, result: Result<String, String> },
    RefreshTick,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum InputMode {
    AddUrl,
    AddTitle,
    AddCategory,
    AddTags,
    EditTags,
    RenameCategory,
    EditFeedTitle,
    EditTag,
    ImportOpml,
}

struct InputPrompt {
    mode: InputMode,
    prompt: String,
    buf: String,
}

// ─── app state ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Scope {
    AllUnread,
    ReadLater,
    Saved,
    Category(String),
    Feed(String),
    Tag(String),
}

struct App {
    cfg: Config,
    feeds: File,
    db: Db,

    // nav tree
    collapsed: std::collections::HashSet<String>,
    preset_idx: usize,
    fav_expanded: bool,
    uncat_expanded: bool,
    tree_sel: usize,
    tree_rows: Vec<TreeRow>,

    // list
    scope: Scope,
    list_sel: usize,
    scoped_items: Vec<(String, Item)>, // (feed_url, item)

    // article
    article_scroll: u16,
    fetching: bool,
    // rendered article body, keyed by item guid (avoid per-frame html2md)
    article_render: Option<(String, ratatui::text::Text<'static>)>,

    focus: usize, // 0 nav, 1 list, 2 article
    fullscreen: bool,
    delete_armed: bool,
    help_scroll: u16,
    status: String,
    show_help: bool,
    pending_refreshes: usize,
    article_area: Rect,
    running: bool,
    input: Option<InputPrompt>,
    add_pending: Option<String>,
    add_pending_title: Option<String>,
    add_pending_category: Option<Vec<String>>,
    edit_tags_url: Option<String>,
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
}

#[derive(Debug, Clone)]
enum TreeRow {
    Section(String),
    AllUnread,
    ReadLater,
    Saved,
    Favourite,
    FavouriteFeed(String, String), // url, display name
    Uncategorized,
    UncategorizedFeed(String, String), // url, display name
    Category(String),
    Feed(String, String, u8), // url, display name, indent
    Tag(String),
}

impl App {
    fn new(cfg: Config) -> App {
        let feeds = File::load_or_default(&cfg.urls_path);
        let db = Db::open(&cfg.db_path).expect("open sqlite db");
        let (tx, rx) = mpsc::channel();
        let mut app = App {
            cfg,
            feeds,
            db,
            collapsed: Default::default(),
            preset_idx: 0,
            fav_expanded: true,
            uncat_expanded: true,
            tree_sel: 0,
            tree_rows: Vec::new(),
            scope: Scope::AllUnread,
            list_sel: 0,
            scoped_items: Vec::new(),
            article_scroll: 0,
            fetching: false,
            article_render: None,
            focus: 0,
            fullscreen: false,
            delete_armed: false,
            help_scroll: 0,
            status: String::new(),
            show_help: false,
            pending_refreshes: 0,
            article_area: Rect::default(),
            running: true,
            input: None,
            add_pending: None,
            add_pending_title: None,
            add_pending_category: None,
            edit_tags_url: None,
            rx,
            tx,
        };
        app.rebuild_tree();
        app.apply_default_view();
        app.rebuild_list();
        app
    }

    fn apply_default_view(&mut self) {
        let Some(dv) = &self.cfg.default_view else { return };
        if let Some(url) = dv.strip_prefix("Feed:") {
            if self.feeds.feeds.iter().any(|f| f.url == url) {
                self.scope = Scope::Feed(url.to_string());
            }
        } else if let Some(cat) = dv.strip_prefix("Category:") {
            if self.feeds.categories().iter().any(|c| c == cat) {
                self.scope = Scope::Category(cat.to_string());
            }
        }
    }

    // ── tree ──────────────────────────────────────────────────────────────

    fn rebuild_tree(&mut self) {
        let preset = self
            .cfg
            .nav_presets
            .get(self.preset_idx)
            .cloned()
            .unwrap_or_else(|| {
                crate::config::DEFAULT_NAV_PRESET
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            });
        let mut rows: Vec<TreeRow> = Vec::new();
        for section in preset {
            // single-node sections (Unread / Read Later / Saved / Favourite)
            // render as the node itself; list sections get a foldable header
            let is_node_section = matches!(
                section.as_str(),
                "Unread" | "Read Later" | "Saved" | "Favourite"
            );
            if !is_node_section {
                rows.push(TreeRow::Section(section.clone()));
            }
            let section_collapsed = self.collapsed.contains(&section);
            if section_collapsed {
                continue;
            }
            match section.as_str() {
                "Unread" => rows.push(TreeRow::AllUnread),
                "Read Later" => rows.push(TreeRow::ReadLater),
                "Saved" => rows.push(TreeRow::Saved),
                "Favourite" => {
                    rows.push(TreeRow::Favourite);
                    if self.fav_expanded {
                        for f in self.feeds.feeds.iter().filter(|f| f.favourite) {
                            rows.push(TreeRow::FavouriteFeed(
                                f.url.clone(),
                                f.display_name().to_string(),
                            ));
                        }
                    }
                }
                "Categories" => {
                    for cat in self.feeds.categories() {
                        rows.push(TreeRow::Category(cat.clone()));
                        if !self.collapsed.contains(&cat) {
                            for f in self.feeds.by_category(&cat) {
                                rows.push(TreeRow::Feed(
                                    f.url.clone(),
                                    f.display_name().to_string(),
                                    4,
                                ));
                            }
                        }
                    }
                    // uncategorized feeds as their own foldable top node
                    rows.push(TreeRow::Uncategorized);
                    if self.uncat_expanded {
                        for f in self.feeds.uncategorized() {
                            rows.push(TreeRow::UncategorizedFeed(
                                f.url.clone(),
                                f.display_name().to_string(),
                            ));
                        }
                    }
                }
                "Tags" => {
                    for t in self.feeds.all_feed_tags() {
                        rows.push(TreeRow::Tag(t.clone()));
                        if !self.collapsed.contains(&format!("tag:{t}")) {
                            for f in self.feeds.feeds.iter().filter(|f| f.has_tag(&t)) {
                                rows.push(TreeRow::Feed(
                                    f.url.clone(),
                                    f.display_name().to_string(),
                                    4,
                                ));
                            }
                        }
                    }
                }
                "Feeds" => {
                    for f in self.feeds.feeds.iter() {
                        rows.push(TreeRow::Feed(f.url.clone(), f.display_name().to_string(), 2));
                    }
                }
                _ => {}
            }
        }
        self.tree_rows = rows;
        if self.tree_sel >= self.tree_rows.len() {
            self.tree_sel = self.tree_rows.len().saturating_sub(1);
        }
    }

    fn cycle_preset(&mut self) {
        if self.cfg.nav_presets.len() > 1 {
            self.preset_idx = (self.preset_idx + 1) % self.cfg.nav_presets.len();
            self.rebuild_tree();
        }
    }

    fn select_scope(&mut self, row: &TreeRow) {
        if matches!(row, TreeRow::Section(_)) {
            return;
        }
        self.scope = match row {
            TreeRow::Section(_) => return,
            TreeRow::AllUnread => Scope::AllUnread,
            TreeRow::ReadLater => Scope::ReadLater,
            TreeRow::Saved => Scope::Saved,
            TreeRow::Category(c) => Scope::Category(c.clone()),
            TreeRow::Feed(url, _, _) => Scope::Feed(url.clone()),
            TreeRow::FavouriteFeed(url, _) => Scope::Feed(url.clone()),
            TreeRow::UncategorizedFeed(url, _) => Scope::Feed(url.clone()),
            TreeRow::Tag(t) => Scope::Tag(t.clone()),
            TreeRow::Favourite | TreeRow::Uncategorized => Scope::AllUnread,
        };
        self.list_sel = 0;
        self.rebuild_list();
        self.focus = 1;
    }

    /// Follow nav selection into the list pane (preview) without moving focus.
    fn preview_scope(&mut self) {
        if let Some(row) = self.tree_rows.get(self.tree_sel).cloned() {
            if matches!(row, TreeRow::Section(_)) {
                return;
            }
            self.scope = match row {
                TreeRow::Section(_) => return,
                TreeRow::AllUnread => Scope::AllUnread,
                TreeRow::ReadLater => Scope::ReadLater,
                TreeRow::Saved => Scope::Saved,
                TreeRow::Category(c) => Scope::Category(c),
                TreeRow::Feed(url, _, _) => Scope::Feed(url),
                TreeRow::FavouriteFeed(url, _) => Scope::Feed(url),
                TreeRow::UncategorizedFeed(url, _) => Scope::Feed(url),
                TreeRow::Tag(t) => Scope::Tag(t),
                TreeRow::Favourite | TreeRow::Uncategorized => Scope::AllUnread,
            };
            self.list_sel = 0;
            self.rebuild_list();
        }
    }

    // ── list ──────────────────────────────────────────────────────────────

    fn rebuild_list(&mut self) {
        let mut items: Vec<(String, Item)> = Vec::new();
        match &self.scope {
            Scope::AllUnread => {
                for f in &self.feeds.feeds {
                    if let Ok(list) = self.db.items_for_feed(&f.url) {
                        for i in list {
                            // startup/refresh view = unread only
                            if !self.db.is_read(&f.url, &i.guid).unwrap_or(false) {
                                items.push((f.url.clone(), i));
                            }
                        }
                    }
                }
            }
            Scope::ReadLater => items = self.db.items_with_flag("read_later").unwrap_or_default(),
            Scope::Saved => items = self.db.items_with_flag("saved").unwrap_or_default(),
            Scope::Category(cat) => {
                for f in self.feeds.by_category(cat) {
                    if let Ok(list) = self.db.items_for_feed(&f.url) {
                        for i in list {
                            items.push((f.url.clone(), i));
                        }
                    }
                }
            }
            Scope::Feed(url) => {
                if let Ok(list) = self.db.items_for_feed(url) {
                    for i in list {
                        items.push((url.clone(), i));
                    }
                }
            }
            Scope::Tag(tag) => {
                for f in self.feeds.feeds.iter().filter(|f| f.has_tag(tag)) {
                    if let Ok(list) = self.db.items_for_feed(&f.url) {
                        for i in list {
                            items.push((f.url.clone(), i));
                        }
                    }
                }
            }
        }
        // stable order (date desc, as inserted by refresh) — never re-sort
        // on read toggles so the selection stays on the same item
        items.sort_by(|a, b| b.1.date.cmp(&a.1.date));
        self.scoped_items = items;
        if self.list_sel >= self.scoped_items.len() {
            self.list_sel = self.scoped_items.len().saturating_sub(1);
        }
    }

    fn select_item(&mut self, idx: usize) {
        if idx >= self.scoped_items.len() {
            return;
        }
        self.list_sel = idx;
        self.article_scroll = 0;
    }

    fn open_item(&mut self) {
        let Some((url, item)) = self.scoped_items.get(self.list_sel).cloned() else {
            return;
        };
        self.db.set_read(&url, &item.guid, true).ok();
        // reading clears read-later; do NOT rebuild the list — the item stays
        // visible in the current view until it's left or refreshed
        if item.read_later {
            self.db.set_flag(&url, &item.guid, "read_later", false).ok();
        }
        self.focus = 2;
        self.article_scroll = 0;
        // summary-only until opened — fetch full content only on explicit
        // <enter> in the article pane
        if item.content.trim().is_empty() {
            self.status = "summary only — press enter to fetch full article".into();
        }
    }

    fn current_item(&self) -> Option<(String, Item)> {
        self.scoped_items.get(self.list_sel).cloned()
    }

    /// Display body markdown: content only (summary lives in the header).
    fn article_markdown_display(&self, item: &Item) -> String {
        if !item.content.trim().is_empty() {
            fetch::html_to_markdown(&item.content)
        } else {
            String::new()
        }
    }

    /// Export body markdown: content, falling back to summary.
    fn article_markdown_export(&self, item: &Item) -> String {
        if !item.content.trim().is_empty() {
            fetch::html_to_markdown(&item.content)
        } else {
            item.summary.clone()
        }
    }

    fn fetch_article(&mut self) {
        let Some((_, item)) = self.current_item() else { return };
        if !item.content.trim().is_empty() {
            self.status = "feed already provides full content".into();
            return;
        }
        if item.url.is_empty() {
            self.status = "no url to fetch".into();
            return;
        }
        if self.fetching {
            return;
        }
        self.fetching = true;
        self.status = format!("fetching {}", item.url);
        let timeout = self.cfg.fetch_timeout;
        let tx = self.tx.clone();
        let url = item.url.clone();
        let guid = item.guid.clone();
        thread::spawn(move || {
            let result = fetch::fetch_article(&url, timeout);
            tx.send(Msg::ArticleFetched { url, guid, result }).ok();
        });
    }

    // ── feed/category CRUD (rewrites urls file) ────────────────────────

    fn save_urls(&mut self) {
        if let Err(e) = self.feeds.save(&self.cfg.urls_path) {
            self.status = format!("urls save failed: {e}");
        }
    }

    fn start_input(&mut self, mode: InputMode) {
        let prompt = match &mode {
            InputMode::AddUrl => "feed URL:".to_string(),
            InputMode::AddTitle => "display title (empty = none):".to_string(),
            InputMode::AddCategory => "category (space-separated, empty = none):".to_string(),
            InputMode::AddTags | InputMode::EditTags => {
                "tags (space-separated, empty = none):".to_string()
            }
            InputMode::RenameCategory => "new category name:".to_string(),
            InputMode::EditFeedTitle => "display title (empty = default):".to_string(),
            InputMode::EditTag => "new tag name:".to_string(),
            InputMode::ImportOpml => "OPML file path:".to_string(),
        };
        let mut buf = String::new();
        // prefill current tags when editing
        if let InputMode::EditTags = &mode {
            if let Some(url) = &self.edit_tags_url {
                if let Some(f) = self.feeds.feeds.iter().find(|f| &f.url == url) {
                    buf = f
                        .feed_tags
                        .iter()
                        .map(|t| format!("#{t}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                }
            }
        }
        // prefill the current custom title when editing
        if let InputMode::EditFeedTitle = &mode {
            if let Some(url) = &self.edit_tags_url {
                if let Some(f) = self.feeds.feeds.iter().find(|f| &f.url == url) {
                    if f.custom_name {
                        buf = f.title.clone().unwrap_or_default();
                    }
                }
            }
        }
        self.input = Some(InputPrompt { mode, prompt, buf });
    }

    fn submit_input(&mut self) {
        let Some(prompt) = self.input.take() else { return };
        let val = prompt.buf.trim().to_string();
        match prompt.mode {
            InputMode::AddUrl => {
                if val.is_empty() {
                    self.status = "add feed cancelled (empty url)".into();
                    return;
                }
                self.add_pending = Some(val);
                self.start_input(InputMode::AddTitle);
            }
            InputMode::AddTitle => {
                let url = self.add_pending.take().unwrap_or_default();
                if url.is_empty() {
                    return;
                }
                self.add_pending = Some(url);
                self.add_pending_title = if val.is_empty() { None } else { Some(val) };
                self.start_input(InputMode::AddCategory);
            }
            InputMode::AddCategory => {
                let url = self.add_pending.take().unwrap_or_default();
                let tags: Vec<String> = val.split_whitespace().map(str::to_string).collect();
                self.add_pending = Some(url);
                self.add_pending_category = Some(tags);
                self.start_input(InputMode::AddTags);
            }
            InputMode::AddTags => {
                let url = self.add_pending.take().unwrap_or_default();
                let title = self.add_pending_title.take();
                let tags = self.add_pending_category.take().unwrap_or_default();
                let feed_tags: Vec<String> = val
                    .split_whitespace()
                    .map(|w| w.trim_start_matches('#').to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                let feed = Feed {
                    url: url.clone(),
                    title,
                    custom_name: false,
                    tags,
                    feed_tags,
                    favourite: false,
                };
                self.feeds.upsert(feed);
                self.save_urls();
                self.rebuild_tree();
                self.status = format!("added {url}");
                self.refresh_feed_thread(url, false);
            }
            InputMode::EditTags => {
                let url = self.edit_tags_url.take().unwrap_or_default();
                if url.is_empty() {
                    return;
                }
                let feed_tags: Vec<String> = val
                    .split_whitespace()
                    .map(|w| w.trim_start_matches('#').to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                if let Some(f) = self.feeds.feeds.iter_mut().find(|f| f.url == url) {
                    f.feed_tags = feed_tags;
                }
                self.save_urls();
                self.rebuild_tree();
                self.status = format!("tags updated for {url}");
            }
            InputMode::RenameCategory => {
                if val.is_empty() {
                    self.status = "rename cancelled".into();
                    return;
                }
                if let Some(TreeRow::Category(old)) = self.tree_rows.get(self.tree_sel) {
                    let old = old.clone();
                    for f in self.feeds.feeds.iter_mut() {
                        if f.category() == Some(old.as_str()) {
                            f.tags[0] = val.clone();
                        }
                    }
                    self.save_urls();
                    self.rebuild_tree();
                    self.status = format!("category {old} → {val}");
                }
            }
            InputMode::EditFeedTitle => {
                let url = self.edit_tags_url.take().unwrap_or_default();
                if url.is_empty() {
                    return;
                }
                if let Some(f) = self.feeds.feeds.iter_mut().find(|f| f.url == url) {
                    if val.is_empty() {
                        // clear the custom title — fall back to feed-provided name
                        f.title = None;
                        f.custom_name = false;
                    } else {
                        f.title = Some(val.clone());
                        f.custom_name = true;
                    }
                }
                self.save_urls();
                self.rebuild_tree();
                self.status = format!("title updated for {url}");
            }
            InputMode::EditTag => {
                if val.is_empty() {
                    self.status = "rename cancelled".into();
                    return;
                }
                if let Some(TreeRow::Tag(old)) = self.tree_rows.get(self.tree_sel) {
                    let old = old.clone();
                    for f in self.feeds.feeds.iter_mut() {
                        if let Some(slot) = f.feed_tags.iter_mut().find(|t| **t == old) {
                            *slot = val.clone();
                        }
                    }
                    self.save_urls();
                    self.rebuild_tree();
                    self.status = format!("tag {old} → {val}");
                }
            }
            InputMode::ImportOpml => {
                if val.is_empty() {
                    self.status = "import cancelled".into();
                    return;
                }
                match std::fs::read_to_string(&val) {
                    Ok(xml) => match opml::import_opml(&xml) {
                        Ok(feeds) => {
                            let n = feeds.len();
                            for f in feeds {
                                self.feeds.upsert(f);
                            }
                            self.save_urls();
                            self.rebuild_tree();
                            self.status = format!("imported {n} feeds from {val}");
                        }
                        Err(e) => self.status = format!("OPML parse failed: {e}"),
                    },
                    Err(e) => self.status = format!("read failed: {e}"),
                }
            }
        }
    }

    fn delete_selected_feed(&mut self) {
        if let Some(TreeRow::Feed(url, name, _)) = self.tree_rows.get(self.tree_sel).cloned() {
            self.feeds.remove(&url);
            self.save_urls();
            self.rebuild_tree();
            self.status = format!("removed {name}");
            if self.scope == Scope::Feed(url) {
                self.scope = Scope::AllUnread;
                self.rebuild_list();
            }
        }
    }

    fn export_opml(&mut self) {
        let path = self.cfg.config_dir.join("feeds.opml");
        match std::fs::write(&path, opml::export_opml(&self.feeds)) {
            Ok(_) => self.status = format!("OPML exported to {}", path.display()),
            Err(e) => self.status = format!("OPML export failed: {e}"),
        }
    }

    // ── refresh ───────────────────────────────────────────────────────────

    /// `full=true` (manual `r`): replace items + rebuild the list.
    /// `full=false` (auto fetch): upsert new items only, append new unread
    /// to the current list — never removes read articles.
    fn refresh_feed_thread(&mut self, url: String, full: bool) {
        self.pending_refreshes += 1;
        let timeout = self.cfg.fetch_timeout;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = fetch::refresh_feed(&url, timeout);
            tx.send(Msg::FeedRefreshed { url, result, full }).ok();
        });
    }

    /// Feed urls belonging to the current scope (for partial refresh).
    fn scope_feeds(&self) -> Vec<String> {
        match &self.scope {
            Scope::AllUnread | Scope::ReadLater | Scope::Saved => {
                self.feeds.feeds.iter().map(|f| f.url.clone()).collect()
            }
            Scope::Category(c) => self
                .feeds
                .by_category(c)
                .iter()
                .map(|f| f.url.clone())
                .collect(),
            Scope::Feed(u) => vec![u.clone()],
            Scope::Tag(t) => self
                .feeds
                .feeds
                .iter()
                .filter(|f| f.has_tag(t))
                .map(|f| f.url.clone())
                .collect(),
        }
    }

    fn refresh_all(&mut self, full: bool) {
        if self.pending_refreshes > 0 {
            return;
        }
        // partial refresh targets only the feeds in the current list;
        // full refresh targets every feed
        let urls: Vec<String> = if full {
            self.feeds.feeds.iter().map(|f| f.url.clone()).collect()
        } else {
            self.scope_feeds()
        };
        if urls.is_empty() {
            self.status = "no feeds — add subscriptions to the urls file".into();
            return;
        }
        self.status = if full {
            format!("refreshing {} feeds…", urls.len())
        } else {
            format!("fetching new items from {} feeds…", urls.len())
        };
        for url in urls {
            self.refresh_feed_thread(url, full);
        }
    }

    fn handle_feed_refreshed(
        &mut self,
        url: String,
        result: Result<Vec<Item>, String>,
        full: bool,
    ) {
        self.pending_refreshes = self.pending_refreshes.saturating_sub(1);
        match result {
            Ok(mut items) => {
                if let Some(cap) = self.cfg.max_items_per_feed {
                    items.truncate(cap);
                }
                if full {
                    self.db.replace_feed_items_preserving_read(&url, &items).ok();
                    self.status = format!("refreshed {url} ({} items)", items.len());
                } else {
                    let added = self.db.upsert_fetch(&url, &items).unwrap_or_default();
                    self.status =
                        format!("fetched {url} ({} new)", added.len());
                    self.append_new_unread(&url, &added);
                }
            }
            Err(e) => self.status = e,
        }
        if self.pending_refreshes == 0 {
            self.status.push_str(" — done");
        }
        if full {
            self.rebuild_list();
        }
    }

    /// Append newly-fetched unread items to the current list (no reorder of
    /// existing entries).
    fn append_new_unread(&mut self, feed_url: &str, added: &[String]) {
        if added.is_empty() {
            return;
        }
        let in_scope = match &self.scope {
            Scope::AllUnread => true,
            Scope::Feed(u) => u == feed_url,
            Scope::Category(c) => self.feeds.by_category(c).iter().any(|f| f.url == feed_url),
            Scope::Tag(t) => self
                .feeds
                .feeds
                .iter()
                .any(|f| f.url == feed_url && f.has_tag(t)),
            _ => false,
        };
        if !in_scope {
            return;
        }
        let existing: std::collections::HashSet<String> = self
            .scoped_items
            .iter()
            .map(|(_, i)| i.guid.clone())
            .collect();
        let mut fresh: Vec<(String, Item)> = Vec::new();
        if let Ok(list) = self.db.items_for_feed(feed_url) {
            for i in list {
                if added.contains(&i.guid)
                    && !existing.contains(&i.guid)
                    && !self.db.is_read(feed_url, &i.guid).unwrap_or(false)
                {
                    fresh.push((feed_url.to_string(), i));
                }
            }
        }
        if !fresh.is_empty() {
            self.scoped_items.splice(0..0, fresh);
        }
    }

    fn handle_article_fetched(&mut self, url: String, guid: String, result: Result<String, String>) {
        self.fetching = false;
        match result {
            Ok(html) => {
                let (feed_url, _) = match self.current_item() {
                    Some(ci) => ci,
                    None => (String::new(), Item {
                        guid: String::new(),
                        title: String::new(),
                        url: String::new(),
                        summary: String::new(),
                        content: String::new(),
                        date: String::new(),
                        read_later: false,
                        saved: false,
                    }),
                };
                self.db.update_item_content(&feed_url, &guid, &html).ok();
                // content changed — invalidate the rendered-body cache
                self.article_render = None;
                self.status = format!("fetched {} ({} chars)", url, html.len());
            }
            Err(e) => {
                self.status = format!("fetch failed: {e}");
            }
        }
        self.rebuild_list();
    }

    // ── actions ───────────────────────────────────────────────────────────

    fn mark_all_read(&mut self) {
        // flag-scoped views: mark the scoped items read directly
        if matches!(self.scope, Scope::ReadLater | Scope::Saved | Scope::Tag(_)) {
            let items = self.scoped_items.clone();
            for (url, i) in items {
                self.db.set_read(&url, &i.guid, true).ok();
            }
            self.rebuild_list();
            self.status = "marked all read".into();
            return;
        }
        let urls: Vec<String> = match &self.scope {
            Scope::AllUnread => self.feeds.feeds.iter().map(|f| f.url.clone()).collect(),
            Scope::Category(cat) => self
                .feeds
                .by_category(cat)
                .iter()
                .map(|f| f.url.clone())
                .collect(),
            Scope::Feed(url) => vec![url.clone()],
            // unreachable: flag/tag scopes handled above
            Scope::ReadLater | Scope::Saved | Scope::Tag(_) => Vec::new(),
        };
        for u in urls {
            self.db.mark_all_read(&u).ok();
        }
        self.rebuild_list();
        self.status = "marked all read".into();
    }

    fn toggle_read(&mut self) {
        let Some((url, item)) = self.current_item() else { return };
        self.db.toggle_read(&url, &item.guid).ok();
        self.rebuild_list();
    }

    fn open_browser(&mut self) {
        let Some((_, item)) = self.current_item() else { return };
        if item.url.is_empty() {
            self.status = "no url".into();
            return;
        }
        let cmd = self.cfg.browser.clone().unwrap_or_else(|| "xdg-open".to_string());
        let _ = std::process::Command::new(&cmd)
            .arg(&item.url)
            .spawn()
            .map_err(|e| {
                self.status = format!("{cmd} failed: {e}");
            });
        self.status = format!("opened {}", item.url);
    }

    fn export_markdown(&mut self) -> io::Result<()> {
        let Some((feed_url, item)) = self.current_item() else {
            return Ok(());
        };
        let feed = self.feeds.feeds.iter().find(|f| f.url == feed_url);
        let category = feed.and_then(|f| f.category()).unwrap_or("");
        let body = self.article_markdown_export(&item);

        let mut md = String::new();
        md.push_str(&format!("---\ntitle: \"{}\"\n", escape_yaml(&item.title)));
        md.push_str(&format!("link: {}\n", item.url));
        if !item.date.is_empty() {
            md.push_str(&format!("date: {}\n", item.date));
        }
        if let Some(f) = feed {
            md.push_str(&format!("feed: \"{}\"\n", escape_yaml(f.display_name())));
        }
        md.push_str("---\n\n");
        md.push_str(&body);
        md.push('\n');

        let slug = slugify(&item.title);
        let dir = if category.is_empty() {
            self.cfg.export_dir.clone()
        } else {
            self.cfg.export_dir.join(category)
        };
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{slug}.md"));
        std::fs::write(&path, md)?;
        self.status = format!("exported {}", path.display());
        Ok(())
    }

    fn next_prev_item(&mut self, delta: isize) {
        if self.scoped_items.is_empty() {
            return;
        }
        let n = self.scoped_items.len() as isize;
        let mut idx = self.list_sel as isize + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx >= n {
            idx = n - 1;
        }
        self.select_item(idx as usize);
        // in the article pane, showing content marks it read
        if self.focus == 2 {
            if let Some((url, item)) = self.current_item() {
                self.db.set_read(&url, &item.guid, true).ok();
                if item.read_later {
                    self.db.set_flag(&url, &item.guid, "read_later", false).ok();
                }
                // no rebuild — keeps the current view stable
            }
        }
    }

    // ── keys ──────────────────────────────────────────────────────────────

    fn on_key(&mut self, key: KeyCode, mods: KeyModifiers) {
        if self.show_help {
            match key {
                KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => self.show_help = false,
                KeyCode::Char('j') | KeyCode::Down => self.help_scroll += 1,
                KeyCode::Char('k') | KeyCode::Up => {
                    self.help_scroll = self.help_scroll.saturating_sub(1)
                }
                _ => {}
            }
            return;
        }
        if let Some(prompt) = self.input.as_mut() {
            match key {
                KeyCode::Esc => self.input = None,
                KeyCode::Enter => self.submit_input(),
                KeyCode::Backspace => {
                    prompt.buf.pop();
                }
                KeyCode::Char(c) => prompt.buf.push(c),
                _ => {}
            }
            return;
        }
        // ctrl+u / ctrl+d belong to the article pane (half-page scroll)
        if mods.contains(KeyModifiers::CONTROL) {
            if self.focus == 2 {
                self.article_scroll_ctrl(key);
            }
            return;
        }
        // any key other than d disarms the delete-confirm
        if key != KeyCode::Char('d') {
            self.delete_armed = false;
        }
        match key {
            // left: h / q / esc — article→list→nav→parent in tree
            KeyCode::Char('h') | KeyCode::Char('q') | KeyCode::Esc => self.go_left(),
            // right: l / enter — expand tree→list→article→fetch full
            KeyCode::Char('l') | KeyCode::Enter => self.go_right(),
            KeyCode::Char('Q') => self.running = false,
            // F: nav → favourite feed; article → fullscreen
            KeyCode::Char('F') if self.focus == 0 => self.toggle_favourite_feed(),
            KeyCode::Char('F') if self.focus == 2 => {
                self.fullscreen = !self.fullscreen;
            }
            KeyCode::Char('f') if self.focus == 0 => self.toggle_favourite_feed(),
            KeyCode::Char('?') => self.show_help = true,
            // r: partial refresh (fetch new unread, append); R: refresh all (rebuild)
            KeyCode::Char('r') => self.refresh_all(false),
            KeyCode::Char('R') => self.refresh_all(true),
            // a: toggle read of the current item; A: mark all in view read
            KeyCode::Char('a') => self.toggle_read(),
            KeyCode::Char('A') => self.mark_all_read(),
            KeyCode::Char('e') => {
                if let Err(e) = self.export_markdown() {
                    self.status = format!("export failed: {e}");
                }
            }
            KeyCode::Char('o') => self.open_browser(),
            KeyCode::Char('t') if self.focus == 0 => self.cycle_preset(),
            // L / S: item flags from list or article pane (toggle; again to cancel)
            KeyCode::Char('L') if self.focus >= 1 => self.toggle_item_flag("read_later"),
            KeyCode::Char('S') if self.focus >= 1 => self.toggle_item_flag("saved"),
            KeyCode::Tab => self.focus = (self.focus + 1) % 3,
            KeyCode::BackTab => self.focus = (self.focus + 2) % 3,
            KeyCode::Char('N') if self.focus == 0 => self.start_input(InputMode::AddUrl),
            KeyCode::Char('d') if self.focus == 0 => {
                if self.delete_armed {
                    self.delete_armed = false;
                    self.delete_selected_feed();
                } else {
                    self.delete_armed = true;
                    let name = self
                        .tree_rows
                        .get(self.tree_sel)
                        .map(|r| match r {
                            TreeRow::Feed(_, n, _) => n.clone(),
                            _ => String::new(),
                        })
                        .unwrap_or_default();
                    self.status = format!("press d again to delete {name}");
                }
            }
            // M: rename — category / tag / feed custom title
            KeyCode::Char('M') if self.focus == 0 => match self.tree_rows.get(self.tree_sel) {
                Some(TreeRow::Category(_)) => self.start_input(InputMode::RenameCategory),
                Some(TreeRow::Tag(_)) => self.start_input(InputMode::EditTag),
                Some(TreeRow::Feed(url, _, _)) => {
                    self.edit_tags_url = Some(url.clone());
                    self.start_input(InputMode::EditFeedTitle);
                }
                _ => {}
            },
            // T: edit tags of the selected feed (nav)
            KeyCode::Char('T') if self.focus == 0 => {
                if let Some(TreeRow::Feed(url, _, _)) = self.tree_rows.get(self.tree_sel).cloned() {
                    self.edit_tags_url = Some(url);
                    self.start_input(InputMode::EditTags);
                }
            }
            KeyCode::Char('i') => self.start_input(InputMode::ImportOpml),
            KeyCode::Char('x') => self.export_opml(),
            _ => match self.focus {
                0 => self.nav_key(key),
                1 => self.list_key(key),
                2 => self.article_key(key),
                _ => {}
            },
        }
    }

    /// Toggle favourite on the selected feed row (persists to urls file).
    fn toggle_favourite_feed(&mut self) {
        let Some(TreeRow::Feed(url, _, _)) = self.tree_rows.get(self.tree_sel).cloned() else {
            return;
        };
        let new_state = {
            let Some(f) = self.feeds.feeds.iter_mut().find(|f| f.url == url) else {
                return;
            };
            f.favourite = !f.favourite;
            f.favourite
        };
        self.save_urls();
        self.rebuild_tree();
        self.status = if new_state { "favourited".into() } else { "unfavourited".into() };
    }

    /// Toggle read_later / saved flag on the current item.
    fn toggle_item_flag(&mut self, flag: &str) {
        let Some((url, item)) = self.current_item() else { return };
        let on = self.db.toggle_flag(&url, &item.guid, flag).unwrap_or(false);
        if flag == "read_later" && on {
            // marking read-later also marks unread
            self.db.set_read(&url, &item.guid, false).ok();
        }
        self.rebuild_list();
    }

    /// Left: article→list→nav→parent in file tree.
    fn go_left(&mut self) {
        match self.focus {
            2 => {
                if self.fullscreen {
                    self.fullscreen = false;
                }
                self.focus = 1;
            }
            1 => self.focus = 0,
            _ => self.nav_left(),
        }
    }

    /// Right: expand tree→list→article→fetch full.
    fn go_right(&mut self) {
        match self.focus {
            0 => self.nav_right(),
            1 => self.open_item(),
            _ => self.fetch_article(),
        }
    }

    /// Nav left: expanded node → fold; folded node → fold its parent;
    /// feed → fold its containing container; top-level folded → stay.
    fn nav_left(&mut self) {
        let Some(row) = self.tree_rows.get(self.tree_sel).cloned() else {
            return;
        };
        match row {
            TreeRow::Section(name) => {
                // top-level: expanded → fold; folded → stay
                if !self.collapsed.contains(&name) {
                    self.collapsed.insert(name);
                    self.rebuild_tree();
                }
            }
            TreeRow::Favourite => {
                if self.fav_expanded {
                    self.fav_expanded = false;
                    self.rebuild_tree();
                }
            }
            TreeRow::Uncategorized => {
                if self.uncat_expanded {
                    self.uncat_expanded = false;
                    self.rebuild_tree();
                }
            }
            TreeRow::Category(cat) => {
                if !self.collapsed.contains(&cat) {
                    self.collapsed.insert(cat);
                    self.rebuild_tree();
                } else {
                    // fold the Categories section (parent)
                    self.collapsed.insert("Categories".to_string());
                    self.rebuild_tree();
                    if let Some(idx) = self
                        .tree_rows
                        .iter()
                        .position(|r| matches!(r, TreeRow::Section(n) if n == "Categories"))
                    {
                        self.tree_sel = idx;
                    }
                }
            }
            TreeRow::Tag(t) => {
                let key = format!("tag:{t}");
                if !self.collapsed.contains(&key) {
                    self.collapsed.insert(key);
                    self.rebuild_tree();
                } else {
                    // fold the Tags section (parent)
                    self.collapsed.insert("Tags".to_string());
                    self.rebuild_tree();
                    if let Some(idx) = self
                        .tree_rows
                        .iter()
                        .position(|r| matches!(r, TreeRow::Section(n) if n == "Tags"))
                    {
                        self.tree_sel = idx;
                    }
                }
            }
            TreeRow::Feed(_, _, _)
            | TreeRow::FavouriteFeed(_, _)
            | TreeRow::UncategorizedFeed(_, _) => {
                // fold the nearest container above this row
                for j in (0..self.tree_sel).rev() {
                    match &self.tree_rows[j] {
                        TreeRow::Category(c) => {
                            self.collapsed.insert(c.clone());
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        TreeRow::Tag(t) => {
                            self.collapsed.insert(format!("tag:{t}"));
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        TreeRow::Favourite => {
                            self.fav_expanded = false;
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        TreeRow::Uncategorized => {
                            self.uncat_expanded = false;
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        TreeRow::Section(n) => {
                            self.collapsed.insert(n.clone());
                            self.rebuild_tree();
                            self.tree_sel = j;
                            return;
                        }
                        _ => {}
                    }
                }
            }
            // Unread / Read Later / Saved: no fold, stay
            _ => {}
        }
    }

    /// Nav right: collapsed section/category/favourite → expand; else descend.
    fn nav_right(&mut self) {
        match self.tree_rows.get(self.tree_sel).cloned() {
            Some(TreeRow::Section(name)) if self.collapsed.contains(&name) => {
                self.collapsed.remove(&name);
                self.rebuild_tree();
                self.step_into_expanded();
            }
            Some(TreeRow::Section(_)) => {}
            Some(TreeRow::Category(cat)) if self.collapsed.contains(&cat) => {
                self.collapsed.remove(&cat);
                self.rebuild_tree();
                self.step_into_expanded();
            }
            Some(TreeRow::Favourite) if !self.fav_expanded => {
                self.fav_expanded = true;
                self.rebuild_tree();
                self.step_into_expanded();
            }
            Some(TreeRow::Uncategorized) if !self.uncat_expanded => {
                self.uncat_expanded = true;
                self.rebuild_tree();
                self.step_into_expanded();
            }
            Some(TreeRow::Tag(t)) if self.collapsed.contains(&format!("tag:{t}")) => {
                self.collapsed.remove(&format!("tag:{t}"));
                self.rebuild_tree();
                self.step_into_expanded();
            }
            Some(row) => self.select_scope(&row),
            None => {}
        }
    }

    /// After expanding a fold, move the cursor to its first child row.
    fn step_into_expanded(&mut self) {
        if self.tree_sel + 1 < self.tree_rows.len() {
            self.tree_sel += 1;
        }
    }

    fn nav_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.tree_sel + 1 < self.tree_rows.len() {
                    self.tree_sel += 1;
                    self.preview_scope();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.tree_sel = self.tree_sel.saturating_sub(1);
                self.preview_scope();
            }
            _ => {}
        }
    }

    fn list_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.list_sel + 1 < self.scoped_items.len() {
                    self.list_sel += 1;
                    self.article_scroll = 0;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.list_sel = self.list_sel.saturating_sub(1);
                self.article_scroll = 0;
            }
            _ => {}
        }
    }

    fn article_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => self.article_scroll += 1,
            KeyCode::Char('k') | KeyCode::Up => self.article_scroll = self.article_scroll.saturating_sub(1),
            KeyCode::Char('n') | KeyCode::PageDown => self.next_prev_item(1),
            KeyCode::Char('p') | KeyCode::PageUp => self.next_prev_item(-1),
            _ => {}
        }
    }

    fn article_scroll_ctrl(&mut self, key: KeyCode) {
        let half = (self.article_area.height.saturating_sub(4) / 2).max(1);
        match key {
            KeyCode::Char('u') => self.article_scroll = self.article_scroll.saturating_sub(half),
            KeyCode::Char('d') => self.article_scroll += half,
            _ => {}
        }
    }
}

// ─── helpers ────────────────────────────────────────────────────────────────

fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn slugify(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if c.is_whitespace() || c == '-' || c == '_' {
            out.push('-');
        }
    }
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let out = out.trim_matches('-');
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out.to_string()
    }
}

fn fmt_date(iso: &str) -> String {
    iso.chars().take(10).collect()
}

// ─── rendering ──────────────────────────────────────────────────────────────

fn render(frame: &mut Frame, app: &mut App) {
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
    let r = app.cfg.pane_ratio;
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
                let n = app.feeds.feeds.iter().filter(|f| f.favourite).count();
                let prefix = if app.fav_expanded { "▾" } else { "▸" };
                (format!("{prefix} Favourite ({n})"), Style::default().add_modifier(Modifier::BOLD))
            }
            TreeRow::Uncategorized => {
                let n = app.feeds.uncategorized().len();
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
                (format!("  {name} ({n})"), Style::default())
            }
            TreeRow::Category(cat) => {
                let n: usize = app
                    .feeds
                    .by_category(cat)
                    .iter()
                    .map(|f| app.db.unread_count(&f.url).unwrap_or(0))
                    .sum();
                let prefix = if app.collapsed.contains(cat) { "▸" } else { "▾" };
                (format!("  {prefix} {cat} ({n})"), Style::default())
            }
            TreeRow::Feed(url, name, indent) => {
                let n = app.db.unread_count(url).unwrap_or(0);
                (format!("{}{} ({n})", " ".repeat(*indent as usize), name), Style::default())
            }
            TreeRow::Tag(t) => {
                let n = app
                    .feeds
                    .feeds
                    .iter()
                    .filter(|f| f.has_tag(t))
                    .count();
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


/// Build the styled article body Text (HTML → markdown → styled spans).
fn render_article_text(app: &App, item: &Item) -> ratatui::text::Text<'static> {
    let md = app.article_markdown_display(item);
    if md.trim().is_empty() {
        return Text::from(""); // caller falls back to the fetch hint
    }
    let link_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED);
    let img_style = Style::default().fg(Color::DarkGray);
    let renderer = the_other_tui_markdown::RendererBuilder::new()
        .with_link(move |alt, _url| {
            vec![Span::styled(alt.to_string(), link_style)]
        })
        .with_image(move |_alt, _url| {
            vec![Span::styled("[img]", img_style)]
        })
        .build();
    the_other_tui_markdown::into_text_with_renderer(&md, &renderer)
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

    let content_ready = !item.content.trim().is_empty() || !item.summary.trim().is_empty();
    let fetching_hint = if app.fetching { " (fetching…)" } else { "" };

    // Fixed header (title/meta/summary), content scrolls below a separator.
    let block = pane_block("Article", app.focus == 2);
    let inner = block.inner(area);
    let [head, sep, body] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(inner);

    let title_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let meta_style = Style::default().fg(Color::Cyan);
    let dim_style = Style::default().fg(Color::DarkGray);
    let read_mark = if app.db.is_read(&url, &item.guid).unwrap_or(false) {
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

    let header_text = Text::from(vec![
        Line::from(Span::styled(item.display_title(), title_style)),
        Line::from(Span::styled(
            format!("{feed_name}  ·  {}  ·  {read_mark}{flags_mark}", fmt_date(&item.date)),
            meta_style,
        )),
        Line::from(Span::styled(item.url.clone(), dim_style)),
        Line::from(""),
        Line::from(Span::styled(item.summary.trim(), Style::default())),
    ]);
    frame.render_widget(Paragraph::new(header_text).wrap(Wrap { trim: true }), head);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(sep.width as usize),
            Style::default().fg(Color::DarkGray),
        )),
        sep,
    );

    // Body: feed/fetched HTML → markdown → styled ratatui Text
    // (eilmeldung-style pipeline: html2md + the_other_tui_markdown).
    // Links render as underlined alt text (no URL); images as [img].
    let body_text = if content_ready {
        // reuse the rendered body when the item hasn't changed
        if let Some((g, t)) = &app.article_render {
            if g == &item.guid {
                t.clone()
            } else {
                let t = render_article_text(app, &item);
                app.article_render = Some((item.guid.clone(), t.clone()));
                t
            }
        } else {
            let t = render_article_text(app, &item);
            app.article_render = Some((item.guid.clone(), t.clone()));
            t
        }
    } else {
        Text::from(format!("[summary only — press enter to fetch full article{fetching_hint}]"))
    };
    let para = Paragraph::new(body_text)
        .wrap(Wrap { trim: true })
        .scroll((app.article_scroll, 0));
    frame.render_widget(para, body);

    frame.render_widget(block, area);
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let line = format!(
        "{}  |  ? help  q quit  tab focus  j/k move  enter open  o browser  e export  u read  A all-read  r refresh",
        app.status
    );
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_help(frame: &mut Frame, area: Rect, app: &App) {
    let text = Text::from(
        "Keys\n\
         ─────\n\
         nav:   j/k move · h/l expand/collapse+descend · N add feed · d delete · M rename (category/tag/feed title) · F favourite\n\
         list:  j/k move · l/enter open (mark read)\n\
         article: j/k scroll · n/p item · ctrl+u/ctrl+d half page · l/enter fetch full\n\
         left:  h/q/esc — article→list→nav→parent\n\
         right: l/enter — expand→list→article→fetch\n\
         global: o browser · e export · a toggle read · A mark all read\n\
         r partial refresh · R refresh all · i import OPML · x export OPML · tab focus · Q quit · ? help\n\n\
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

// ─── main loop ──────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    let cfg = Config::load();
    let mut app = App::new(cfg);
    app.db.cleanup_content(app.cfg.cache_ttl_days).ok();
    // startup: fetch new items (append-only) — never a full refresh
    if app.cfg.refresh_on_startup {
        app.refresh_all(false);
    }
    // interval auto-refresh
    if let Some(interval_min) = app.cfg.refresh_interval_minutes {
        if interval_min > 0 {
            let tx = app.tx.clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_secs(interval_min * 60));
                tx.send(Msg::RefreshTick).ok();
            });
        }
    }

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    while app.running {
        terminal.draw(|f| render(f, app))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    app.on_key(k.code, k.modifiers);
                }
            }
        }
        while let Ok(msg) = app.rx.try_recv() {
            match msg {
                Msg::FeedRefreshed { url, result, full } => app.handle_feed_refreshed(url, result, full),
                Msg::ArticleFetched { url, guid, result } => {
                    app.handle_article_fetched(url, guid, result)
                }
                Msg::RefreshTick => app.refresh_all(false),
            }
        }
    }
    Ok(())
}
