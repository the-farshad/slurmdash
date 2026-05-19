//! Token check used by every API handler.
//!
//! Accepts the token in either:
//! - the `token` query string parameter (used by the index page), or
//! - the `Authorization: Bearer <token>` header.
//!
//! The browser stashes the token in a cookie after the first page load, so
//! subsequent fetches send it back automatically via that header (the
//! embedded `app.js` adds it). Stricter cross-site protections aren't
//! needed for a loopback-bound server, but if `--host` was overridden away
//! from localhost we still gate every endpoint on the token.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use std::sync::Arc;

use crate::web::state::WebState;

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

pub fn check(state: &WebState, headers: &HeaderMap, query: Option<&str>) -> Result<(), StatusCode> {
    if let Some(t) = query {
        if t == state.token {
            return Ok(());
        }
    }
    if let Some(h) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = h.to_str() {
            if let Some(rest) = s.strip_prefix("Bearer ") {
                if rest == state.token {
                    return Ok(());
                }
            }
        }
    }
    if let Some(c) = headers.get(axum::http::header::COOKIE) {
        if let Ok(s) = c.to_str() {
            for part in s.split(';') {
                let trimmed = part.trim();
                if let Some(v) = trimmed.strip_prefix("slurmdash_token=") {
                    if v == state.token {
                        return Ok(());
                    }
                }
            }
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

/// Helper for handlers: pull token from query+headers, return UNAUTHORIZED
/// if it doesn't match.
pub fn require(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    query: Option<Query<TokenQuery>>,
) -> Result<Arc<WebState>, StatusCode> {
    let q = query.as_ref().and_then(|q| q.0.token.as_deref());
    check(&state, &headers, q)?;
    Ok(state)
}
