//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-login-with-role
//! ```

use std::{collections::HashMap, sync::Arc};

use axum::{response::IntoResponse, routing::get, Extension, Router};
use axum_login::{
    axum_sessions::{async_session::MemoryStore as SessionMemoryStore, SessionLayer},
    extractors::AuthContext,
    memory_store::MemoryStore as AuthMemoryStore,
    secrecy::SecretVec,
    AuthLayer, AuthUser, RequireAuthorizationLayer,
};
use rand::Rng;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
struct User {
    id: usize,
    name: String,
    password_hash: String,
    role: Role,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
enum Role {
    // User,
    Admin,
}

impl User {
    fn get_rusty_user() -> Self {
        Self {
            id: 1,
            name: "Ferris the Crab".to_string(),
            password_hash: "password".to_string(),
            role: Role::Admin,
        }
    }
}

impl AuthUser<usize, Role> for User {
    fn get_id(&self) -> usize {
        self.id
    }

    fn get_password_hash(&self) -> SecretVec<u8> {
        SecretVec::new(self.password_hash.clone().into())
    }

    fn get_role(&self) -> Option<Role> {
        Some(self.role.clone())
    }
}

type Auth = AuthContext<usize, User, AuthMemoryStore<usize, User>, Role>;
type RequireAuth = RequireAuthorizationLayer<usize, User, Role>;

#[tokio::main]
async fn main() {
    let secret = rand::thread_rng().gen::<[u8; 64]>();

    let session_store = SessionMemoryStore::new();
    let session_layer = SessionLayer::new(session_store, &secret);

    let store = Arc::new(RwLock::new(HashMap::default()));
    let user = User::get_rusty_user();

    store.write().await.insert(user.get_id(), user);

    let user_store = AuthMemoryStore::new(&store);
    let auth_layer = AuthLayer::new(user_store, &secret);

    async fn login_handler(mut auth: Auth) {
        auth.login(&User::get_rusty_user()).await.unwrap();
    }

    async fn logout_handler(mut auth: Auth) {
        dbg!("Logging out user: {}", &auth.current_user);
        auth.logout().await;
    }

    async fn protected_handler(Extension(user): Extension<User>) -> impl IntoResponse {
        format!("Logged in as: {}", user.name)
    }

    async fn admin_handler(Extension(user): Extension<User>) -> impl IntoResponse {
        format!("Logged in as admin: {}", user.name)
    }

    let app = Router::new()
        .route("/admin", get(admin_handler))
        .route_layer(RequireAuth::login_with_role(Role::Admin..))
        .route("/", get(protected_handler))
        .route_layer(RequireAuth::login())
        .route("/login", get(login_handler))
        .route("/logout", get(logout_handler))
        .layer(auth_layer)
        .layer(session_layer);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
