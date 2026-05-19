//! JSON API endpoints.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Serialize;

use crate::actions::ActionKind;
use crate::slurm::{model::JobDetails, scontrol};
use crate::web::auth::{TokenQuery, require};
use crate::web::state::{Snapshot, WebState};

#[derive(Serialize)]
pub struct DashboardResponse<'a> {
    pub snapshot: &'a Snapshot,
    pub cluster: ClusterInfo,
    pub readonly: bool,
}

#[derive(Serialize)]
pub struct ClusterInfo {
    pub name: String,
    pub is_local: bool,
}

pub async fn dashboard(
    state_in: State<Arc<WebState>>,
    headers: HeaderMap,
    query: Option<Query<TokenQuery>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let state = require(state_in, headers, query)?;
    let snap = state.snapshot.read().await;
    let payload = serde_json::json!({
        "snapshot": &*snap,
        "cluster": {
            "name": state.handle.cluster_name,
            "is_local": state.handle.is_local,
        },
        "readonly": state.readonly,
    });
    Ok(Json(payload))
}

pub async fn job_details(
    state_in: State<Arc<WebState>>,
    headers: HeaderMap,
    query: Option<Query<TokenQuery>>,
    Path(job_id): Path<String>,
) -> Result<Json<JobDetails>, StatusCode> {
    let state = require(state_in, headers, query)?;
    let runner = state.handle.runner.as_ref();
    scontrol::show(runner, &job_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn cancel(
    s: State<Arc<WebState>>,
    h: HeaderMap,
    q: Option<Query<TokenQuery>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    run_action(s, h, q, id, ActionKind::Cancel).await
}

pub async fn hold(
    s: State<Arc<WebState>>,
    h: HeaderMap,
    q: Option<Query<TokenQuery>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    run_action(s, h, q, id, ActionKind::Hold).await
}

pub async fn release(
    s: State<Arc<WebState>>,
    h: HeaderMap,
    q: Option<Query<TokenQuery>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    run_action(s, h, q, id, ActionKind::Release).await
}

pub async fn requeue(
    s: State<Arc<WebState>>,
    h: HeaderMap,
    q: Option<Query<TokenQuery>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    run_action(s, h, q, id, ActionKind::Requeue).await
}

async fn run_action(
    state_in: State<Arc<WebState>>,
    headers: HeaderMap,
    query: Option<Query<TokenQuery>>,
    job_id: String,
    kind: ActionKind,
) -> Result<StatusCode, (StatusCode, String)> {
    let state =
        require(state_in, headers, query).map_err(|s| (s, "unauthorized".to_string()))?;
    if state.readonly {
        return Err((StatusCode::FORBIDDEN, "server is readonly".to_string()));
    }
    let runner = state.handle.runner.as_ref();
    crate::actions::run(
        kind,
        &job_id,
        runner,
        state.db.as_ref(),
        &state.handle.cluster_name,
        state.handle.is_local,
        true,
    )
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("{e}")))?;
    Ok(StatusCode::NO_CONTENT)
}
