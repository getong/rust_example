use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow};

use crate::{npm_downloader::NpmConfig, npm_registry::NpmRegistry, npm_specifier::NpmSpecifier};

/// Analyzes npm package dependencies and creates a dependency tree
pub struct NpmDependencyAnalyzer {
  registry: NpmRegistry,
}

#[derive(Debug, Clone)]
pub struct DependencyNode {
  pub name: String,
  pub version: String,
  pub dependencies: HashMap<String, DependencyNode>,
  pub dev_dependencies: HashMap<String, DependencyNode>,
  pub peer_dependencies: HashMap<String, DependencyNode>,
}

impl NpmDependencyAnalyzer {
  pub fn new() -> Result<Self> {
    let config = NpmConfig::default();
    let registry = NpmRegistry::new(&config)?;

    Ok(Self { registry })
  }

  /// Analyze dependencies for a package without downloading
  pub async fn analyze_dependencies(&self, specifier: &str) -> Result<DependencyNode> {
    let mut analyzed = HashSet::new();
    self
      .analyze_package_recursive(specifier, &mut analyzed)
      .await
  }

  fn analyze_package_recursive<'a>(
    &'a self,
    specifier: &'a str,
    analyzed: &'a mut HashSet<String>,
  ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<DependencyNode>> + 'a>> {
    Box::pin(async move {
      // Parse the npm: specifier
      let npm_spec = NpmSpecifier::parse(specifier)?;

      // Avoid circular dependencies
      let package_key = format!(
        "{}@{}",
        npm_spec.name,
        npm_spec.version.as_deref().unwrap_or("latest")
      );
      if analyzed.contains(&package_key) {
        return Ok(DependencyNode {
          name: npm_spec.name,
          version: "circular".to_string(),
          dependencies: HashMap::new(),
          dev_dependencies: HashMap::new(),
          peer_dependencies: HashMap::new(),
        });
      }
      analyzed.insert(package_key);

      // Fetch package metadata from registry
      let metadata = self.registry.get_package_metadata(&npm_spec.name).await?;

      // Resolve version
      let resolved_version = match &npm_spec.version {
        Some(version_req) => {
          // Simple version resolution - just take the first matching version
          let mut versions: Vec<semver::Version> = Vec::new();
          for version_str in metadata.versions.keys() {
            if let Ok(version) = semver::Version::parse(version_str) {
              versions.push(version);
            }
          }
          versions.sort();
          versions.reverse();

          let req = semver::VersionReq::parse(version_req)?;
          let mut found_version = None;
          for version in &versions {
            if req.matches(version) {
              found_version = Some(version.to_string());
              break;
            }
          }

          found_version.unwrap_or_else(|| {
            versions
              .first()
              .map(|v| v.to_string())
              .unwrap_or_else(|| "latest".to_string())
          })
        }
        None => metadata
          .dist_tags
          .get("latest")
          .cloned()
          .unwrap_or_else(|| "unknown".to_string()),
      };

      let version_info = metadata
        .versions
        .get(&resolved_version)
        .ok_or_else(|| anyhow!("Version {} not found", resolved_version))?;

      let mut node = DependencyNode {
        name: npm_spec.name,
        version: resolved_version,
        dependencies: HashMap::new(),
        dev_dependencies: HashMap::new(),
        peer_dependencies: HashMap::new(),
      };

      // Analyze regular dependencies
      if let Some(deps) = &version_info.dependencies {
        for (dep_name, version_spec) in deps {
          let dep_spec = format!("npm:{}@{}", dep_name, version_spec);
          if let Ok(dep_node) = self.analyze_package_recursive(&dep_spec, analyzed).await {
            node.dependencies.insert(dep_name.clone(), dep_node);
          }
        }
      }

      // Analyze dev dependencies
      if let Some(dev_deps) = &version_info.dev_dependencies {
        for (dep_name, version_spec) in dev_deps {
          let dep_spec = format!("npm:{}@{}", dep_name, version_spec);
          if let Ok(dep_node) = self.analyze_package_recursive(&dep_spec, analyzed).await {
            node.dev_dependencies.insert(dep_name.clone(), dep_node);
          }
        }
      }

      Ok(node)
    })
  }

  /// Print a dependency tree
  pub fn print_dependency_tree(&self, node: &DependencyNode, prefix: &str, is_last: bool) {
    let current_prefix = if prefix.is_empty() {
      ""
    } else if is_last {
      "└── "
    } else {
      "├── "
    };

    println!("{}{}{}@{}", prefix, current_prefix, node.name, node.version);

    let children: Vec<_> = node.dependencies.iter().collect();
    let total_children = children.len();

    for (i, (_, child)) in children.iter().enumerate() {
      let is_last_child = i == total_children - 1;
      let new_prefix = if prefix.is_empty() {
        if is_last_child {
          "    ".to_string()
        } else {
          "│   ".to_string()
        }
      } else {
        format!("{}{}", prefix, if is_last { "    " } else { "│   " })
      };

      self.print_dependency_tree(child, &new_prefix, is_last_child);
    }
  }

  /// Get dependency count statistics
  pub fn get_dependency_stats(&self, node: &DependencyNode) -> (usize, usize, usize) {
    let mut visited = HashSet::new();
    self.count_dependencies_recursive(node, &mut visited)
  }

  fn count_dependencies_recursive(
    &self,
    node: &DependencyNode,
    visited: &mut HashSet<String>,
  ) -> (usize, usize, usize) {
    let key = format!("{}@{}", node.name, node.version);
    if visited.contains(&key) {
      return (0, 0, 0);
    }
    visited.insert(key);

    let mut total_deps = node.dependencies.len();
    let mut total_dev_deps = node.dev_dependencies.len();
    let mut total_peer_deps = node.peer_dependencies.len();

    // Count recursive dependencies
    for (_, child) in &node.dependencies {
      let (deps, dev_deps, peer_deps) = self.count_dependencies_recursive(child, visited);
      total_deps += deps;
      total_dev_deps += dev_deps;
      total_peer_deps += peer_deps;
    }

    for (_, child) in &node.dev_dependencies {
      let (deps, dev_deps, peer_deps) = self.count_dependencies_recursive(child, visited);
      total_deps += deps;
      total_dev_deps += dev_deps;
      total_peer_deps += peer_deps;
    }

    (total_deps, total_dev_deps, total_peer_deps)
  }
}

/// CLI tool to analyze npm package dependencies
pub async fn analyze_npm_package(package_name: &str) -> Result<()> {
  let analyzer = NpmDependencyAnalyzer::new()?;

  println!("🔍 Analyzing dependencies for: {}", package_name);
  println!("{}", "=".repeat(60));

  let root_node = analyzer.analyze_dependencies(package_name).await?;

  println!("📦 Dependency Tree:");
  analyzer.print_dependency_tree(&root_node, "", true);

  let (deps, dev_deps, peer_deps) = analyzer.get_dependency_stats(&root_node);

  println!("\n📊 Statistics:");
  println!("   Regular dependencies: {}", deps);
  println!("   Dev dependencies: {}", dev_deps);
  println!("   Peer dependencies: {}", peer_deps);
  println!("   Total unique packages: {}", deps + dev_deps + peer_deps);

  Ok(())
}
