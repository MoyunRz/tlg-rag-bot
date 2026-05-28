use axum::{
    routing::{delete, get, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{routes, state::AppState};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .nest(
            "/api",
            Router::new()
                .route("/health", get(routes::health::get_health))
                .route("/kb/upload", post(routes::kb::upload_knowledge_base))
                .route("/kb/text", post(routes::kb::upload_text))
                .route("/kb/chunks", get(routes::kb::list_chunks))
                .route("/kb/chunks", delete(routes::kb::delete_chunks))
                .route("/kb/sources", get(routes::kb::list_sources))
                .route("/debug/query", post(routes::debug::run_debug_query)),
        )
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
