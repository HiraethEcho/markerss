#pragma once
#include <string>

namespace markerss {

// XDG base dirs, resolved per spec (env var, else ~/.config etc.).
std::string xdg_config_home();
std::string xdg_cache_home();
std::string xdg_state_home();

// <base>/markerss
std::string markerss_config_dir();
std::string markerss_cache_dir();
std::string markerss_state_dir();

} // namespace markerss
