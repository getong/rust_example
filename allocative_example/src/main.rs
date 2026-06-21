use allocative::{Allocative, FlameGraphBuilder, size_of_unique_allocated_data};

#[derive(Allocative)]
#[allow(dead_code)]
struct ServiceState {
  name: String,
  requests: Vec<RequestRecord>,
  response_cache: Vec<(String, CachedResponse)>,
  scratch: Vec<u64>,
}

#[derive(Allocative)]
#[allow(dead_code)]
struct RequestRecord {
  path: String,
  tags: Vec<String>,
  body: Box<[u8]>,
}

#[derive(Allocative)]
#[allow(dead_code)]
struct CachedResponse {
  status: u16,
  headers: Vec<(String, String)>,
  body: Box<[u8]>,
}

fn main() {
  let state = build_sample_state();
  let folded_stacks = folded_stacks_for(&state);

  println!("allocative flamegraph folded stacks:");
  println!("{folded_stacks}");

  println!(
    "unique heap allocation estimate: {} bytes",
    size_of_unique_allocated_data(&state)
  );

  let mut compact_state = build_sample_state();
  compact_state.scratch.shrink_to_fit();
  compact_state.requests.shrink_to_fit();

  println!("\nmerged folded stacks for two roots:");
  println!("{}", folded_stacks_for_pair(&state, &compact_state));
}

fn folded_stacks_for(root: &impl Allocative) -> String {
  let mut flamegraph = FlameGraphBuilder::default();
  flamegraph.visit_root(root);

  let output = flamegraph.finish();
  let warnings = output.warnings();
  if !warnings.is_empty() {
    eprintln!("allocative warnings:\n{warnings}");
  }

  output.flamegraph().write()
}

fn folded_stacks_for_pair(left: &impl Allocative, right: &impl Allocative) -> String {
  let mut flamegraph = FlameGraphBuilder::default();
  {
    let mut visitor = flamegraph.root_visitor();
    visitor.visit_field(allocative::ident_key!(original), left);
    visitor.visit_field(allocative::ident_key!(compact), right);
    visitor.exit();
  }

  flamegraph.finish_and_write_flame_graph()
}

fn build_sample_state() -> ServiceState {
  let mut scratch = Vec::with_capacity(256);
  scratch.extend(0 .. 32);

  let requests = vec![
    RequestRecord {
      path: "/search?q=allocative".to_owned(),
      tags: vec!["search".to_owned(), "hot-path".to_owned()],
      body: vec![7; 128].into_boxed_slice(),
    },
    RequestRecord {
      path: "/profile/42".to_owned(),
      tags: vec!["profile".to_owned(), "cacheable".to_owned()],
      body: vec![3; 384].into_boxed_slice(),
    },
  ];

  let response_cache = vec![
    (
      "/search?q=allocative".to_owned(),
      CachedResponse {
        status: 200,
        headers: vec![
          ("content-type".to_owned(), "application/json".to_owned()),
          ("cache-control".to_owned(), "max-age=30".to_owned()),
        ],
        body: vec![1; 1_024].into_boxed_slice(),
      },
    ),
    (
      "/profile/42".to_owned(),
      CachedResponse {
        status: 200,
        headers: vec![("content-type".to_owned(), "application/json".to_owned())],
        body: vec![2; 2_048].into_boxed_slice(),
      },
    ),
  ];

  ServiceState {
    name: "allocative demo service".to_owned(),
    requests,
    response_cache,
    scratch,
  }
}
