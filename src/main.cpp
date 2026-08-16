// markerss TUI skeleton: three-pane layout (nav | list | article).
// Phase 1-2 scaffold only: static placeholder data, nav tree from urls parser.
#include "feedlist.h"
#include "xdg.h"

#include <ncurses.h>

#include <algorithm>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

namespace {

// Sample subscriptions used when no urls file exists yet.
const char* kSampleUrls = R"(# markerss sample subscriptions
https://example.com/feed.xml "Example Feed" tech news
https://other.example.org/rss "~CustomName" tech
https://plain.example.net/plain.xml
https://tagged.example.net/rss "Tagged" one two
)";

std::string read_file(const std::string& path) {
  std::ifstream in(path);
  if (!in) return "";
  std::ostringstream ss;
  ss << in.rdbuf();
  return ss.str();
}

std::vector<markerss::Feed> load_feeds(int argc, char** argv) {
  // Priority: argv path > $XDG_CONFIG_HOME/markerss/urls > embedded sample.
  if (argc > 1) {
    std::string c = read_file(argv[1]);
    if (!c.empty()) return markerss::parse_urls(c).first;
  }
  std::string cfg = markerss::markerss_config_dir() + "/urls";
  std::string c = read_file(cfg);
  if (!c.empty()) return markerss::parse_urls(c).first;
  return markerss::parse_urls(kSampleUrls).first;
}

struct NavFeed {
  std::string label;
  int unread; // placeholder until Phase 3 state DB
};

struct NavCat {
  std::string name;               // first tag; empty = uncategorized
  std::vector<NavFeed> feeds;
};

std::vector<NavCat> build_nav(const std::vector<markerss::Feed>& feeds) {
  std::vector<NavCat> cats;
  for (const auto& f : feeds) {
    std::string cat = f.tags.empty() ? "" : f.tags[0];
    auto it = std::find_if(cats.begin(), cats.end(),
                           [&](const NavCat& c) { return c.name == cat; });
    if (it == cats.end()) {
      cats.push_back({cat, {}});
      it = cats.end() - 1;
    }
    std::string label = f.custom_display || !f.title.empty()
                            ? f.title
                            : f.url; // display name; real title comes from feed (Phase 5)
    it->feeds.push_back({label, static_cast<int>(it->feeds.size() * 5 % 13)}); // placeholder count
  }
  return cats;
}

void draw_clipped(WINDOW* w, int y, int x, const std::string& s, int maxw) {
  int len = static_cast<int>(s.size());
  if (len > maxw) len = maxw;
  mvwaddnstr(w, y, x, s.c_str(), len);
}

void draw_nav(WINDOW* w, const std::vector<NavCat>& cats) {
  int y = 1, maxy = getmaxy(w) - 2, maxx = getmaxx(w) - 3;
  if (y <= maxy) draw_clipped(w, y++, 2, "All Unread (17)", maxx); // placeholder aggregate
  for (const auto& c : cats) {
    if (y > maxy) break;
    if (c.name.empty()) {
      for (const auto& f : c.feeds) {
        if (y > maxy) break;
        draw_clipped(w, y++, 2, f.label + " (" + std::to_string(f.unread) + ")", maxx);
      }
    } else {
      draw_clipped(w, y++, 2, "> " + c.name, maxx);
      for (const auto& f : c.feeds) {
        if (y > maxy) break;
        draw_clipped(w, y++, 4, f.label + " (" + std::to_string(f.unread) + ")", maxx);
      }
    }
  }
}

void draw_list(WINDOW* w) {
  static const char* items[] = {
      "1. placeholder item title one", "2. placeholder item title two",
      "3. placeholder item title three", "4. placeholder item title four",
  };
  int maxy = getmaxy(w) - 2, maxx = getmaxx(w) - 3;
  for (int i = 0; i < 4 && 1 + i <= maxy; ++i)
    draw_clipped(w, 1 + i, 2, items[i], maxx);
}

void draw_article(WINDOW* w) {
  int maxx = getmaxx(w) - 3;
  int y = 1;
  draw_clipped(w, y++, 2, "placeholder title", maxx);
  draw_clipped(w, y++, 2, "feed name - 2026-08-01 - https://example.com/post", maxx);
  draw_clipped(w, y++, 2, "", maxx);
  draw_clipped(w, y++, 2, "summary line (Phase 5: real feed data)", maxx);
  draw_clipped(w, y++, 2, "", maxx);
  draw_clipped(w, y++, 2, "content: (Phase 5: full article)", maxx);
}

void draw_panes(int focus, const std::vector<NavCat>& cats) {
  int rows = getmaxy(stdscr), cols = getmaxx(stdscr);
  if (rows < 6 || cols < 30) { // too small
    mvaddstr(0, 0, "terminal too small");
    return;
  }
  int nav_w = std::max(cols / 4, 18), list_w = std::max(cols / 3, 20);
  int art_w = cols - nav_w - list_w;

  WINDOW* nav = newwin(rows, nav_w, 0, 0);
  WINDOW* list = newwin(rows, list_w, 0, nav_w);
  WINDOW* art = newwin(rows, art_w, 0, nav_w + list_w);

  const char* titles[3] = {"Nav", "List", "Article"};
  WINDOW* wins[3] = {nav, list, art};
  for (int i = 0; i < 3; ++i) {
    if (i == focus) wattron(wins[i], A_REVERSE);
    box(wins[i], 0, 0);
    mvwaddstr(wins[i], 0, 2, titles[i]);
    if (i == focus) wattroff(wins[i], A_REVERSE);
  }
  draw_nav(nav, cats);
  draw_list(list);
  draw_article(art);
  mvaddstr(rows - 1, 0, "q quit | Tab focus | skeleton only");
  refresh();
  for (auto* win : wins) wrefresh(win);
  for (auto* win : wins) delwin(win);
}

} // namespace

int main(int argc, char** argv) {
  auto feeds = load_feeds(argc, argv);
  auto cats = build_nav(feeds);

  initscr();
  cbreak();
  noecho();
  keypad(stdscr, TRUE);
  curs_set(0);

  int focus = 0;
  draw_panes(focus, cats);
  for (;;) {
    int ch = getch();
    if (ch == 'q') break;
    if (ch == '\t' || ch == KEY_BTAB) focus = (focus + 2) % 3;
    if (ch == KEY_RESIZE) resize_term(0, 0);
    draw_panes(focus, cats);
  }
  endwin();
  return 0;
}
