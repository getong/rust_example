use std::fmt::Debug;

use graphql_client::GraphQLQuery;
use serde_json::Value;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use yew::{
  html,
  Component, Context, Html,
};
use web_sys::{Request, RequestInit, RequestMode, Response};

#[derive(Debug, Clone, PartialEq)]
pub struct FetchError {
  err: JsValue,
}

impl From<JsValue> for FetchError {
  fn from(value: JsValue) -> Self {
    Self { err: value }
  }
}

#[derive(GraphQLQuery)]
#[graphql(
  schema_path = "./graphql/schema.graphql",
  query_path = "./graphql/all_projects.graphql",
  response_derives = "Debug"
)]
struct AllProjects;
type ObjectId = String;

async fn fetch_projects() -> Result<Vec<Value>, FetchError> {
  let build_query = AllProjects::build_query(all_projects::Variables {});
  let query = serde_json::json!(build_query);

  let mut opts = RequestInit::new();
  opts.method("POST");
  opts.body(Some(&JsValue::from_str(&query.to_string())));
  opts.mode(RequestMode::Cors); // 可以不写，默认为 Cors

  let gql_uri = "http://127.0.0.1:8000/graphql";
  let request = Request::new_with_str_and_init(gql_uri, &opts)?;

  let window = web_sys::window().expect("window is required for fetch");
  let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
  let resp: Response = resp_value.dyn_into().unwrap();
  let resp_text = JsFuture::from(resp.text()?).await?;

  let projects_str = resp_text.as_string().unwrap();
  let projects_value: Value = serde_json::from_str(&projects_str).unwrap();
  let projects_vec = projects_value["data"]["allProjects"]
    .as_array()
    .unwrap()
    .to_owned();

  Ok(projects_vec)
}

pub struct Projects {
  list: Vec<Value>,
}

pub enum Msg {
  UpdateList(Vec<Value>),
}

impl Component for Projects {
  type Message = Msg;
  type Properties = ();

  fn create(_: &Context<Self>) -> Self {
    Self {
      list: Vec::new(),
    }
  }

  fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
    if first_render {
      let link = ctx.link().clone();
      spawn_local(async move {
        let res = fetch_projects().await;
        link.send_message(Msg::UpdateList(res.unwrap()))
      });
    }
  }

  fn update(&mut self, _: &Context<Self>, msg: Self::Message) -> bool {
    match msg {
      Msg::UpdateList(res) => {
        self.list = res;
        true
      }
    }
  }

  fn view(&self, _: &Context<Self>) -> Html {
    let projects = self.list.iter().map(|project| {
      html! {
          <div>
              <li>
                  <strong>{ &project["subject"].as_str().unwrap() }</strong>
              </li>
              <ul>
                  <li>{ &project["userId"].as_str().unwrap() }</li>
                  <li>{ &project["id"].as_str().unwrap() }</li>
                  <li>
                      <a href={ project["website"].as_str().unwrap().to_owned() }>
                          { &project["website"].as_str().unwrap() }
                      </a>
                  </li>
              </ul>
          </div>
      }
    });

    html! {
        <>
            <h1>{ "all projects" }</h1>
            <ul>
                { for projects }
            </ul>
        </>
    }
  }
}
