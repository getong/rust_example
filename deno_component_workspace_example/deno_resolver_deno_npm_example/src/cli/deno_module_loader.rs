// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_cache_dir::file_fetcher::FetchLocalOptions;
use deno_core::FastString;
use deno_core::ModuleLoader;
use deno_core::ModuleResolutionError;
use deno_core::ModuleSource;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::SourceCodeCacheInfo;
use deno_core::anyhow::Context as _;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::error::ModuleLoaderError;
use deno_core::futures::StreamExt;
use deno_core::futures::future::FutureExt;
use deno_core::futures::io::BufReader;
use deno_core::futures::stream::FuturesOrdered;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url;
use deno_core::serde_json;
use deno_error::JsErrorBox;
use deno_graph::GraphKind;
use deno_graph::ModuleGraph;
use deno_graph::WalkOptions;
use deno_lib::loader::as_deno_resolver_requested_module_type;
use deno_lib::loader::loaded_module_source_to_module_source_code;
use deno_lib::loader::module_type_from_media_and_requested_type;
use deno_lib::npm::NpmRegistryReadPermissionChecker;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lib::worker::CreateModuleLoaderResult;
use deno_lib::worker::ModuleLoaderFactory;
use deno_path_util::PathToUrlError;
use deno_path_util::resolve_url_or_path;
use deno_resolver::cache::ParsedSourceCache;
use deno_resolver::file_fetcher::FetchOptions;
use deno_resolver::file_fetcher::FetchPermissionsOptionRef;
use deno_resolver::graph::ResolveWithGraphErrorKind;
use deno_resolver::graph::ResolveWithGraphOptions;
use deno_resolver::loader::LoadCodeSourceError;
use deno_resolver::loader::LoadPreparedModuleError;
use deno_resolver::loader::LoadedModule;
use deno_resolver::loader::LoadedModuleOrAsset;
use deno_resolver::loader::StrippingTypesNodeModulesError;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_runtime::code_cache;
use deno_runtime::deno_node::NodeRequireLoader;
use deno_runtime::deno_node::create_host_defined_options;
use deno_runtime::deno_node::ops::require::UnableToGetCwdError;
use deno_runtime::deno_permissions::CheckSpecifierKind;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use eszip::EszipV2;
use node_resolver::InNpmPackageChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;
use sys_traits::FsRead;
use tokio_util::compat::TokioAsyncReadCompatExt;

// Placeholder imports - these would need to be implemented or replaced
// use crate::args::CliLockfile;
// use crate::args::CliOptions;
// use crate::args::DenoSubcommand;
// use crate::args::TsTypeLib;
// use crate::args::jsr_url;
// use crate::cache::CodeCache;
// use crate::file_fetcher::CliFileFetcher;
// use crate::graph_container::MainModuleGraphContainer;
// use crate::graph_container::ModuleGraphContainer;
// use crate::graph_container::ModuleGraphUpdatePermit;
// use crate::graph_util::BuildGraphRequest;
// use crate::graph_util::BuildGraphWithNpmOptions;
// use crate::graph_util::ModuleGraphBuilder;
use crate::cli::npm::CliNpmResolver;
use crate::cli::deno_resolver::CliCjsTracker;
use crate::cli::deno_resolver::CliResolver;
use crate::cli::CliSys;
// use crate::type_checker::CheckError;
// use crate::type_checker::CheckOptions;
// use crate::type_checker::TypeChecker;
use crate::cli::util::progress_bar::ProgressBar;
// use crate::util::text_encoding::code_without_source_map;
// use crate::util::text_encoding::source_map_from_code;

pub type CliEmitter =
  deno_resolver::emit::Emitter<DenoInNpmPackageChecker, CliSys>;
pub type CliDenoResolverModuleLoader =
  deno_resolver::loader::ModuleLoader<CliSys>;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum PrepareModuleLoadError {
  // #[class(inherit)]
  // #[error(transparent)]
  // BuildGraphWithNpmResolution(
  //   #[from] crate::graph_util::BuildGraphWithNpmResolutionError,
  // ),
  // #[class(inherit)]
  // #[error(transparent)]
  // Check(#[from] CheckError),
  #[class(inherit)]
  #[error(transparent)]
  LockfileWrite(#[from] deno_resolver::lockfile::LockfileWriteError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

pub struct CliModuleLoaderState {
  pub(crate) shared: Arc<SharedCliModuleLoaderState>,
}

pub(crate) struct SharedCliModuleLoaderState {
  // pub(crate) graph_kind: GraphKind,
  // pub(crate) lib_window: TsTypeLib,
  // pub(crate) lib_worker: TsTypeLib,
  // pub(crate) in_npm_pkg_checker: DenoInNpmPackageChecker,
  // pub(crate) npm_resolver: Arc<CliNpmResolver>,
  // pub(crate) cjs_tracker: CliCjsTracker,
  // pub(crate) npm_req_resolver: CliNpmReqResolver,
  // pub(crate) parsed_source_cache: Arc<ParsedSourceCache>,
  // pub(crate) module_loader: Arc<CliDenoResolverModuleLoader>,
  // pub(crate) resolver: Arc<CliResolver>,
  // pub(crate) sys: CliSys,
  // pub(crate) in_flight_loads_tracker: InFlightModuleLoadsTracker,
  // pub(crate) maybe_eszip_loader: Option<Arc<EszipModuleLoader>>,
}
