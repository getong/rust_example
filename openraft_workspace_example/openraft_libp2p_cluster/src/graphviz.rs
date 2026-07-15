use std::collections::{BTreeMap, BTreeSet};

use graphviz_rust::{cmd::Format, exec_dot};
use openraft::ServerState;
use petgraph::{
  dot::{Config, Dot},
  graph::DiGraph,
};

use crate::{GroupId, NodeId, typ::RaftMetrics};

#[derive(Debug, Clone)]
pub struct ClusterGraphNode {
  pub node_id: NodeId,
  pub peer_id: String,
  pub addr: String,
  pub connected: bool,
  /// Live raft server state of this node, PER GROUP (a node can be leader
  /// of one group and follower of another).
  pub server_states: BTreeMap<GroupId, ServerState>,
}

/// One raft group's view for the graph: its local metrics (membership,
/// leader) or the reason they are unavailable.
#[derive(Debug, Clone)]
pub struct ClusterGraphGroup {
  pub group_id: GroupId,
  pub metrics: Option<RaftMetrics>,
  pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClusterGraphSnapshot {
  pub self_node_id: NodeId,
  pub self_peer_id: String,
  pub self_listen: String,
  /// `Some` when the view is filtered to one group; `None` = all groups.
  pub selected: Option<GroupId>,
  /// Every group id this node serves (for the group picker).
  pub all_group_ids: Vec<GroupId>,
  /// The groups actually rendered (all of them, or the selected one).
  pub groups: Vec<ClusterGraphGroup>,
  pub nodes: Vec<ClusterGraphNode>,
  pub error: Option<String>,
}

/// Per-group edge/accent colors, assigned by group position so every raft
/// group's replication edges are visually distinct.
const GROUP_COLORS: [&str; 6] = [
  "#1d4ed8", // blue
  "#0f766e", // teal
  "#b45309", // amber
  "#be185d", // pink
  "#7c3aed", // violet
  "#334155", // slate
];

fn group_color(index: usize) -> &'static str {
  GROUP_COLORS[index % GROUP_COLORS.len()]
}

#[derive(Debug, Clone)]
struct GraphNodeLabel {
  title: String,
  lines: Vec<String>,
  fill_color: &'static str,
  border_color: &'static str,
  pen_width: &'static str,
}

#[derive(Debug, Clone)]
struct GraphEdgeLabel {
  label: String,
  color: &'static str,
  style: &'static str,
  pen_width: &'static str,
}

pub fn cluster_graph_dot(snapshot: &ClusterGraphSnapshot) -> String {
  let mut graph = DiGraph::<GraphNodeLabel, GraphEdgeLabel>::new();
  let mut indices = BTreeMap::new();
  let mut known_node_ids = BTreeSet::new();

  let mut nodes = snapshot.nodes.clone();
  nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));

  // Role of every node in EVERY rendered group: a node can be leader of one
  // group and follower/learner of another, so attribution is per group.
  let roles_by_group: Vec<(GroupId, BTreeMap<NodeId, String>)> = snapshot
    .groups
    .iter()
    .map(|group| {
      (
        group.group_id.clone(),
        group_roles(&group.group_id, group.metrics.as_ref(), &nodes),
      )
    })
    .collect();
  let leads_any_group = |node_id: &NodeId| {
    snapshot.groups.iter().any(|group| {
      group
        .metrics
        .as_ref()
        .is_some_and(|metrics| metrics.current_leader.as_ref() == Some(node_id))
    })
  };

  for node in &nodes {
    known_node_ids.insert(node.node_id.clone());
    let is_leader = leads_any_group(&node.node_id);
    let is_self = node.node_id == snapshot.self_node_id;
    let title = short_peer(&node.peer_id);

    // One role line per group this node belongs to.
    let mut member_of_any = false;
    let mut lines = Vec::new();
    let mut strongest_role = "discovered";
    for (group_id, roles) in &roles_by_group {
      if let Some(role) = roles.get(&node.node_id) {
        member_of_any = true;
        lines.push(format!("{group_id}: {role}"));
        strongest_role = strongest_of(strongest_role, role);
      }
    }
    if !member_of_any {
      lines.push("role: discovered".to_string());
    }
    lines.push(format!("peer_id: {}", short_peer(&node.peer_id)));
    lines.push(format!("addr: {}", compact_addr(&node.addr)));
    if is_self {
      lines.push("local HTTP view".to_string());
      for group in &snapshot.groups {
        if let Some(metrics) = group.metrics.as_ref() {
          lines.push(format!(
            "{}: term {}, last log {}",
            group.group_id,
            metrics.current_term,
            display_option(metrics.last_log_index)
          ));
        }
      }
    }

    let index = graph.add_node(GraphNodeLabel {
      title,
      lines,
      fill_color: node_fill_color(strongest_role, is_leader, is_self),
      border_color: node_border_color(strongest_role, node.connected),
      pen_width: if is_leader || is_self { "2.4" } else { "1.5" },
    });
    indices.insert(node.node_id.clone(), index);
  }

  if !known_node_ids.contains(&snapshot.self_node_id) {
    let index = graph.add_node(GraphNodeLabel {
      title: short_peer(&snapshot.self_peer_id),
      lines: vec![
        "role: local".to_string(),
        format!("peer_id: {}", short_peer(&snapshot.self_peer_id)),
        format!("addr: {}", compact_addr(&snapshot.self_listen)),
      ],
      fill_color: "#e8f7ff",
      border_color: "#2563eb",
      pen_width: "2.4",
    });
    indices.insert(snapshot.self_node_id.clone(), index);
  }

  for node in &nodes {
    if node.node_id == snapshot.self_node_id {
      continue;
    }
    let Some(source) = indices.get(&snapshot.self_node_id).copied() else {
      continue;
    };
    let Some(target) = indices.get(&node.node_id).copied() else {
      continue;
    };
    graph.add_edge(
      source,
      target,
      GraphEdgeLabel {
        label: if node.connected {
          "libp2p connected".to_string()
        } else {
          "libp2p known".to_string()
        },
        color: if node.connected { "#0f766e" } else { "#94a3b8" },
        style: if node.connected { "solid" } else { "dashed" },
        pen_width: if node.connected { "1.8" } else { "1.2" },
      },
    );
  }

  // Replication edges of every rendered group, color-coded per group.
  for (group_index, group) in snapshot.groups.iter().enumerate() {
    let Some(metrics) = group.metrics.as_ref() else {
      continue;
    };
    let color = group_color(group_index);
    let membership = metrics.membership_config.membership();
    let voters = membership.voter_ids().collect::<BTreeSet<_>>();
    let learners = membership.learner_ids().collect::<BTreeSet<_>>();

    if let Some(leader) = metrics.current_leader.as_ref() {
      for voter in &voters {
        if voter == leader {
          continue;
        }
        add_openraft_edge(
          &mut graph,
          &indices,
          leader,
          voter,
          &format!("{}: replicate follower", group.group_id),
          color,
          "solid",
        );
      }
      for learner in &learners {
        add_openraft_edge(
          &mut graph,
          &indices,
          leader,
          learner,
          &format!("{}: replicate learner", group.group_id),
          color,
          "dotted",
        );
      }
    }
  }

  let dot = Dot::with_attr_getters(
    &graph,
    &[Config::EdgeNoLabel, Config::NodeNoLabel],
    &|_, edge| {
      let weight = edge.weight();
      format!(
        "label=\"{}\", color=\"{}\", fontcolor=\"{}\", style=\"{}\", penwidth=\"{}\", \
         arrowsize=\"0.8\"",
        dot_escape(&weight.label),
        weight.color,
        weight.color,
        weight.style,
        weight.pen_width,
      )
    },
    &|_, (_, weight)| {
      format!(
        "label=<{}>, shape=\"box\", style=\"rounded,filled\", fillcolor=\"{}\", color=\"{}\", \
         penwidth=\"{}\", fontname=\"Helvetica\", fontsize=\"11\", margin=\"0.12,0.08\"",
        html_label(weight),
        weight.fill_color,
        weight.border_color,
        weight.pen_width,
      )
    },
  );

  with_graph_attributes(&format!("{dot:?}"), snapshot)
}

pub fn cluster_graph_svg(snapshot: &ClusterGraphSnapshot) -> std::io::Result<Vec<u8>> {
  exec_dot(cluster_graph_dot(snapshot), vec![Format::Svg.into()])
}

fn add_openraft_edge(
  graph: &mut DiGraph<GraphNodeLabel, GraphEdgeLabel>,
  indices: &BTreeMap<NodeId, petgraph::graph::NodeIndex>,
  source_id: &NodeId,
  target_id: &NodeId,
  label: &str,
  color: &'static str,
  style: &'static str,
) {
  let Some(source) = indices.get(source_id).copied() else {
    return;
  };
  let Some(target) = indices.get(target_id).copied() else {
    return;
  };
  graph.add_edge(
    source,
    target,
    GraphEdgeLabel {
      label: label.to_string(),
      color,
      style,
      pen_width: "2.2",
    },
  );
}

/// Roles of every member of ONE group, derived from the group's local
/// membership view and refined with the members' live server states (which
/// can reveal e.g. a candidate mid-election).
fn group_roles(
  group_id: &GroupId,
  metrics: Option<&RaftMetrics>,
  nodes: &[ClusterGraphNode],
) -> BTreeMap<NodeId, String> {
  let mut roles = BTreeMap::new();
  if let Some(metrics) = metrics {
    let membership = metrics.membership_config.membership();
    for node_id in membership.voter_ids() {
      roles.insert(node_id, "follower".to_string());
    }
    for node_id in membership.learner_ids() {
      roles.insert(node_id, "learner".to_string());
    }
    if let Some(leader_id) = metrics.current_leader.as_ref() {
      roles.insert(leader_id.clone(), "leader".to_string());
    }
  }

  for node in nodes {
    if let Some(state) = node.server_states.get(group_id) {
      // Only refine members of this group; a live state for a node outside
      // the membership view would otherwise conjure a phantom member.
      if roles.contains_key(&node.node_id) {
        roles.insert(node.node_id.clone(), server_state_label(*state).to_string());
      }
    }
  }

  roles
}

/// For node coloring, pick the most significant role a node holds across
/// groups: leader > candidate > follower > learner > everything else.
fn strongest_of(current: &'static str, candidate: &str) -> &'static str {
  fn rank(role: &str) -> u8 {
    match role {
      "leader" => 4,
      "candidate" => 3,
      "follower" => 2,
      "learner" => 1,
      _ => 0,
    }
  }
  let interned = match candidate {
    "leader" => "leader",
    "candidate" => "candidate",
    "follower" => "follower",
    "learner" => "learner",
    "shutdown" => "shutdown",
    _ => "discovered",
  };
  if rank(interned) > rank(current) {
    interned
  } else {
    current
  }
}

fn server_state_label(state: ServerState) -> &'static str {
  match state {
    ServerState::Learner => "learner",
    ServerState::Follower => "follower",
    ServerState::Candidate => "candidate",
    ServerState::Leader => "leader",
    ServerState::Shutdown => "shutdown",
  }
}

fn node_fill_color(role: &str, is_leader: bool, is_self: bool) -> &'static str {
  if is_leader {
    return "#dcfce7";
  }
  if is_self {
    return "#e8f7ff";
  }
  match role {
    "follower" => "#dbeafe",
    "learner" => "#f3e8ff",
    "candidate" => "#fef9c3",
    "shutdown" => "#e2e8f0",
    _ => "#f8fafc",
  }
}

fn node_border_color(role: &str, connected: bool) -> &'static str {
  if !connected {
    return "#94a3b8";
  }
  match role {
    "leader" => "#15803d",
    "follower" => "#1d4ed8",
    "learner" => "#7c3aed",
    "candidate" => "#ca8a04",
    "shutdown" => "#64748b",
    _ => "#475569",
  }
}

fn html_label(label: &GraphNodeLabel) -> String {
  let mut html = String::from("<TABLE BORDER=\"0\" CELLBORDER=\"0\" CELLSPACING=\"0\">");
  html.push_str(&format!(
    "<TR><TD><B>{}</B></TD></TR>",
    html_escape(&label.title)
  ));
  for line in &label.lines {
    html.push_str(&format!(
      "<TR><TD ALIGN=\"LEFT\"><FONT POINT-SIZE=\"9\">{}</FONT></TD></TR>",
      html_escape(line)
    ));
  }
  html.push_str("</TABLE>");
  html
}

fn with_graph_attributes(dot: &str, snapshot: &ClusterGraphSnapshot) -> String {
  let scope = match snapshot.selected.as_ref() {
    Some(group_id) => format!("group: {group_id}"),
    None => format!("all {} groups", snapshot.groups.len()),
  };
  let leaders = snapshot
    .groups
    .iter()
    .map(|group| {
      let leader = group
        .metrics
        .as_ref()
        .and_then(|metrics| metrics.current_leader.as_ref())
        .map(|leader| short_text(leader.as_str(), 8, 4))
        .unwrap_or_else(|| "?".to_string());
      format!("{}\u{2192}{}", group.group_id, leader)
    })
    .collect::<Vec<_>>()
    .join(", ");
  let label = format!(
    "libp2p / openraft cluster\\n{} | local peer_id: {} | leaders: {}",
    dot_escape(&scope),
    dot_escape(&snapshot.self_peer_id),
    dot_escape(&leaders)
  );
  let attrs = format!(
    "digraph {{\n  graph [rankdir=\"LR\", bgcolor=\"transparent\", pad=\"0.35\", \
     nodesep=\"0.55\", ranksep=\"0.85\", splines=\"spline\", overlap=\"false\", label=\"{}\", \
     labelloc=\"t\", fontname=\"Helvetica\", fontsize=\"16\"];\n  node \
     [fontname=\"Helvetica\"];\n  edge [fontname=\"Helvetica\", fontsize=\"9\"];\n",
    label
  );
  dot.replacen("digraph {", &attrs, 1)
}

fn short_peer(peer_id: &str) -> String {
  short_text(peer_id, 10, 8)
}

fn short_text(value: &str, prefix_len: usize, suffix_len: usize) -> String {
  if value.len() <= prefix_len + suffix_len + 3 {
    return value.to_string();
  }
  format!(
    "{}...{}",
    &value[.. prefix_len.min(value.len())],
    &value[value.len().saturating_sub(suffix_len) ..]
  )
}

fn compact_addr(addr: &str) -> String {
  addr
    .replace("/ip4/", "")
    .replace("/tcp/", ":")
    .replace("/udp/", ":")
    .replace("/p2p/", "/p2p/")
}

fn display_option<T: std::fmt::Display>(value: Option<T>) -> String {
  value
    .map(|value| value.to_string())
    .unwrap_or_else(|| "none".to_string())
}

fn dot_escape(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn html_escape(value: &str) -> String {
  value
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
}
