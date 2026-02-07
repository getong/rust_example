use std::path::PathBuf;

use deno_resolver::npm::{ByonmNpmResolver, ByonmNpmResolverCreateOptions};
use node_resolver::cache::NodeResolutionSys;
use sys_traits::impls::RealSys;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Create a system implementation
  let sys = RealSys::default();

  // Create a package.json resolver
  // For this example, we'll create a simple one
  let pkg_json_resolver =
    deno_maybe_sync::new_rc(node_resolver::PackageJsonResolver::new(sys.clone(), None));

  // Create options for ByonmNpmResolver
  let options = ByonmNpmResolverCreateOptions {
    root_node_modules_dir: Some(PathBuf::from("./node_modules")),
    sys: NodeResolutionSys::new(sys, None),
    pkg_json_resolver,
  };

  // Create the ByonmNpmResolver
  let resolver = ByonmNpmResolver::new(options);

  // Example usage: Get the root node_modules path
  if let Some(root_path) = resolver.root_node_modules_path() {
    println!("Root node_modules path: {:?}", root_path);
  }

  // Example: Try to find an ancestor package.json with a dependency
  let referrer_url = Url::parse("file:///path/to/some/file.js")?;
  if let Some(pkg_json) = resolver.find_ancestor_package_json_with_dep("lodash", &referrer_url) {
    println!(
      "Found package.json with lodash dependency: {:?}",
      pkg_json.path
    );
  } else {
    println!("No package.json found with lodash dependency");
  }

  // Example: Try to resolve a package folder from a Deno module request
  // This would require a proper PackageReq, but for demonstration:
  // let req = PackageReq::parse("lodash@4.17.21")?;
  // match resolver.resolve_pkg_folder_from_deno_module_req(&req, &referrer_url) {
  //     Ok(path) => println!("Resolved package path: {:?}", path),
  //     Err(e) => println!("Failed to resolve package: {:?}", e),
  // }

  println!("ByonmNpmResolver example completed successfully!");
  Ok(())
}
