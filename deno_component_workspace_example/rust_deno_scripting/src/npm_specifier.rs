use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct NpmSpecifier {
  pub name: String,
  pub version: String,
}

impl NpmSpecifier {
  pub fn parse(specifier: &str) -> Result<Self> {
    if !specifier.starts_with("npm:") {
      return Err(anyhow!("Not an npm: specifier"));
    }

    let npm_part = specifier.strip_prefix("npm:").unwrap();
    let (name, version, _sub_path) = parse_npm_specifier(npm_part);

    Ok(Self { name, version })
  }
}

pub fn parse_npm_specifier(specifier: &str) -> (String, String, Option<String>) {
  // Handle scoped packages like @types/node
  let (package_part, sub_path) = if specifier.starts_with('@') {
    // Find the second '/' for scoped packages
    if let Some(first_slash) = specifier.find('/') {
      if let Some(second_slash) = specifier[first_slash + 1 ..].find('/') {
        let total_pos = first_slash + 1 + second_slash;
        (
          &specifier[.. total_pos],
          Some(specifier[total_pos + 1 ..].to_string()),
        )
      } else {
        (specifier, None)
      }
    } else {
      (specifier, None)
    }
  } else {
    // Regular packages
    if let Some(slash_pos) = specifier.find('/') {
      (
        &specifier[.. slash_pos],
        Some(specifier[slash_pos + 1 ..].to_string()),
      )
    } else {
      (specifier, None)
    }
  };

  // Split name and version
  let (name, version) = if let Some(at_pos) = package_part.rfind('@') {
    // Make sure this @ is not part of a scoped package name
    if at_pos == 0 {
      (package_part.to_string(), "latest".to_string())
    } else {
      (
        package_part[.. at_pos].to_string(),
        package_part[at_pos + 1 ..].to_string(),
      )
    }
  } else {
    (package_part.to_string(), "latest".to_string())
  };

  (name, version, sub_path)
}
