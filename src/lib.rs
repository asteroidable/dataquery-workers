use axum::{routing::get, Router};
use axum::{extract::Path, http::StatusCode};
use axum::extract::RawQuery;
use encoding_rs::{Encoding};
use jmespath::{compile, Variable};
use tower_service::Service;
use worker::{event, HttpRequest, Env, Context, Result as WorkerResult, Fetch, Url};

use tokio::sync::oneshot;

// --- spawn_local 을 타겟별로 alias (핸들러 내부 로직은 동일) ---
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local as spawn_local_task;

#[cfg(not(target_arch = "wasm32"))]
use tokio::task::spawn_local as spawn_local_task;
// ------------------------------------------------------------

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    _env: Env,
    _ctx: Context,
) -> WorkerResult<axum::http::Response<axum::body::Body>> {
    Ok(router().call(req).await?)
}


fn router() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/raw/{input}/s/{*url}", get(raw))
        .route("/jp/{input}/s/{*url}", get(jmespath))
}


pub async fn root() -> &'static str {
    "dataquery"
}

/// /raw/{input}/s/{url}
pub async fn raw(Path((input, url)): Path<(String, String)>, RawQuery(query): RawQuery) -> String {
    let full_url = build_full_url(url, query);
    format!("{input}\n{full_url}")
}

/// /jp/{input}/s/{url}
pub async fn jmespath(Path((input, url)): Path<(String, String)>, RawQuery(query): RawQuery) -> Result<String, (StatusCode, String)> {
    // oneshot 채널로 결과만 전달받기 → 핸들러 future 는 Send
    let (tx, rx) = oneshot::channel();

    // non-Send 연산은 로컬 태스크에서 수행
    spawn_local_task(async move {
        let result = async {
            let full_url = build_full_url(url, query);
            let url = Url::parse(&full_url)
                .map_err(|e| err_bad_request(format!("ERROR[PARSE]; {e}")))?;

            // 1) 바이트로 받기
            let mut resp = Fetch::Url(url)
                .send()
                .await
                .map_err(|e| err_internal_server(format!("ERROR[SEND]; {e}")))?;

            let body = resp.bytes()
                .await
                .map_err(|e| err_internal_server(format!("ERROR[BYTES]; {e}")))?;

            // 2) Content-Type 헤더에서 charset 추출 (있으면 우선 적용)
            let charset = resp.headers()
                .get("content-type")
                .ok()
                .flatten()
                .and_then(|ct| extract_charset(&ct));

            // 3) 디코딩
            let text = decode_bytes_with_charset(&body, charset.as_deref())
                .map_err(|e| err_internal_server(format!("ERROR[DECODE]; {e}")))?;
            Ok(text)
        }.await;

        // 실패하더라도 send 에러는 무시(수신측에서 처리)
        let _ = tx.send(result);
    });

    // 여기서 기다리는 건 oneshot Receiver 이므로 Send
    let source = rx.await
        .map_err(|e| err_internal_server(format!("ERROR[RX]; {e}")))
        .flatten()?;

    let data = Variable::from_json(&source)
        .map_err(|e| err_internal_server(format!("ERROR[JSON]; {e}")))?;

    // compile expr
    let expr = compile(&input).map_err(|e| err_bad_request(format!("ERROR[COMPILE]; {e}")))?;

    let result = expr.search(data)
        .map_err(|e| err_internal_server(format!("ERROR[SEARCH]; {e}")))?;

    Ok(result.to_string())
}

fn err_bad_request(msg: String) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg)
}
fn err_internal_server(msg: String) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, msg)
}


fn build_full_url(url: String, query: Option<String>) -> String {
    match query {
        Some(q) if !q.is_empty() => {
            let sep = if url.contains('?') { '&' } else { '?' };
            format!("{url}{sep}{q}")
        }
        _ => url,
    }
}

/// Content-Type에서 charset=... 추출 (대/소문자 및 따옴표 허용)
fn extract_charset(ct: &str) -> Option<String> {
    ct.split(';')
        .skip(1)
        .map(|p| p.trim())
        .find_map(|p| {
            let p = p.strip_prefix("charset=").or_else(|| {
                let lower = p.to_lowercase();
                lower.strip_prefix("charset=").map(|_| &p[8..])
            })?;
            Some(p.trim_matches('"').trim().to_string())
        })
}

/// charset 힌트가 있으면 해당 인코딩, 없거나 실패하면 UTF-8 시도
fn decode_bytes_with_charset(bytes: &[u8], charset_label: Option<&str>) -> Result<String,String> {
    // charset 힌트가 있으면 해당 인코딩으로 시도
    if let Some(label) = charset_label {
        if let Some(enc) = Encoding::for_label(label.as_bytes()) {
            let (cow, _, _) = enc.decode(bytes);
            return Ok(cow.into_owned());
        }
    }
    // 우선 UTF-8 디코드 시도
    String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string())
}
