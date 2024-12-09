use axum::{Router, routing::get};
use database::MemoryDB;
use handlebars::Handlebars;

use crate::build_templates;

#[derive(Clone)]
pub struct AppState {
  pub templates: Handlebars<'static>,
  pub db: MemoryDB,
}

pub fn build_router() -> Router {
  let state = AppState {
    templates: build_templates(),
    db: MemoryDB::new(),
  };

  Router::new()
    .route("/", get(handlers::index))
    .route("/item/:id", get(handlers::show))
    .with_state(state)
}

mod handlers {
  use axum::{
    extract::{Path, State},
    response::Html,
  };
  use serde::Serialize;
  use uuid::Uuid;

  use super::AppState;

  #[derive(Serialize)]
  struct ItemViewModel<'a> {
    pub name: &'a str,
    pub uuid: &'a Uuid,
  }

  pub async fn index(State(AppState { db, templates }): State<AppState>) -> Html<String> {
    #[derive(Serialize)]
    struct ViewModel<'a> {
      pub title: &'a str,
      pub items: Vec<ItemViewModel<'a>>,
    }

    let items = db
      .all_items()
      .await
      .into_iter()
      .map(|(uuid, name)| ItemViewModel { name, uuid })
      .collect();

    let view = ViewModel {
      title: "All Items",
      items,
    };

    Html(templates.render("index", &view).unwrap())
  }

  pub async fn show(
    Path(id): Path<Uuid>,
    State(AppState { db, templates }): State<AppState>,
  ) -> Html<String> {
    let item = db
      .get_item(&id)
      .await
      .map(|name| ItemViewModel { name, uuid: &id })
      .unwrap();

    Html(templates.render("show", &item).unwrap())
  }
}

mod database {
  use std::{collections::HashMap, sync::Arc};

  use uuid::{Uuid, uuid};

  #[derive(Clone)]
  pub struct MemoryDB {
    items: Arc<HashMap<Uuid, String>>,
  }

  impl MemoryDB {
    pub fn new() -> Self {
      let items = [
        (
          uuid!("fd03f48c-af4f-4485-8a56-03e5354277ce"),
          "Apple Pie".to_owned(),
        ),
        (
          uuid!("deba1d8c-81fd-4273-9fcd-f4c5b5666fe2"),
          "Marshmallow".to_owned(),
        ),
        (
          uuid!("29cf7887-d228-41ca-883c-516cf3105634"),
          "Eclair au chocolat".to_owned(),
        ),
        (
          uuid!("9103a2b0-af58-4db5-a9a8-cbdd7274e15a"),
          "Merveilleux".to_owned(),
        ),
      ];

      Self {
        items: Arc::new(items.into_iter().collect()),
      }
    }

    pub async fn all_items(&self) -> Vec<(&Uuid, &String)> {
      self.items.iter().collect()
    }

    pub async fn get_item(&self, item_id: &Uuid) -> Option<&String> {
      self.items.get(item_id)
    }
  }
}
