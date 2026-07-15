use axum::http::{header, HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _request_parts| {
            if let Ok(origin_str) = origin.to_str() {
                // Allow official domains and local development
                if origin_str == "https://ffoverseas.in"
                    || origin_str == "https://www.ffoverseas.in"
                    || origin_str.starts_with("http://localhost:")
                    || origin_str.starts_with("http://127.0.0.1:")
                {
                    return true;
                }
                // Allow Vercel preview deployments (https://*.vercel.app)
                if origin_str.starts_with("https://") && origin_str.ends_with(".vercel.app") {
                    return true;
                }
            }
            false
        }))
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ])
        .allow_headers(vec![
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::ORIGIN,
            header::HeaderName::from_static("x-orbit-api-key"),
            header::HeaderName::from_static("x-request-id"),
        ])
        .allow_credentials(true)
}
