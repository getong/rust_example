use deno_graph::{
  GraphKind, ModuleGraph, ModuleSpecifier,
  source::{MemoryLoader, Source},
};
use futures::executor::block_on;

fn main() {
  let loader = MemoryLoader::new(
    vec![
      (
        "file:///test.ts",
        Source::Module {
          specifier: "file:///test.ts",
          maybe_headers: None,
          content: "import * as a from \"./a.ts\";",
        },
      ),
      (
        "file:///a.ts",
        Source::Module {
          specifier: "file:///a.ts",
          maybe_headers: None,
          content: "export const a = \"a\";",
        },
      ),
    ],
    Vec::new(),
  );
  let roots = vec![ModuleSpecifier::parse("file:///test.ts").unwrap()];
  let future = async move {
    let mut graph = ModuleGraph::new(GraphKind::All);
    graph
      .build(roots, vec![], &loader, Default::default())
      .await;
    println!("{:#?}", graph);
  };
  block_on(future)
}
