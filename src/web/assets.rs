//! Static assets for the embedded web dashboard. Bundled at compile time
//! with `include_str!` so the binary stays single-file.

use axum::http::header;
use axum::response::IntoResponse;

const INDEX_HTML: &str = include_str!("../../assets/web/index.html");
const STYLE_CSS: &str = include_str!("../../assets/web/style.css");
const APP_JS: &str = include_str!("../../assets/web/app.js");

pub async fn index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        INDEX_HTML,
    )
}

pub async fn style() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        STYLE_CSS,
    )
}

pub async fn app_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        APP_JS,
    )
}
