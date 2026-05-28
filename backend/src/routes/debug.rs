use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::{models::kb::RagAnswer, state::AppState};

#[derive(Deserialize)]
pub struct DebugQueryPayload {
    question: String,
}

pub async fn run_debug_query(
    State(state): State<AppState>,
    Json(payload): Json<DebugQueryPayload>,
) -> Result<Json<RagAnswer>, ApiError> {
    let question = payload.question.trim();
    if question.is_empty() {
        return Err(ApiError::bad_request("question cannot be empty"));
    }

    let answer = state.rag.answer_question(question).await.map_err(|error| {
        tracing::error!(question = %question, error = ?error, "debug RAG query failed");
        ApiError::internal(format!("failed to answer question: {error}"))
    })?;

    Ok(Json(answer))
}

#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "status": "error",
                "message": self.message,
            })),
        )
            .into_response()
    }
}
