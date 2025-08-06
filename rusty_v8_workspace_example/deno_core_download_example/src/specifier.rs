use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

/// Represents a parsed npm: specifier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpmSpecifier {
  pub name: String,
  pub version: Option<String>,
  pub sub_path: Option<String>,
}

impl NpmSpecifier {
  /// Parse npm: specifier string
  ///
  /// Examples:
  /// - "npm:lodash" -> name: "lodash", version: None
  /// - "npm:lodash@4.17.21" -> name: "lodash", version: Some("4.17.21")
  /// - "npm:@supabase/supabase-js@2.40.0" -> name: "@supabase/supabase-js", version: Some("2.40.0")
  /// - "npm:lodash/fp" -> name: "lodash", sub_path: Some("fp")
  /// - "npm:@types/node/fs" -> name: "@types/node", sub_path: Some("fs")
  pub fn parse(specifier: &str) -> Result<Self> {
    if !specifier.starts_with("npm:") {
      return Err(anyhow!("Invalid npm specifier: must start with 'npm:'"));
    }

    let without_prefix = &specifier[4 ..]; // Remove "npm:" prefix

    // Handle empty specifier
    if without_prefix.is_empty() {
      return Err(anyhow!("Empty npm specifier"));
    }

    let (name_version, sub_path) = if let Some(slash_pos) = without_prefix.find('/') {
      // Check if this is a scoped package or sub-path
      if without_prefix.starts_with('@') {
        // Scoped package - find second slash for sub-path
        let remaining = &without_prefix[slash_pos + 1 ..];
        if let Some(second_slash) = remaining.find('/') {
          let name_version = &without_prefix[.. slash_pos + 1 + second_slash];
          let sub_path = &without_prefix[slash_pos + 1 + second_slash + 1 ..];
          (name_version, Some(sub_path.to_string()))
        } else {
          (without_prefix, None)
        }
      } else {
        // Regular package with sub-path
        let name_version = &without_prefix[.. slash_pos];
        let sub_path = &without_prefix[slash_pos + 1 ..];
        (name_version, Some(sub_path.to_string()))
      }
    } else {
      (without_prefix, None)
    };

    // Parse name and version
    let (name, version) = if let Some(at_pos) = name_version.rfind('@') {
      if name_version.starts_with('@') && at_pos == 0 {
        // Scoped package without version like "@types/node"
        (name_version.to_string(), None)
      } else if name_version.starts_with('@') {
        // Scoped package with version like "@types/node@18.0.0"
        let name = name_version[.. at_pos].to_string();
        let version = name_version[at_pos + 1 ..].to_string();
        (name, Some(version))
      } else {
        // Regular package with version like "lodash@4.17.21"
        let name = name_version[.. at_pos].to_string();
        let version = name_version[at_pos + 1 ..].to_string();
        (name, Some(version))
      }
    } else {
      // No version specified
      (name_version.to_string(), None)
    };

    // Validate package name
    if name.is_empty() {
      return Err(anyhow!("Empty package name"));
    }

    // Validate scoped package name format
    if name.starts_with('@') {
      if !name.contains('/') || name.split('/').count() != 2 {
        return Err(anyhow!("Invalid scoped package name format: {}", name));
      }
    }

    Ok(Self {
      name,
      version,
      sub_path,
    })
  }

  /// Convert back to npm: specifier string
  pub fn to_string(&self) -> String {
    let mut result = format!("npm:{}", self.name);

    if let Some(ref version) = self.version {
      result.push('@');
      result.push_str(version);
    }

    if let Some(ref sub_path) = self.sub_path {
      result.push('/');
      result.push_str(sub_path);
    }

    result
  }

  /// Get the registry URL path for this package
  pub fn registry_path(&self) -> String {
    // URL encode package name (replace / with %2F for scoped packages)
    self.name.replace('/', "%2F")
  }

  /// Check if this is a scoped package
  pub fn is_scoped(&self) -> bool {
    self.name.starts_with('@')
  }

  /// Get the package scope if this is a scoped package
  pub fn scope(&self) -> Option<&str> {
    if self.is_scoped() {
      self.name.split('/').next().map(|s| &s[1 ..]) // Remove @ prefix
    } else {
      None
    }
  }

  /// Get the package name without scope
  pub fn base_name(&self) -> &str {
    if self.is_scoped() {
      self.name.split('/').nth(1).unwrap_or(&self.name)
    } else {
      &self.name
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_simple_package() {
    let spec = NpmSpecifier::parse("npm:lodash").unwrap();
    assert_eq!(spec.name, "lodash");
    assert_eq!(spec.version, None);
    assert_eq!(spec.sub_path, None);
  }

  #[test]
  fn test_parse_package_with_version() {
    let spec = NpmSpecifier::parse("npm:lodash@4.17.21").unwrap();
    assert_eq!(spec.name, "lodash");
    assert_eq!(spec.version, Some("4.17.21".to_string()));
    assert_eq!(spec.sub_path, None);
  }

  #[test]
  fn test_parse_scoped_package() {
    let spec = NpmSpecifier::parse("npm:@supabase/supabase-js@2.40.0").unwrap();
    assert_eq!(spec.name, "@supabase/supabase-js");
    assert_eq!(spec.version, Some("2.40.0".to_string()));
    assert_eq!(spec.sub_path, None);
    assert!(spec.is_scoped());
    assert_eq!(spec.scope(), Some("supabase"));
    assert_eq!(spec.base_name(), "supabase-js");
  }

  #[test]
  fn test_parse_package_with_subpath() {
    let spec = NpmSpecifier::parse("npm:lodash/fp").unwrap();
    assert_eq!(spec.name, "lodash");
    assert_eq!(spec.version, None);
    assert_eq!(spec.sub_path, Some("fp".to_string()));
  }

  #[test]
  fn test_parse_scoped_package_with_subpath() {
    let spec = NpmSpecifier::parse("npm:@types/node/fs").unwrap();
    assert_eq!(spec.name, "@types/node");
    assert_eq!(spec.version, None);
    assert_eq!(spec.sub_path, Some("fs".to_string()));
  }

  #[test]
  fn test_parse_version_ranges() {
    let spec = NpmSpecifier::parse("npm:express@^4.18.0").unwrap();
    assert_eq!(spec.name, "express");
    assert_eq!(spec.version, Some("^4.18.0".to_string()));

    let spec = NpmSpecifier::parse("npm:react@~18.2.0").unwrap();
    assert_eq!(spec.name, "react");
    assert_eq!(spec.version, Some("~18.2.0".to_string()));
  }

  #[test]
  fn test_invalid_specifiers() {
    assert!(NpmSpecifier::parse("lodash").is_err());
    assert!(NpmSpecifier::parse("npm:").is_err());
    assert!(NpmSpecifier::parse("").is_err());
    assert!(NpmSpecifier::parse("npm:@invalid").is_err());
  }

  #[test]
  fn test_to_string() {
    let spec = NpmSpecifier {
      name: "@supabase/supabase-js".to_string(),
      version: Some("2.40.0".to_string()),
      sub_path: None,
    };
    assert_eq!(spec.to_string(), "npm:@supabase/supabase-js@2.40.0");
  }

  #[test]
  fn test_registry_path() {
    let spec = NpmSpecifier::parse("npm:@supabase/supabase-js").unwrap();
    assert_eq!(spec.registry_path(), "@supabase%2Fsupabase-js");

    let spec = NpmSpecifier::parse("npm:lodash").unwrap();
    assert_eq!(spec.registry_path(), "lodash");
  }
}
