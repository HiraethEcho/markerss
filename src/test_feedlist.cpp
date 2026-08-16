// Plain assert-based tests for feedlist parser. No framework.
#include "feedlist.h"

#include <cstdio>
#include <cstdlib>
#include <string>

using markerss::Feed;
using markerss::parse_line;
using markerss::parse_urls;

static int failures = 0;
static int checks = 0;

#define CHECK(cond)                                                                 \
  do {                                                                              \
    ++checks;                                                                       \
    if (!(cond)) {                                                                  \
      ++failures;                                                                   \
      std::printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond);                   \
    }                                                                               \
  } while (0)

int main() {
  Feed f;

  // Quoted title.
  CHECK(parse_line("https://example.com/feed.xml \"Display Name\" tech", &f) != nullptr);
  CHECK(f.url == "https://example.com/feed.xml");
  CHECK(f.title == "Display Name");
  CHECK(!f.custom_display);
  CHECK(f.tags.size() == 1 && f.tags[0] == "tech");

  // ~ prefix inside quotes = custom display name, tilde stripped.
  CHECK(parse_line("https://other.example.org/rss \"~CustomName\" tech news", &f) != nullptr);
  CHECK(f.title == "CustomName");
  CHECK(f.custom_display);
  CHECK(f.tags.size() == 2 && f.tags[0] == "tech" && f.tags[1] == "news");

  // Multiple tags.
  CHECK(parse_line("https://tagged.example.net/rss \"Tagged\" one two three", &f) != nullptr);
  CHECK(f.tags.size() == 3 && f.tags[2] == "three");

  // No title: first token url, rest tags.
  CHECK(parse_line("https://plain.example.net/rss feed", &f) != nullptr);
  CHECK(f.url == "https://plain.example.net/rss");
  CHECK(f.title.empty());
  CHECK(!f.custom_display);
  CHECK(f.tags.size() == 1 && f.tags[0] == "feed");

  // URL only, no title no tags.
  CHECK(parse_line("https://bare.example.net/rss", &f) != nullptr);
  CHECK(f.url == "https://bare.example.net/rss");
  CHECK(f.title.empty() && f.tags.empty());

  // Comment line.
  CHECK(parse_line("# this is a comment", &f) == nullptr);

  // Blank line.
  CHECK(parse_line("   ", &f) == nullptr);
  CHECK(parse_line("", &f) == nullptr);

  // Unterminated quote = malformed.
  CHECK(parse_line("https://x.example/feed \"oops", &f) == nullptr);

  // Whole-file parse: blank + comment skipped, count correct.
  std::string doc = "# subscriptions\n"
                    "\n"
                    "https://a.example/feed \"A Feed\" cat1\n"
                    "  \n"
                    "https://b.example/feed \"~B\" cat2\n";
  auto [feeds, skipped] = parse_urls(doc);
  CHECK(feeds.size() == 2);
  CHECK(feeds[0].title == "A Feed" && feeds[0].tags == std::vector<std::string>{"cat1"});
  CHECK(feeds[1].title == "B" && feeds[1].custom_display);

  std::printf("%d checks, %d failures\n", checks, failures);
  return failures == 0 ? 0 : 1;
}
