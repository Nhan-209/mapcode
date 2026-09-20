// tests/fixtures/sample_workspace/src/api/routes.rs

use crate::service::user_service::UserService;
use axum::{routing::get, Json, Router};

pub fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/users", get(get_users).post(create_user))
}

pub async fn health_check() -> &'static str {
    "OK"
}

pub async fn get_users() -> Json<Vec<String>> {
    let service = UserService::new();
    let names = service.get_all().into_iter().map(|u| u.username).collect();
    Json(names)
}

pub async fn create_user() -> Json<String> {
    let mut service = UserService::new();
    let user = service.create_user("charlie", "charlie@example.com");
    Json(user.username)
}
