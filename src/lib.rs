use std::string::FromUtf8Error;
use std::result::{Result as StdResult};
use encoding_rs::{Encoding};
use jmespath::{compile, Variable};
use worker::{event, Router, Request, Env, Context, Result, Fetch, Url, Response, RouteContext, ResponseBuilder};


const BAD_REQUEST: u16 = 400;
const INTERNAL_SERVER_ERROR: u16 = 500;

macro_rules! resp_try {
    ($expr:expr, $status:expr, $tag:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return Ok(resp_err!(e, $status, $tag)),
        }
    }
}
macro_rules! resp_err {
    ($err:expr, $status:expr, $tag:expr) => {
        ResponseBuilder::new()
            .with_status($status)
            .fixed(format!("{}\n{}", $tag, $err).into_bytes())
    }
}


#[event(fetch)]
async fn fetch(
    req: Request,
    env: Env,
    _ctx: Context,
) -> Result<Response> {
    Router::new()
        .get_async("/", root)
        .get_async("/raw/:input/s/*url", raw)
        .get_async("/jp/:input/s/*url", jmespath)
        .get_async("/jmespath/:input/s/*url", jmespath)
        .run(req, env)
        .await
}


pub async fn root(_: Request, _: RouteContext<()>) -> Result<Response> {
    build_response("dataquery")
}

fn build_response(body: impl Into<String>) -> Result<Response> {
    let mut res = Response::ok(body)?;
    let h = res.headers_mut();
    h.set("Content-Type", "text/plain; charset=UTF-8")?;
    h.set("Cache-Control", "max-age=60")?;
    Ok(res)
}

/// /raw/{input}/s/{url}
pub async fn raw(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let input = ctx.param("input")
        .map(|v| urlencoding::decode(v).ok()).flatten()
        .map(String::from).unwrap_or_default();
    let url = ctx.param("url")
        .map(|v| urlencoding::decode(v).ok()).flatten()
        .map(String::from).unwrap_or_default();
    let query = req.url()?.query().map(String::from);

    // url
    let full_url = build_full_url(url, query);

    // fetch
    let text = match fetch_url(&full_url).await {
        Ok(v) => v,
        Err(e) => return Ok(e),
    };

    build_response(format!("{input}\n{full_url}\n{text}"))
}

/// /jp/{input}/s/{url}
pub async fn jmespath(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let input = ctx.param("input")
        .map(|v| urlencoding::decode(v).ok()).flatten()
        .map(String::from).unwrap_or_default();
    let url = ctx.param("url")
        .map(|v| urlencoding::decode(v).ok()).flatten()
        .map(String::from).unwrap_or_default();
    let query = req.url()?.query().map(String::from);

    // url
    let full_url = build_full_url(url, query);

    // fetch
    let text = match fetch_url(&full_url).await {
        Ok(v) => v,
        Err(e) => return Ok(e),
    };

    // json
    let data = resp_try!(
        Variable::from_json(&text),
        INTERNAL_SERVER_ERROR, "ERROR[JSON];");

    // jmespath
    let expr = resp_try!(
        compile(&input),
        INTERNAL_SERVER_ERROR, "ERROR[COMPILE];");
    let result = resp_try!(
        expr.search(data).map(|v| v.to_string()),
        INTERNAL_SERVER_ERROR, "ERROR[SEARCH];");

    build_response(result)
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

/// charset from Content-Type
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

/// decode with charset, or UTF-8
fn decode_bytes_with_charset(bytes: &[u8], charset_label: Option<&str>) -> StdResult<String,FromUtf8Error> {
    if let Some(label) = charset_label {
        if let Some(enc) = Encoding::for_label(label.as_bytes()) {
            let (cow, _, _) = enc.decode(bytes);
            return Ok(cow.into_owned());
        }
    }
    String::from_utf8(bytes.to_vec())
}

async fn fetch_url(url: &String) -> StdResult<String,Response> {
    let parsed = Url::parse(url)
        .map_err(|e| resp_err!(e, BAD_REQUEST, "ERROR[PARSE];"))?;

    let mut resp = Fetch::Url(parsed).send().await
        .map_err(|e| resp_err!(e, INTERNAL_SERVER_ERROR, "ERROR[SEND];"))?;

    let body = resp.bytes().await
        .map_err(|e| resp_err!(e, INTERNAL_SERVER_ERROR, "ERROR[BYTES];"))?;

    let charset = match resp.headers().get("content-type") {
        Ok(Some(v)) => extract_charset(&v),
        _ => None,
    };

    let text = decode_bytes_with_charset(&body, charset.as_deref())
        .map_err(|e| resp_err!(e, INTERNAL_SERVER_ERROR, "ERROR[DECODE];"))?;

    Ok(text)
}