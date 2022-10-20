use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Response {
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum Element {
    Node(Node),
    Way(Way),
    Relation(Relation),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    #[serde(flatten)]
    pub info: ElementInfo,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Way {
    #[serde(flatten)]
    pub info: ElementInfo,
    pub nodes: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    #[serde(flatten)]
    pub info: ElementInfo,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementInfo {
    pub changeset: u64,
    pub id: i64,
    pub timestamp: String,
    pub uid: u64,
    pub user: String,
    pub version: u64,
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    #[serde(rename = "ref")]
    pub reference: i64,
    pub role: Option<String>,
    #[serde(rename = "type")]
    pub kind: MemberType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MemberType {
    Node,
    Way,
    Relation,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let response = reqwest::get("https://api.openstreetmap.org/api/0.6/map.json?bbox=17.030873894691467,51.110227939761934,17.03128159046173,51.110551258091014").await?;

    let json: Response = response.json().await?;

    println!("{json:#?}");

    Ok(())
}
