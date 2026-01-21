use actix_web::{web, HttpRequest, HttpResponse};
use tracing::warn;

/// API key stored in app data for middleware access
#[derive(Clone)]
pub struct ApiKey(pub Option<String>);

/// Check API key for protected routes.
/// Returns Ok(()) if authorized, or an Unauthorized response.
pub fn require_auth(req: &HttpRequest) -> Result<(), HttpResponse> {
    let api_key = req.app_data::<web::Data<ApiKey>>();
    match api_key {
        Some(data) => match &data.0 {
            Some(expected) => {
                let authorized = req
                    .headers()
                    .get("Authorization")
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v == format!("Bearer {}", expected))
                    .unwrap_or(false);
                if authorized {
                    Ok(())
                } else {
                    warn!("Unauthorized request to {}", req.path());
                    Err(HttpResponse::Unauthorized()
                        .json(serde_json::json!({"error": "Unauthorized"})))
                }
            }
            None => Ok(()), // No API key configured
        },
        None => Ok(()),
    }
}
