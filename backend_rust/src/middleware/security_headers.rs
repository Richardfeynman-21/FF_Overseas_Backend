use axum::{
    body::Body,
    http::Request,
    middleware::Next,
};

pub async fn security_headers(request: Request<Body>, next: Next) -> axum::response::Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // 1. HSTS (Strict-Transport-Security)
    headers.insert(
        "Strict-Transport-Security",
        "max-age=63072000; includeSubDomains; preload".parse().unwrap(),
    );

    // 2. CSP (Content-Security-Policy)
    headers.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self';".parse().unwrap(),
    );

    // 3. X-Frame-Options
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());

    // 4. X-Content-Type-Options
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());

    // 5. Referrer-Policy
    headers.insert(
        "Referrer-Policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );

    // 6. X-XSS-Protection
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());

    response
}
