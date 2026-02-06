// Copyright 2018-2026 the Deno authors. MIT license.

use deno_resolver::{cjs::analyzer::DenoCjsCodeAnalyzer, npm::DenoInNpmPackageChecker};
use node_resolver::{DenoIsBuiltInNodeModuleChecker, analyze::CjsModuleExportAnalyzer};

use crate::{npm::CliNpmResolver, sys::CliSys};

pub type CliCjsCodeAnalyzer = DenoCjsCodeAnalyzer<CliSys>;

pub type CliCjsModuleExportAnalyzer = CjsModuleExportAnalyzer<
  CliCjsCodeAnalyzer,
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  CliNpmResolver,
  CliSys,
>;
pub type CliNodeResolver<TSys = CliSys> =
  deno_runtime::deno_node::NodeResolver<DenoInNpmPackageChecker, CliNpmResolver<TSys>, TSys>;
pub type CliPackageJsonResolver<TSys = CliSys> = node_resolver::PackageJsonResolver<TSys>;
