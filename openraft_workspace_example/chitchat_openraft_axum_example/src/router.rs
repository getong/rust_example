use aide::axum::ApiRouter;

use crate::{
  api::{AppState, get_members_docs, get_state_docs, update_service_docs},
  docs::docs_routes,
};

pub fn create_router() -> ApiRouter<AppState> {
  ApiRouter::new()
    .api_route("/", get_state_docs().into())
    .api_route("/members", get_members_docs().into())
    .api_route("/update_service", update_service_docs().into())
    .merge(docs_routes())
}

// async fn serve_docs(Extension(api): Extension<OpenApi>) -> impl IntoApiResponse {
//   Json(api)
// }
