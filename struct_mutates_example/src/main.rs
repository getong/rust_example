use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::HashMap;

#[derive(Debug)]
pub struct HaGraph {
    pub graph: Graph<String, ()>,
    names: Vec<String>,
    pub nodes: HashMap<String, NodeIndex>,
}

impl HaGraph {
    fn new() -> Self {
        HaGraph {
            graph: Graph::<String, ()>::new(),
            names: Vec::new(),
            nodes: HashMap::new(),
        }
    }

    pub fn has_node(&self, name: &str) -> bool {
        self.nodes.contains_key(name)
    }

    pub fn add_edge<'a>(&'a mut self, from: &'a str, to: &'a str) -> Result<(), &str> {
        let from_node = self.nodes.get(from).ok_or(from)?;
        let to_node = self.nodes.get(to).ok_or(to)?;
        self.graph.add_edge(*from_node, *to_node, ());
        Ok(())
    }

    pub fn load_names(&mut self, names: &[&str]) {
        for &name in names {
            let name_string = name.to_string();
            self.names.push(name_string.clone());
            let idx = self.graph.add_node(name_string.clone());
            self.nodes.insert(name_string, idx);
        }
    }

    pub fn get_node(&self, name: &str) -> Option<&NodeIndex> {
        self.nodes.get(name)
    }
}

fn main() {
    let ttypes = vec![
        "amzapi.missingean-asin.solvepack",
        "amzapi.missingean-asin.solvepack_q",
        "amzapi.missingean-asin.solvepack_q2",
        "amzapi.missingean-asin.solvepack_q3",
    ];
    let mut hagraph = HaGraph::new();
    hagraph.load_names(&ttypes);
    println!("graph: {:?}", hagraph.graph);
    let transitions = vec![
        (
            "amzapi.missingean-asin.solvepack",
            "amzapi.missingean-asin.solvepack_q",
        ),
        (
            "amzapi.missingean-asin.solvepack_q",
            "amzapi.missingean-asin.solvepack_q2",
        ),
        (
            "amzapi.missingean-asin.solvepack_q2",
            "amzapi.missingean-asin.solvepack_q3",
        ),
        (
            "amzapi.missingean-asin.solvepack_q",
            "amzapi.missingean-asin.solvepack_q3",
        ),
    ];
    for (from, to) in transitions {
        _ = hagraph.add_edge(from, to);
    }
    println!("graph: {:?}", hagraph.graph);
}

// copy from https://www.reddit.com/r/rust/comments/142780f/struct_mutates_as_a_block_in_170/
// copy from https://play.rust-lang.org/?version=nightly&mode=debug&edition=2021&gist=44ebbc0ed6f69524ca18f03f46f8cad9
