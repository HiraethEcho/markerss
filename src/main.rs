//! markerss — TUI RSS reader.
//!
//! Three panes: nav tree (categories → feeds) | item list | article.
//! Storage: SQLite (items + read state). Content: feed HTML → markdown →
//! styled Text (eilmeldung-style pipeline). Design authority: DESIGN.md.

mod clipboard;
mod config;
mod db;
mod feedlist;
mod fetch;
mod keys;
mod model;
mod opml;
mod ui;
mod util;
mod xdg;

use crate::ui::render;
use crate::util::{escape_yaml, slugify};
use ratatui::layout::Rect;

use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use model::Item;

use crate::config::{Config, ThemeColors};
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
    ExportFile,
    ImportOpml,
    Search,
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
    theme: ThemeColors,
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
    // rendered article body, keyed by item guid (avoid per-frame conversion)
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
    pending_keys: Vec<KeyCode>,
    sort_stack: Vec<(String, bool)>,
    search_base: Option<Vec<(String, Item)>>,
    search_active: bool,
    search_query: String,
    keymap: std::collections::HashMap<Vec<KeyCode>, crate::keys::Action>,
    feed_errors: std::collections::HashMap<String, String>,
    input: Option<InputPrompt>,
    add_pending: Option<String>,
    add_pending_title: Option<String>,
    add_pending_category: Option<Vec<String>>,
    edit_tags_url: Option<String>,
    export_pending: Option<(String, String)>,
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
        let theme = ThemeColors::load(cfg.theme_path.as_ref());
        let sort_stack: Vec<(String, bool)> =
            cfg.sort.iter().map(|s| (s.clone(), false)).collect();
        let keymap = crate::keys::build_keymap(&cfg.keybindings);
        let mut app = App {
            cfg,
            theme,
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
            pending_keys: Vec::new(),
            sort_stack,
            search_base: None,
            search_active: false,
            search_query: String::new(),
            keymap,
            feed_errors: std::collections::HashMap::new(),
            input: None,
            add_pending: None,
            add_pending_title: None,
            add_pending_category: None,
            edit_tags_url: None,
            export_pending: None,
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
                    for path in self.feeds.categories_tree() {
                        let joined = path.join("/");
                        let depth = path.len();
                        // hidden when any ancestor is collapsed
                        let mut ancestor_folded = false;
                        for i in 1..depth {
                            if self.collapsed.contains(&path[..i].join("/")) {
                                ancestor_folded = true;
                                break;
                            }
                        }
                        if ancestor_folded {
                            continue;
                        }
                        rows.push(TreeRow::Category(joined.clone()));
                        if !self.collapsed.contains(&joined) {
                            for f in self.feeds.by_category_path(&path) {
                                rows.push(TreeRow::Feed(
                                    f.url.clone(),
                                    f.display_name().to_string(),
                                    (depth * 2 + 2) as u8,
                                ));
                            }
                        }
                    }
                    // uncategorized feeds as their own foldable top node
                    // (hidden when every feed has a category)
                    if !self.feeds.uncategorized().is_empty() {
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

    /// Drop any active search when the scope changes (its base snapshot is stale).
    fn clear_search(&mut self) {
        self.search_base = None;
        self.search_active = false;
        self.search_query.clear();
        if let Some(p) = &self.input {
            if p.mode == InputMode::Search {
                self.input = None;
            }
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
        self.clear_search();
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
        // explicit sort stack (st/sn/sf/su) re-orders the snapshot on demand
        self.apply_sort();
        self.reapply_search_filter();
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
        if item.url.is_empty() {
            self.status = "no url to fetch".into();
            return;
        }
        if self.fetching {
            return;
        }
        // always try to fetch full content — even if the feed provided some
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
            InputMode::Search => "/ search:".to_string(),
            InputMode::EditFeedTitle => "display title (empty = default):".to_string(),
            InputMode::EditTag => "new tag name:".to_string(),
            InputMode::ExportFile => "export as (enter = default):".to_string(),
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
            InputMode::Search => {} // Enter is intercepted before submit
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
                    self.feeds.rename_category(&old, &val);
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
            InputMode::ExportFile => {
                let path = if val.is_empty() {
                    // fall back to the prefilled default
                    prompt.buf.trim().to_string()
                } else {
                    val
                };
                self.finish_export(std::path::PathBuf::from(path));
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
                self.feed_errors.remove(&url);
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
                    // in-place refresh of snapshot entries (content etc.) —
                    // no rebuild, so read items stay in place
                    self.refresh_snapshot_content(&url);
                }
            }
            Err(e) => {
                self.status = e.clone();
                self.feed_errors.insert(url.clone(), e);
            }
        }
        if self.pending_refreshes == 0 {
            self.status.push_str(" — done");
        }
        if full {
            self.rebuild_list();
        }
    }

    /// Update the in-memory list snapshot for one feed from the DB
    /// (content/summary/title) without rebuilding or reordering.
    fn refresh_snapshot_content(&mut self, feed_url: &str) {
        let Ok(list) = self.db.items_for_feed(feed_url) else {
            return;
        };
        let by_guid: std::collections::HashMap<String, Item> =
            list.into_iter().map(|i| (i.guid.clone(), i)).collect();
        for (u, i) in self.scoped_items.iter_mut() {
            if u == feed_url {
                if let Some(fresh) = by_guid.get(&i.guid) {
                    i.content = fresh.content.clone();
                    i.summary = fresh.summary.clone();
                    i.title = fresh.title.clone();
                }
            }
        }
        // the article body cache may reference the old content
        self.article_render = None;
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
        self.reapply_search_filter();
    }

    fn handle_article_fetched(&mut self, url: String, guid: String, result: Result<String, String>) {
        self.fetching = false;
        match result {
            Ok(html) => {
                let Some((feed_url, _)) = self.current_item() else {
                    self.status = format!("fetched {} (stale view)", url);
                    return;
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
        self.open_url(&item.url);
    }

    /// Open an arbitrary URL in the configured browser (fallback xdg-open).
    fn open_url(&mut self, url: &str) {
        let cmd = self.cfg.browser.clone().unwrap_or_else(|| "xdg-open".to_string());
        let _ = std::process::Command::new(&cmd)
            .arg(url)
            .spawn()
            .map_err(|e| {
                self.status = format!("{cmd} failed: {e}");
            });
        self.status = format!("opened {url}");
    }

    /// Start the export flow: prompt with the default filename as placeholder.
    fn start_export(&mut self) {
        let Some((feed_url, item)) = self.current_item() else {
            self.status = "no item selected".into();
            return;
        };
        let feed = self.feeds.feeds.iter().find(|f| f.url == feed_url);
        let category = feed.and_then(|f| f.category()).unwrap_or("");
        let slug = slugify(&item.title);
        let dir = if category.is_empty() {
            self.cfg.export_dir.clone()
        } else {
            self.cfg.export_dir.join(category)
        };
        let default_path = dir.join(format!("{slug}.md"));
        self.export_pending = Some((feed_url, item.guid));
        self.input = Some(InputPrompt {
            mode: InputMode::ExportFile,
            prompt: "export as (enter = default):".to_string(),
            buf: default_path.to_string_lossy().to_string(),
        });
    }

    /// Finish the export after the rename prompt (or default path).
    fn finish_export(&mut self, path: std::path::PathBuf) {
        let Some((feed_url, guid)) = self.export_pending.take() else {
            return;
        };
        let Some(item) = self
            .db
            .items_for_feed(&feed_url)
            .ok()
            .and_then(|list| list.into_iter().find(|i| i.guid == guid))
        else {
            self.status = "export failed: item not found".into();
            return;
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        match self.write_export(&path, &feed_url, &item) {
            Ok(_) => self.status = format!("exported {}", path.display()),
            Err(e) => self.status = format!("export failed: {e}"),
        }
    }

    fn write_export(&self, path: &std::path::Path, feed_url: &str, item: &Item) -> io::Result<()> {
        let feed = self.feeds.feeds.iter().find(|f| f.url == feed_url);
        let body = self.article_markdown_export(item);
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
        std::fs::write(path, md)
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
}

// ── main ──────────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    let cfg = Config::load();
    let mut app = App::new(cfg);
    // foldlevel: initial nav fold depth (0 = only top rows visible)
    if let Some(level) = app.cfg.foldlevel {
        for path in app.feeds.categories_tree() {
            if path.len() > level {
                app.collapsed.insert(path.join("/"));
            }
        }
    }
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
    let mut redraw = true;
    while app.running {
        // draw only when something changed (key, message, or first frame) —
        // avoids the per-tick full repaint that made scrolling lag
        if redraw {
            terminal.draw(|f| render(f, app))?;
            redraw = false;
        }
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    app.on_key(k.code, k.modifiers);
                    redraw = true;
                }
            }
            // swallow resize/other events, still redraw
            while event::poll(Duration::from_millis(0))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind == KeyEventKind::Press {
                        app.on_key(k.code, k.modifiers);
                        redraw = true;
                    }
                }
            }
        }
        while let Ok(msg) = app.rx.try_recv() {
            redraw = true;
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

