// Copyright 2018-2025 the Deno authors. MIT license.
// Adapted from deno/cli/lib/loader.rs

use deno_ast::MediaType;
use deno_core::{FastString, ModuleSourceCode, ModuleType, RequestedModuleType};

pub fn module_type_from_media_and_requested_type(
  media_type: MediaType,
  requested_module_type: &RequestedModuleType,
) -> ModuleType {
  match requested_module_type {
    RequestedModuleType::Text => ModuleType::Text,
    RequestedModuleType::Bytes => ModuleType::Bytes,
    RequestedModuleType::None | RequestedModuleType::Other(_) | RequestedModuleType::Json => {
      match media_type {
        MediaType::Json => ModuleType::Json,
        MediaType::Wasm => ModuleType::Wasm,
        _ => ModuleType::JavaScript,
      }
    }
  }
}

pub fn string_to_module_source_code(text: String) -> ModuleSourceCode {
  ModuleSourceCode::String(FastString::from(text))
}

#[allow(dead_code)]
pub fn bytes_to_module_source_code(bytes: Vec<u8>) -> ModuleSourceCode {
  ModuleSourceCode::Bytes(bytes.into_boxed_slice().into())
}

/// Helper to determine media type from a file path
#[allow(dead_code)]
pub fn media_type_from_path(path: &std::path::Path) -> MediaType {
  MediaType::from_path(path)
}

/// Helper to determine if a specifier is likely a CommonJS module
#[allow(dead_code)]
pub fn is_likely_cjs(specifier: &deno_core::ModuleSpecifier) -> bool {
  let path = specifier.path();
  path.ends_with(".cjs") || path.contains("/node_modules/")
}
