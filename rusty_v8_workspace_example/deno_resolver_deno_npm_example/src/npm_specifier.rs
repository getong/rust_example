use anyhow::{Result, anyhow};

/// NPM package specifier parser
#[derive(Debug, Clone, PartialEq)]
pub struct NpmSpecifier {
  pub name: String,
  pub version: Option<String>,
  pub sub_path: Option<String>,
}

impl NpmSpecifier {
  /// Parse an npm: specifier into components
  pub fn parse(specifier: &str) -> Result<Self> {
    let spec = specifier.strip_prefix("npm:").unwrap_or(specifier);

    if spec.is_empty() {
      return Err(anyhow!("Empty npm specifier"));
    }

    let (name, version, sub_path) = parse_npm_specifier(spec);

    Ok(Self {
      name,
      version: if version == "latest" || version.is_empty() {
        None
      } else {
        Some(version)
      },
      sub_path,
    })
  }
}

pub fn parse_npm_specifier(spec: &str) -> (String, String, Option<String>) {
  // Handle scoped packages like @types/node or @types/node@1.0.0
  if spec.starts_with('@') {
    if let Some(slash_pos) = spec[1 ..].find('/') {
      let scope_and_name_end = slash_pos + 1;
      let after_name = &spec[scope_and_name_end + 1 ..];

      if let Some(at_pos) = after_name.find('@') {
        // @scope/name@version or @scope/name@version/subpath
        let name = spec[.. scope_and_name_end + 1 + at_pos].to_string();
        let rest = &after_name[at_pos + 1 ..];

        if let Some(slash_pos) = rest.find('/') {
          let version = rest[.. slash_pos].to_string();
          let sub_path = rest[slash_pos + 1 ..].to_string();
          (name, version, Some(sub_path))
        } else {
          (name, rest.to_string(), None)
        }
      } else if let Some(slash_pos) = after_name.find('/') {
        // @scope/name/subpath
        let name = spec[.. scope_and_name_end + 1 + slash_pos].to_string();
        let sub_path = after_name[slash_pos + 1 ..].to_string();
        (name, "latest".to_string(), Some(sub_path))
      } else {
        // @scope/name
        (spec.to_string(), "latest".to_string(), None)
      }
    } else {
      // Invalid scoped package
      (spec.to_string(), "latest".to_string(), None)
    }
  } else {
    // Regular packages
    if let Some(at_pos) = spec.find('@') {
      let name = spec[.. at_pos].to_string();
      let rest = &spec[at_pos + 1 ..];

      if let Some(slash_pos) = rest.find('/') {
        let version = rest[.. slash_pos].to_string();
        let sub_path = rest[slash_pos + 1 ..].to_string();
        (name, version, Some(sub_path))
      } else {
        (name, rest.to_string(), None)
      }
    } else if let Some(slash_pos) = spec.find('/') {
      let name = spec[.. slash_pos].to_string();
      let sub_path = spec[slash_pos + 1 ..].to_string();
      (name, "latest".to_string(), Some(sub_path))
    } else {
      (spec.to_string(), "latest".to_string(), None)
    }
  }
}
