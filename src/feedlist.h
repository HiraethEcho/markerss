#pragma once
#include <string>
#include <vector>

namespace markerss {

// One subscription line from a newsboat-format urls file.
struct Feed {
  std::string url;                    // first token
  std::string title;                  // quoted title, ~ stripped when custom
  bool custom_display = false;        // title had ~ prefix
  std::vector<std::string> tags;      // remaining tokens
};

// Parse a single urls line. Returns nullopt if line is blank/comment/malformed.
// Format: URL "Title" tag1 tag2   (quoted title optional; ~Title = custom name)
Feed* parse_line(const std::string& line, Feed* out);

// Parse whole file content. Skips blank/comment/malformed lines.
// Returns {feeds, skipped_count}.
std::pair<std::vector<Feed>, int> parse_urls(const std::string& content);

} // namespace markerss
