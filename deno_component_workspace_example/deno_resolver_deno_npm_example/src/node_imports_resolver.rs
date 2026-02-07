use std::path::Path;

use deno_core::ModuleSpecifier;
use deno_error::JsErrorBox;
use serde_json::Value;

pub struct NodeImportsResolver {
  cache_dir: std::path::PathBuf,
}

impl NodeImportsResolver {
  pub fn new(cache_dir: std::path::PathBuf) -> Self {
    tracing::debug!("NodeImportsResolver cache_dir: {}", cache_dir.display());
    Self { cache_dir }
  }

  /// Resolve package.json imports field for # prefixed imports
  pub fn resolve_package_import(
    &self,
    import_specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, JsErrorBox> {
    tracing::debug!(
      "Resolving package import with NodeImportsResolver cache_dir={}",
      self.cache_dir.display()
    );

    if !import_specifier.starts_with('#') {
      return Err(JsErrorBox::generic("Not a package import"));
    }

    // Find the package.json file for the referrer
    let referrer_path = referrer
      .to_file_path()
      .map_err(|_| JsErrorBox::generic("Invalid referrer path"))?;

    let package_json_path = self.find_package_json(&referrer_path)?;
    let package_json_content = std::fs::read_to_string(&package_json_path)
      .map_err(|e| JsErrorBox::generic(format!("Failed to read package.json: {}", e)))?;

    let package_json: serde_json::Value = serde_json::from_str(&package_json_content)
      .map_err(|e| JsErrorBox::generic(format!("Failed to parse package.json: {}", e)))?;

    // Check imports field
    if let Some(imports) = package_json.get("imports") {
      if let Value::Object(imports_obj) = imports {
        if let Some(target) = self.resolve_import_target(imports_obj, import_specifier)? {
          // Resolve the target relative to package.json directory
          let package_dir = package_json_path
            .parent()
            .ok_or_else(|| JsErrorBox::generic("Invalid package.json path"))?;

          let resolved_path = package_dir.join(&target);

          return ModuleSpecifier::from_file_path(resolved_path)
            .map_err(|_| JsErrorBox::generic("Failed to create module specifier"));
        }
      }
    }

    Err(JsErrorBox::generic(format!(
      "Package import '{}' not found",
      import_specifier
    )))
  }

  fn find_package_json(&self, start_path: &Path) -> Result<std::path::PathBuf, JsErrorBox> {
    let mut current_dir = start_path
      .parent()
      .ok_or_else(|| JsErrorBox::generic("No parent directory"))?;

    loop {
      let package_json_path = current_dir.join("package.json");
      if package_json_path.exists() {
        return Ok(package_json_path);
      }

      if let Some(parent) = current_dir.parent() {
        current_dir = parent;
      } else {
        break;
      }
    }

    Err(JsErrorBox::generic("No package.json found"))
  }

  fn resolve_import_target(
    &self,
    imports: &serde_json::Map<String, Value>,
    import_specifier: &str,
  ) -> Result<Option<String>, JsErrorBox> {
    // First try exact match
    if let Some(target) = imports.get(import_specifier) {
      return Ok(Some(self.extract_target_string(target)?));
    }

    // Try pattern matching for wildcard imports
    for (pattern, target) in imports {
      if let Some(star_pos) = pattern.find('*') {
        let prefix = &pattern[.. star_pos];
        let suffix = &pattern[star_pos + 1 ..];

        if import_specifier.starts_with(prefix) && import_specifier.ends_with(suffix) {
          let matched_part =
            &import_specifier[prefix.len() .. import_specifier.len() - suffix.len()];

          // Replace * in target with matched part
          let target_str = self.extract_target_string(target)?;
          if target_str.contains('*') {
            let resolved_target = target_str.replace('*', matched_part);
            return Ok(Some(resolved_target));
          }
        }
      }
    }

    Ok(None)
  }

  fn extract_target_string(&self, target: &Value) -> Result<String, JsErrorBox> {
    match target {
      Value::String(s) => Ok(s.clone()),
      Value::Object(obj) => {
        // Handle conditional exports like { "default": "./path" }
        if let Some(default_target) = obj.get("default") {
          if let Value::String(s) = default_target {
            return Ok(s.clone());
          }
        }

        // Try other conditions like "node", "import", etc.
        for condition in &["node", "import", "require"] {
          if let Some(target) = obj.get(*condition) {
            if let Value::String(s) = target {
              return Ok(s.clone());
            }
          }
        }

        Err(JsErrorBox::generic(
          "No suitable target found in conditional export",
        ))
      }
      _ => Err(JsErrorBox::generic("Invalid target type in imports")),
    }
  }
}
