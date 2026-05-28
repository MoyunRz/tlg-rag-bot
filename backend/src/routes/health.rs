use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

pub async fn get_health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "tlg-rag-bot-backend",
    })
}
