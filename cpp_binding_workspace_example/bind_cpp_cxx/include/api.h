#pragma once

#include "bind_cpp_cxx/include/toml.hpp"
#include "rust/cxx.h"
#include <memory>

// cxx bridge 中声明的不透明类型 `type TomlTable`。
// Rust 侧只持有 UniquePtr<TomlTable>，不知道也不需要知道其布局；
// 生命周期由 UniquePtr 的 Drop 调用 ~TomlTable() 管理，无需手写 free。
class TomlTable {
public:
  explicit TomlTable(toml::table &&t) : table(std::move(t)) {}
  toml::table table;
};

std::unique_ptr<TomlTable> parse_file(rust::Str path);
rust::String to_toml_string(const TomlTable &table);
bool contains_key(const TomlTable &table, rust::Str key);
rust::String get_string(const TomlTable &table, rust::Str dotted_path);
