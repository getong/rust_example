mod migrations;

use cot::{
  App, AppBuilder, Project, Template,
  auth::db::DatabaseUserApp,
  cli::CliMetadata,
  db::migrations::SyncDynMigration,
  html::Html,
  middleware::{AuthMiddleware, LiveReloadMiddleware, SessionMiddleware},
  project::{MiddlewareContext, RegisterAppsContext, RootHandler, RootHandlerBuilder},
  request::extractors::StaticFiles,
  router::{Route, Router},
  session::db::SessionApp,
  static_files,
  static_files::{StaticFile, StaticFilesMiddleware},
};

#[derive(Debug, Template)]
#[template(path = "index.html")]
struct IndexTemplate {
  static_files: StaticFiles,
}

async fn index(static_files: StaticFiles) -> cot::Result<Html> {
  let index_template = IndexTemplate { static_files };
  let rendered = index_template.render()?;

  Ok(Html::new(rendered))
}

struct CotExampleApp;

impl App for CotExampleApp {
  fn name(&self) -> &'static str {
    env!("CARGO_CRATE_NAME")
  }

  fn migrations(&self) -> Vec<Box<SyncDynMigration>> {
    cot::db::migrations::wrap_migrations(migrations::MIGRATIONS)
  }

  fn router(&self) -> Router {
    Router::with_urls([Route::with_handler_and_name("/", index, "index")])
  }

  fn static_files(&self) -> Vec<StaticFile> {
    static_files!("css/main.css")
  }
}

struct CotExampleProject;

impl Project for CotExampleProject {
  fn cli_metadata(&self) -> CliMetadata {
    cot::cli::metadata!()
  }

  fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
    apps.register_with_views(CotExampleApp, "");
    apps.register(DatabaseUserApp::new());
    apps.register(SessionApp::new());
  }

  fn middlewares(&self, handler: RootHandlerBuilder, context: &MiddlewareContext) -> RootHandler {
    handler
      .middleware(StaticFilesMiddleware::from_context(context))
      .middleware(AuthMiddleware::new())
      .middleware(SessionMiddleware::from_context(context))
      .middleware(LiveReloadMiddleware::from_context(context))
      .build()
  }
}

#[cot::main]
fn main() -> impl Project {
  CotExampleProject
}
