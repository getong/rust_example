#include "api.h"

#include <sstream>
#include <stdexcept>
#include <string_view>

namespace {
std::string_view as_view(rust::Str s) {
  return std::string_view(s.data(), s.size());
}
} // namespace

// 解析失败时 toml++ 抛出 toml::parse_error（继承 std::runtime_error），
// cxx 在生成的胶水层里 catch 住并转成 Rust 的 Err —— 这就是 bridge 中
// 声明 Result<UniquePtr<TomlTable>> 的由来，不需要手写错误码。
std::unique_ptr<TomlTable> parse_file(rust::Str path) {
  return std::make_unique<TomlTable>(toml::parse_file(as_view(path)));
}

rust::String to_toml_string(const TomlTable &table) {
  std::ostringstream oss;
  oss << table.table;
  return rust::String(oss.str());
}

bool contains_key(const TomlTable &table, rust::Str key) {
  return table.table.contains(as_view(key));
}

rust::String get_string(const TomlTable &table, rust::Str dotted_path) {
  auto node = table.table.at_path(as_view(dotted_path));
  if (auto s = node.value<std::string>()) {
    return rust::String(*s);
  }
  throw std::runtime_error("not found or not a string: " +
                           std::string(as_view(dotted_path)));
}
