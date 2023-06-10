use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::HashMap;

#[derive(Debug)]
pub struct HaGraph<'a> {
    pub graph: Graph<&'a str, ()>,
    names: Vec<String>,
    pub nodes: HashMap<&'a str, NodeIndex>,
}

impl<'a> HaGraph<'a> {
    fn new() -> Self {
        HaGraph {
            graph: Graph::<&'a str, ()>::new(),
            names: vec![],
            nodes: HashMap::new(),
        }
    }
    pub fn has_node(&self, str: &str) -> bool {
        self.nodes.contains_key(str)
    }

    pub fn add_edge<'b>(&mut self, from: &'b str, to: &'b str) -> Result<(), &'b str> {
        let from_node = self.nodes.get(from).ok_or(from)?;
        let to_node = self.nodes.get(to).ok_or(to)?;
        self.graph.add_edge(*from_node, *to_node, ());
        Ok(())
    }

    pub fn load_names(&mut self, names: &'a [&'a str]) {
        for name in names {
            self.names.push(name.to_string());
            let idx = self.graph.add_node(&name);
            self.nodes.insert(&name, idx);
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
    hagraph.load_names(ttypes.as_slice());
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
