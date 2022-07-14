use std::collections::{HashMap, HashSet};

type Family = Vec<HashSet<u32>>;
type Graph = HashMap<usize, HashSet<usize>>;

fn max_disjoint_subset(f: &Family) -> Family {
    let max_indep = maximum_independent_set(&create_graph(f));
    f.iter()
        .enumerate()
        .filter(|(idx, _)| max_indep.contains(idx))
        .map(|(_, s)| s.clone())
        .collect()
}

fn create_graph(f: &Family) -> Graph {
    let mut g = Graph::new();
    for (idx, _) in f.iter().enumerate() {
        g.insert(
            idx,
            f.iter()
                .enumerate()
                .filter(|(v_idx, v)| v_idx != &idx && !f[idx].is_disjoint(v))
                .map(|(v_idx, _)| v_idx)
                .collect(),
        );
    }
    g
}

fn maximum_independent_set(g: &Graph) -> HashSet<usize> {
    if g.is_empty() {
        HashSet::new()
    } else {
        let mut res = HashSet::new();
        for v in g.keys() {
            let mut g_red = g.clone();
            remove_vertex_and_neighbors(&mut g_red, *v);
            let mut mis = maximum_independent_set(&g_red);
            mis.insert(*v);
            if mis.len() > res.len() {
                res = mis;
            }
        }
        res
    }
}

fn remove_vertex(g: &mut Graph, v: usize) {
    g.remove(&v);
    for nb in g.values_mut() {
        nb.remove(&v);
    }
}

fn remove_vertex_and_neighbors(g: &mut Graph, v: usize) {
    let neighbors = g.get(&v).unwrap().clone();
    remove_vertex(g, v);
    for nb in neighbors {
        remove_vertex(g, nb);
    }
}

fn maximum_independent_set_approx(g: &Graph) -> HashSet<usize> {
    if g.is_empty() {
        HashSet::new()
    } else {
        let v = min_degree_vertex(g);
        let mut g_red = g.clone();
        remove_vertex_and_neighbors(&mut g_red, v);
        let mut res = HashSet::new();
        res.insert(v);
        res.extend(maximum_independent_set_approx(&g_red));
        res
    }
}

fn min_degree_vertex(g: &Graph) -> usize {
    *g.iter()
        .min_by(|(_, nbs1), (_, nbs2)| nbs1.len().cmp(&nbs2.len()))
        .unwrap()
        .0
}

fn main() {
    let f = vec![
        HashSet::from([1, 2, 3]),
        HashSet::from([1, 7, 5]),
        HashSet::from([6, 9]),
        HashSet::from([5, 8, 4]),
    ];
    let family = max_disjoint_subset(&f);
    let graph = create_graph(&family);

    println!("vertex:{}", min_degree_vertex(&graph));
    println!("set approx:{:#?}", maximum_independent_set_approx(&graph));
}
