//! Key dispatch (on_key) + per-key actions.
//! Child module of main — impl App blocks may touch private fields.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::clipboard::copy_to_clipboard;
use crate::{App, InputMode, TreeRow};

/// Keybinding actions — user-remappable single-key actions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Action {
    Open,
    Back,
    Quit,
    Refresh,
    RefreshAll,
    ToggleRead,
    MarkAllRead,
    Export,
    Browser,
    Favourite,
    ReadLater,
    Saved,
    NewFeed,
    Delete,
    Rename,
    EditTags,
    Help,
    FocusNext,
    Search,
    JumpTop,
    JumpBottom,
    NextUnread,
    PrevUnread,
}

impl Action {
    /// Parse a config action name → Action.
    pub(crate) fn from_str(s: &str) -> Option<Action> {
        Some(match s.trim() {
            "open" => Action::Open,
            "back" => Action::Back,
            "quit" => Action::Quit,
            "refresh" => Action::Refresh,
            "refresh_all" => Action::RefreshAll,
            "toggle_read" => Action::ToggleRead,
            "mark_all_read" => Action::MarkAllRead,
            "export" => Action::Export,
            "browser" => Action::Browser,
            "favourite" => Action::Favourite,
            "read_later" => Action::ReadLater,
            "saved" => Action::Saved,
            "new_feed" => Action::NewFeed,
            "delete" => Action::Delete,
            "rename" => Action::Rename,
            "edit_tags" => Action::EditTags,
            "help" => Action::Help,
            "focus_next" => Action::FocusNext,
            "search" => Action::Search,
            "jump_top" => Action::JumpTop,
            "jump_bottom" => Action::JumpBottom,
            "next_unread" => Action::NextUnread,
            "prev_unread" => Action::PrevUnread,
            _ => return None,
        })
    }

    /// Parse a key string → KeyCode ("l", "enter", "esc", "tab", "?", …).
    pub(crate) fn parse_key(s: &str) -> Option<KeyCode> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "enter" => KeyCode::Enter,
            "esc" => KeyCode::Esc,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "space" => KeyCode::Char(' '),
            other if other.chars().count() == 1 => {
                KeyCode::Char(other.chars().next().unwrap())
            }
            _ => return None,
        })
    }
}

/// Build the user keymap: key → action (invalid entries skipped).
pub(crate) fn build_keymap(
    raw: &std::collections::HashMap<String, String>,
) -> std::collections::HashMap<KeyCode, Action> {
    let mut map = std::collections::HashMap::new();
    for (action, key) in raw {
        if let (Some(a), Some(k)) = (Action::from_str(action), Action::parse_key(key)) {
            map.insert(k, a);
        }
    }
    map
}

impl App {
    /// Reset prefix-key and delete-armed state (help/input/ctrl paths return early).
    fn clear_pending(&mut self) {
        self.pending_g = false;
        self.pending_s = false;
        self.pending_y = false;
        self.delete_armed = false;
    }

    /// Execute a remapped action (user keybindings). Guards per pane.
    fn execute_action(&mut self, action: Action) {
        match action {
            Action::Open => self.go_right(),
            Action::Back => self.go_left(),
            Action::Quit => self.running = false,
            Action::Refresh => self.refresh_all(false),
            Action::RefreshAll => self.refresh_all(true),
            Action::ToggleRead => self.toggle_read(),
            Action::MarkAllRead => self.mark_all_read(),
            Action::Export => self.start_export(),
            Action::Browser => self.open_browser(),
            Action::Favourite if self.focus == 0 => self.toggle_favourite_feed(),
            Action::ReadLater if self.focus >= 1 => self.toggle_item_flag("read_later"),
            Action::Saved if self.focus >= 1 => self.toggle_item_flag("saved"),
            Action::NewFeed if self.focus == 0 => self.start_input(InputMode::AddUrl),
            Action::Delete if self.focus == 0 => {
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
            Action::Rename if self.focus == 0 => {
                match self.tree_rows.get(self.tree_sel) {
                    Some(TreeRow::Category(_)) => self.start_input(InputMode::RenameCategory),
                    Some(TreeRow::Tag(_)) => self.start_input(InputMode::EditTag),
                    Some(TreeRow::Feed(url, _, _)) => {
                        self.edit_tags_url = Some(url.clone());
                        self.start_input(InputMode::EditFeedTitle);
                    }
                    _ => {}
                }
            }
            Action::EditTags if self.focus == 0 => {
                if let Some(TreeRow::Feed(url, _, _)) = self.tree_rows.get(self.tree_sel).cloned() {
                    self.edit_tags_url = Some(url);
                    self.start_input(InputMode::EditTags);
                }
            }
            Action::Help => self.show_help = true,
            Action::FocusNext => self.focus = (self.focus + 1) % 3,
            Action::Search if self.focus == 1 => {
                self.search_base = Some(self.scoped_items.clone());
                self.search_active = true;
                self.search_query.clear();
                self.start_input(InputMode::Search);
            }
            Action::JumpTop => match self.focus {
                0 => self.tree_sel = 0,
                1 => {
                    self.list_sel = 0;
                    self.article_scroll = 0;
                }
                2 => self.article_scroll = 0,
                _ => {}
            },
            Action::JumpBottom => match self.focus {
                0 => self.tree_sel = self.tree_rows.len().saturating_sub(1),
                1 => {
                    self.list_sel = self.scoped_items.len().saturating_sub(1);
                    self.article_scroll = 0;
                }
                2 => self.article_scroll = u16::MAX,
                _ => {}
            },
            Action::NextUnread => self.mark_read_and_jump(1),
            Action::PrevUnread => self.mark_read_and_jump(-1),
            _ => {}
        }
    }

    pub(crate) fn on_key(&mut self, key: KeyCode, mods: KeyModifiers) {
        if self.show_help {
            match key {
                KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => self.show_help = false,
                KeyCode::Char('j') | KeyCode::Down => self.help_scroll += 1,
                KeyCode::Char('k') | KeyCode::Up => {
                    self.help_scroll = self.help_scroll.saturating_sub(1)
                }
                _ => {}
            }
            self.clear_pending();
            return;
        }
        if let Some(mut prompt) = self.input.take() {
            let search_mode = prompt.mode == InputMode::Search;
            match key {
                KeyCode::Esc => {
                    if search_mode {
                        self.cancel_search();
                    }
                }
                KeyCode::Enter if search_mode => {
                    // keep the filter active (left stops it); keep the base
                    // snapshot so left can restore the full list
                }
                KeyCode::Enter => {
                    self.input = Some(prompt);
                    self.submit_input();
                    return;
                }
                KeyCode::Backspace => {
                    prompt.buf.pop();
                    if search_mode {
                        self.apply_search_filter(&prompt.buf);
                    }
                    self.input = Some(prompt);
                }
                KeyCode::Char(c) => {
                    prompt.buf.push(c);
                    if search_mode {
                        self.apply_search_filter(&prompt.buf);
                    }
                    self.input = Some(prompt);
                }
                _ => {
                    self.input = Some(prompt);
                }
            }
            self.clear_pending();
            return;
        }
        // user keybindings: remapped key → action (defaults still work);
        // never intercept ctrl chords (ctrl+u/d/f/b keep their meaning)
        if !mods.contains(KeyModifiers::CONTROL) {
            if let Some(&action) = self.keymap.get(&key) {
                self.clear_pending();
                self.execute_action(action);
                return;
            }
        }
        // ctrl+u / ctrl+d half-page (article); ctrl+f/b full page (list+article)
        if mods.contains(KeyModifiers::CONTROL) {
            match key {
                KeyCode::Char('f') | KeyCode::Char('b') if self.focus >= 1 => {
                    let dir = if key == KeyCode::Char('f') { 1 } else { -1 };
                    self.page_scroll(dir);
                }
                _ if self.focus == 2 => self.article_scroll_ctrl(key),
                _ => {}
            }
            self.clear_pending();
            return;
        }
        if key != KeyCode::Char('d') {
            self.delete_armed = false;
        }
        match key {
            // left: h / q / esc — article→list→nav→parent in tree
            KeyCode::Char('h') | KeyCode::Char('q') | KeyCode::Esc => self.go_left(),
            // right: l / enter — expand tree→list→article→fetch full
            KeyCode::Char('l') | KeyCode::Enter => self.go_right(),
            KeyCode::Char('Q') => self.execute_action(Action::Quit),
            // y-prefix: yy item url, yn item title, yf feed url
            KeyCode::Char('y') => {
                if self.pending_y {
                    self.pending_y = false;
                    if let Some((_, item)) = self.current_item() {
                        copy_to_clipboard(&item.url);
                        self.status = "copied item url".into();
                    }
                } else {
                    self.pending_y = true;
                    self.status = "copy: yy item url · yn item title · yf feed url".into();
                }
            }
            KeyCode::Char('n') if self.pending_y => {
                self.pending_y = false;
                if let Some((_, item)) = self.current_item() {
                    copy_to_clipboard(item.display_title());
                    self.status = "copied item title".into();
                }
            }
            KeyCode::Char('f') if self.pending_y => {
                self.pending_y = false;
                let url = match self.focus {
                    0 => self.tree_rows.get(self.tree_sel).and_then(|r| match r {
                        TreeRow::Feed(u, _, _) | TreeRow::FavouriteFeed(u, _) | TreeRow::UncategorizedFeed(u, _) => Some(u.clone()),
                        _ => None,
                    }),
                    _ => self.current_item().map(|(u, _)| u),
                };
                if let Some(u) = url {
                    copy_to_clipboard(&u);
                    self.status = "copied feed url".into();
                }
            }
            KeyCode::Char('r') => self.execute_action(Action::Refresh),
            KeyCode::Char('R') => self.execute_action(Action::RefreshAll),
            KeyCode::Char('a') => self.execute_action(Action::ToggleRead),
            KeyCode::Char('A') => self.execute_action(Action::MarkAllRead),
            KeyCode::Char('e') => self.execute_action(Action::Export),
            KeyCode::Char('o') => self.execute_action(Action::Browser),
            KeyCode::Char('t') if self.focus == 0 => self.cycle_preset(),
            KeyCode::Char('L') if self.focus >= 1 => self.execute_action(Action::ReadLater),
            KeyCode::Char('S') if self.focus >= 1 => self.execute_action(Action::Saved),
            // gg / G: jump top / bottom (nav + list + article)
            KeyCode::Char('g') => {
                if self.pending_g {
                    self.pending_g = false;
                    match self.focus {
                        0 => self.tree_sel = 0,
                        1 => {
                            self.list_sel = 0;
                            self.article_scroll = 0;
                        }
                        2 => self.article_scroll = 0,
                        _ => {}
                    }
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('G') => match self.focus {
                0 => self.tree_sel = self.tree_rows.len().saturating_sub(1),
                1 => {
                    self.list_sel = self.scoped_items.len().saturating_sub(1);
                    self.article_scroll = 0;
                }
                2 => self.article_scroll = u16::MAX, // clamped at render
                _ => {}
            },
            // F: nav → favourite feed; article → fullscreen
            KeyCode::Char('F') if self.focus == 0 => self.execute_action(Action::Favourite),
            KeyCode::Char('F') if self.focus == 2 => {
                self.fullscreen = !self.fullscreen;
            }
            KeyCode::Char('f') if self.focus == 0 => self.execute_action(Action::Favourite),
            KeyCode::Char('?') => self.execute_action(Action::Help),
            // / — modal list search (live filter; enter keeps, esc restores)
            KeyCode::Char('/') if self.focus == 1 => self.execute_action(Action::Search),
            // s-prefix: sort levels — lowercase = forward, uppercase = reverse
            // st/sn/sf/su: time/title/feed/unread · sT/sN/sF/sU: reversed
            KeyCode::Char('s') if self.focus == 1 => {
                self.pending_s = !self.pending_s;
                if self.pending_s {
                    self.status =
                        "sort: st/sn/sf/su (lowercase) or sT/sN/sF/sU (reversed) — time/title/feed/unread".into();
                }
            }
            KeyCode::Char('t') if self.pending_s && self.focus == 1 => {
                self.pending_s = false;
                self.push_sort("time", false);
            }
            KeyCode::Char('T') if self.pending_s && self.focus == 1 => {
                self.pending_s = false;
                self.push_sort("time", true);
            }
            KeyCode::Char('n') if self.pending_s && self.focus == 1 => {
                self.pending_s = false;
                self.push_sort("title", false);
            }
            KeyCode::Char('N') if self.pending_s && self.focus == 1 => {
                self.pending_s = false;
                self.push_sort("title", true);
            }
            KeyCode::Char('f') if self.pending_s && self.focus == 1 => {
                self.pending_s = false;
                self.push_sort("feed", false);
            }
            KeyCode::Char('F') if self.pending_s && self.focus == 1 => {
                self.pending_s = false;
                self.push_sort("feed", true);
            }
            KeyCode::Char('u') if self.pending_s && self.focus == 1 => {
                self.pending_s = false;
                self.push_sort("unread", false);
            }
            KeyCode::Char('U') if self.pending_s && self.focus == 1 => {
                self.pending_s = false;
                self.push_sort("unread", true);
            }
            KeyCode::Tab => self.execute_action(Action::FocusNext),
            KeyCode::BackTab => self.focus = (self.focus + 2) % 3,
            KeyCode::Char('N') if self.focus == 0 => self.execute_action(Action::NewFeed),
            KeyCode::Char('d') if self.focus == 0 => self.execute_action(Action::Delete),
            // M: rename — category / tag / feed custom title
            KeyCode::Char('M') if self.focus == 0 => self.execute_action(Action::Rename),
            // T: edit tags of the selected feed (nav)
            KeyCode::Char('T') if self.focus == 0 => self.execute_action(Action::EditTags),
            KeyCode::Char('i') => self.start_input(InputMode::ImportOpml),
            KeyCode::Char('x') => self.export_opml(),
            _ => match self.focus {
                0 => self.nav_key(key),
                1 => self.list_key(key),
                2 => self.article_key(key),
                _ => {}
            },
        }
        // post-match: disarm pending prefixes (combos already consumed+cleared)
        match key {
            KeyCode::Char('g') | KeyCode::Char('s') | KeyCode::Char('y') => {}
            _ => {
                self.pending_g = false;
                self.pending_s = false;
                self.pending_y = false;
            }
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
        // an active search is stopped by left; list stays until next left
        if self.focus == 1 && self.search_active {
            self.cancel_search();
            return;
        }
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
                    self.collapsed.insert(cat.clone());
                    self.rebuild_tree();
                } else if let Some(parent) = cat.rfind('/').map(|i| cat[..i].to_string()) {
                    // already folded — fold the parent category instead
                    self.collapsed.insert(parent.clone());
                    self.rebuild_tree();
                    if let Some(idx) = self
                        .tree_rows
                        .iter()
                        .position(|r| matches!(r, TreeRow::Category(c) if c == &parent))
                    {
                        self.tree_sel = idx;
                    }
                } else {
                    // top-level folded category — fold the Categories section
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
            // n/p: mark current read, jump to next/prev unread
            KeyCode::Char('n') => self.mark_read_and_jump(1),
            KeyCode::Char('p') => self.mark_read_and_jump(-1),
            _ => {}
        }
    }

    /// Mark the current item read, then select the next/previous unread
    /// item in the current list (no reorder).
    fn mark_read_and_jump(&mut self, dir: isize) {
        if self.scoped_items.is_empty() {
            return;
        }
        let (url, item) = self.scoped_items[self.list_sel].clone();
        self.db.set_read(&url, &item.guid, true).ok();
        if item.read_later {
            self.db.set_flag(&url, &item.guid, "read_later", false).ok();
        }
        let n = self.scoped_items.len() as isize;
        let mut i = self.list_sel as isize;
        loop {
            i += dir;
            if i < 0 || i >= n {
                // no unread in that direction — stay on current
                break;
            }
            let (u, it) = &self.scoped_items[i as usize];
            if !self.db.is_read(u, &it.guid).unwrap_or(false) {
                self.list_sel = i as usize;
                self.article_scroll = 0;
                break;
            }
        }
    }

    fn article_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => self.article_scroll = self.article_scroll.saturating_add(1),
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
            KeyCode::Char('d') => self.article_scroll = self.article_scroll.saturating_add(half),
            _ => {}
        }
    }

    /// Ctrl+f / Ctrl+b: full-page move — list selection or article scroll.
    fn page_scroll(&mut self, dir: isize) {
        let page = (self.article_area.height.saturating_sub(4)).max(1) as isize;
        match self.focus {
            1 => {
                let n = self.scoped_items.len() as isize;
                if n > 0 {
                    self.list_sel =
                        (self.list_sel as isize + dir * page).clamp(0, n - 1) as usize;
                    self.article_scroll = 0;
                }
            }
            2 => {
                self.article_scroll =
                    (self.article_scroll as isize + dir * page).max(0) as u16;
            }
            _ => {}
        }
    }

    /// Apply the current search query to the snapshot taken when `/` opened.
    pub(crate) fn apply_search_filter(&mut self, q: &str) {
        self.search_query = q.to_string();
        let Some(base) = &self.search_base else { return };
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            self.scoped_items = base.clone();
        } else {
            self.scoped_items = base
                .iter()
                .filter(|(_, i)| {
                    i.title.to_lowercase().contains(&q)
                        || i.summary.to_lowercase().contains(&q)
                })
                .cloned()
                .collect();
        }
        self.list_sel = 0;
        self.article_scroll = 0;
    }

    /// Re-run the active search filter after list mutations (append/rebuild).
    pub(crate) fn reapply_search_filter(&mut self) {
        if self.search_active {
            let q = self.search_query.clone();
            self.apply_search_filter(&q);
        }
    }

    /// Esc from the search box — restore the pre-search list.
    fn cancel_search(&mut self) {
        self.search_active = false;
        self.search_query.clear();
        if let Some(base) = self.search_base.take() {
            self.scoped_items = base;
        } else {
            // snapshot already dropped (enter kept the filter) — rebuild the
            // full scope list from scratch (active is false, so no re-filter)
            self.rebuild_list();
        }
        self.list_sel = 0;
    }

    /// Push a sort level (last pressed = highest priority); keep last 3.
    /// `reverse` inverts that level's direction (sT = time ascending).
    fn push_sort(&mut self, level: &str, reverse: bool) {
        self.sort_stack.retain(|(l, _)| l != level);
        self.sort_stack.insert(0, (level.to_string(), reverse));
        self.sort_stack.truncate(3);
        self.rebuild_list();
        let shown: Vec<String> = self
            .sort_stack
            .iter()
            .map(|(l, r)| format!("{}{}", if *r { "-" } else { "" }, l))
            .collect();
        self.status = format!("sort: {}", shown.join(" > "));
    }

    /// Sort the current list snapshot by the sort stack (no DB changes).
    pub(crate) fn apply_sort(&mut self) {
        if self.sort_stack.is_empty() {
            return;
        }
        self.scoped_items.sort_by(|(ua, a), (ub, b)| {
            let mut ord = std::cmp::Ordering::Equal;
            for (level, reverse) in &self.sort_stack {
                let mut o = match level.as_str() {
                    "time" => b.date.cmp(&a.date), // newest first
                    "title" => a.title.cmp(&b.title),
                    "feed" => ua.cmp(ub),
                    "unread" => {
                        let ra = self.db.is_read(ua, &a.guid).unwrap_or(false);
                        let rb = self.db.is_read(ub, &b.guid).unwrap_or(false);
                        ra.cmp(&rb)
                    }
                    _ => ord,
                };
                if *reverse {
                    o = o.reverse();
                }
                if o != std::cmp::Ordering::Equal {
                    ord = o;
                    break;
                }
            }
            ord
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_actions() {
        assert_eq!(Action::from_str("open"), Some(Action::Open));
        assert_eq!(Action::from_str("refresh_all"), Some(Action::RefreshAll));
        assert_eq!(Action::from_str("bogus"), None);
    }

    #[test]
    fn parse_keys() {
        assert_eq!(Action::parse_key("l"), Some(KeyCode::Char('l')));
        assert_eq!(Action::parse_key("L"), Some(KeyCode::Char('l')));
        assert_eq!(Action::parse_key("enter"), Some(KeyCode::Enter));
        assert_eq!(Action::parse_key("esc"), Some(KeyCode::Esc));
        assert_eq!(Action::parse_key("??"), None);
    }

    #[test]
    fn keymap_skips_invalid() {
        let mut raw = std::collections::HashMap::new();
        raw.insert("open".to_string(), "o".to_string());
        raw.insert("bogus".to_string(), "x".to_string());
        raw.insert("help".to_string(), "??".to_string());
        let m = build_keymap(&raw);
        assert_eq!(m.len(), 1);
        assert_eq!(m.get(&KeyCode::Char('o')), Some(&Action::Open));
    }
}
