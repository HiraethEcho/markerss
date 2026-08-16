#include "xdg.h"

#include <cstdlib>
#include <filesystem>

namespace markerss {
namespace {

std::string env_or(const char* name, const char* fallback) {
  const char* v = std::getenv(name);
  // XDG spec: ignore relative paths, use fallback.
  if (v && v[0] == '/') return v;
  return fallback;
}

std::string home() {
  const char* h = std::getenv("HOME");
  return h ? h : "";
}

} // namespace

std::string xdg_config_home() { return env_or("XDG_CONFIG_HOME", (home() + "/.config").c_str()); }
std::string xdg_cache_home()  { return env_or("XDG_CACHE_HOME",  (home() + "/.cache").c_str()); }
std::string xdg_state_home()  { return env_or("XDG_STATE_HOME",  (home() + "/.local/state").c_str()); }

std::string markerss_config_dir() { return (std::filesystem::path(xdg_config_home()) / "markerss").string(); }
std::string markerss_cache_dir()  { return (std::filesystem::path(xdg_cache_home())  / "markerss").string(); }
std::string markerss_state_dir()  { return (std::filesystem::path(xdg_state_home())  / "markerss").string(); }

} // namespace markerss
