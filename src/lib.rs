use axum::{routing::get, Router};
use axum::{extract::Path, http::Uri};
use tower_service::Service;
use worker::*;


#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    _env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    Ok(router().call(req).await?)
}


fn router() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/jp/{input}/s/{*url}", get(jp))
}


pub async fn root() -> &'static str {
    "dataquery"
}

/// /jp/{input}/s/{url}
pub async fn jp(Path((input, url)): Path<(String, String)>, uri: Uri) -> String {
    let full_url = match uri.query() {
        Some(q) if !q.is_empty() => {
            let sep = if url.contains('?') { '&' } else { '?' };
            format!("{url}{sep}{q}")
        },
        _ => url,
    };
    format!("{input}\n{full_url}")
}
