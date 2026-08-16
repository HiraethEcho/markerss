#include "feedlist.h"

#include <optional>
#include <ranges>

namespace markerss {

namespace {

std::string trim(const std::string& s) {
  size_t b = s.find_first_not_of(" \t\r\n");
  if (b == std::string::npos) return "";
  size_t e = s.find_last_not_of(" \t\r\n");
  return s.substr(b, e - b + 1);
}

} // namespace

Feed* parse_line(const std::string& line, Feed* out) {
  std::string s = trim(line);
  if (s.empty() || s[0] == '#') return nullptr;

  // First token = URL.
  size_t sp = s.find_first_of(" \t");
  std::string url = sp == std::string::npos ? s : s.substr(0, sp);
  s = sp == std::string::npos ? "" : trim(s.substr(sp));
  if (url.empty()) return nullptr;

  out->url = url;
  out->title.clear();
  out->custom_display = false;
  out->tags.clear();

  // Quoted title (optional).
  if (!s.empty() && s[0] == '"') {
    size_t end = s.find('"', 1);
    if (end == std::string::npos) return nullptr; // unterminated quote
    std::string t = s.substr(1, end - 1);
    if (!t.empty() && t[0] == '~') {
      out->custom_display = true;
      t = t.substr(1);
    }
    out->title = t;
    s = trim(s.substr(end + 1));
  }

  // Remaining tokens = tags.
  while (!s.empty()) {
    size_t n = s.find_first_of(" \t");
    out->tags.push_back(n == std::string::npos ? s : s.substr(0, n));
    if (n == std::string::npos) break;
    s = trim(s.substr(n));
  }
  return out;
}

std::pair<std::vector<Feed>, int> parse_urls(const std::string& content) {
  std::vector<Feed> feeds;
  int skipped = 0;
  for (const auto& raw : std::views::split(content, '\n')) {
    Feed f;
    if (parse_line(std::string(raw.begin(), raw.end()), &f)) {
      feeds.push_back(std::move(f));
    } else {
      ++skipped;
    }
  }
  return {feeds, skipped};
}

} // namespace markerss
