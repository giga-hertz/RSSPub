use std::sync::Arc;
use axum::extract::State;
use axum::Json;
use axum::http::StatusCode;
use crate::{db, email};
use crate::models::{AppState, EmailConfig};

pub async fn get_email_config_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Option<EmailConfig>>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    let config = db::get_email_config(&db)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(config))
}

pub async fn update_email_config_handler(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<EmailConfig>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;

    payload.email_address = payload.email_address.trim().to_string();
    payload.to_email = payload.to_email.trim().to_string();

    // Basic string validation instead of lettre::Address parsing
    if payload.to_email.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Recipient email cannot be empty".to_string()));
    }

    payload.to_email = email::normalize_recipient_list(&payload.to_email, "to").map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid recipient email list".to_string(),
        )
    })?;

    db::save_email_config(&db, &payload)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}
