// Copyright 2018-2025 the Deno authors. MIT license.
// Adapted from deno/cli/graph_container.rs

use std::sync::Arc;

use deno_core::parking_lot::RwLock;
use deno_graph::ModuleGraph;

pub trait ModuleGraphContainer: Clone + 'static {
  /// Acquires a permit to modify the module graph without other code
  /// having the chance to modify it. In the meantime, other code may
  /// still read from the existing module graph.
  async fn acquire_update_permit(&self) -> impl ModuleGraphUpdatePermit;
  /// Gets a copy of the graph.
  fn graph(&self) -> Arc<ModuleGraph>;
}

/// A permit for updating the module graph. When complete and
/// everything looks fine, calling `.commit()` will store the
/// new graph in the ModuleGraphContainer.
pub trait ModuleGraphUpdatePermit {
  /// Gets the module graph for mutation.
  fn graph_mut(&mut self) -> &mut ModuleGraph;
  /// Saves the mutated module graph in the container.
  fn commit(self);
}

/// Holds the `ModuleGraph` for the main worker.
#[derive(Clone)]
pub struct MainModuleGraphContainer {
  // Allow only one request to update the graph data at a time,
  // but allow other requests to read from it at any time even
  // while another request is updating the data.
  update_queue: Arc<deno_core::unsync::sync::TaskQueue>,
  inner: Arc<RwLock<Arc<ModuleGraph>>>,
}

impl MainModuleGraphContainer {
  pub fn new(graph_kind: deno_graph::GraphKind) -> Self {
    Self {
      update_queue: Default::default(),
      inner: Arc::new(RwLock::new(Arc::new(ModuleGraph::new(graph_kind)))),
    }
  }
}

impl ModuleGraphContainer for MainModuleGraphContainer {
  async fn acquire_update_permit(&self) -> impl ModuleGraphUpdatePermit {
    let permit = self.update_queue.acquire().await;
    MainModuleGraphUpdatePermit {
      permit,
      inner: self.inner.clone(),
      graph: self.inner.read().clone(),
    }
  }

  fn graph(&self) -> Arc<ModuleGraph> {
    self.inner.read().clone()
  }
}

/// A permit for updating the module graph. When complete and
/// everything looks fine, calling `.commit()` will store the
/// new graph in the ModuleGraphContainer.
pub struct MainModuleGraphUpdatePermit<'a> {
  permit: deno_core::unsync::sync::TaskQueuePermit<'a>,
  inner: Arc<RwLock<Arc<ModuleGraph>>>,
  graph: Arc<ModuleGraph>,
}

impl ModuleGraphUpdatePermit for MainModuleGraphUpdatePermit<'_> {
  fn graph_mut(&mut self) -> &mut ModuleGraph {
    Arc::make_mut(&mut self.graph)
  }

  fn commit(self) {
    *self.inner.write() = self.graph;
    drop(self.permit); // explicit drop for clarity
  }
}

/// Worker module graph container - a simpler version for workers
#[derive(Clone)]
pub struct WorkerModuleGraphContainer {
  inner: Arc<RwLock<Arc<ModuleGraph>>>,
  update_queue: Arc<deno_core::unsync::sync::TaskQueue>,
}

impl WorkerModuleGraphContainer {
  pub fn new(graph_kind: deno_graph::GraphKind) -> Self {
    Self {
      inner: Arc::new(RwLock::new(Arc::new(ModuleGraph::new(graph_kind)))),
      update_queue: Default::default(),
    }
  }
}

impl ModuleGraphContainer for WorkerModuleGraphContainer {
  async fn acquire_update_permit(&self) -> impl ModuleGraphUpdatePermit {
    let permit = self.update_queue.acquire().await;
    WorkerModuleGraphUpdatePermit {
      permit,
      inner: self.inner.clone(),
      graph: self.inner.read().clone(),
    }
  }

  fn graph(&self) -> Arc<ModuleGraph> {
    self.inner.read().clone()
  }
}

pub struct WorkerModuleGraphUpdatePermit<'a> {
  permit: deno_core::unsync::sync::TaskQueuePermit<'a>,
  inner: Arc<RwLock<Arc<ModuleGraph>>>,
  graph: Arc<ModuleGraph>,
}

impl ModuleGraphUpdatePermit for WorkerModuleGraphUpdatePermit<'_> {
  fn graph_mut(&mut self) -> &mut ModuleGraph {
    Arc::make_mut(&mut self.graph)
  }

  fn commit(self) {
    *self.inner.write() = self.graph;
    drop(self.permit);
  }
}
