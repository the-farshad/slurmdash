//! Pending-reason explainer.
//!
//! Slurm reports a short reason code for pending jobs (e.g. `Resources`,
//! `Priority`, `Dependency`). This module maps the most common ones to a
//! one-line plain-language explanation and an optional suggestion.

#[derive(Debug, Clone)]
pub struct ReasonExplanation {
    pub code: String,
    pub summary: &'static str,
    pub suggestion: Option<&'static str>,
}

pub fn explain(reason: &str) -> ReasonExplanation {
    let code = reason.trim();
    let (summary, suggestion): (&'static str, Option<&'static str>) = match code {
        "Resources" => (
            "The job is valid but waiting for CPUs/GPUs/nodes to free up.",
            Some("Check `slurmdash trends` for partition pressure."),
        ),
        "Priority" => (
            "Other jobs currently have higher scheduling priority.",
            None,
        ),
        "Dependency" => (
            "The job is waiting for another job to finish.",
            Some("Inspect the `Dependency=` field in job details."),
        ),
        "JobHeldUser" => (
            "The job was held by the user.",
            Some("Release with `slurmdash release <id>`."),
        ),
        "JobHeldAdmin" => (
            "The job was held by an administrator.",
            Some("Contact the cluster operator."),
        ),
        "QOSMaxWallDurationPerJobLimit" => (
            "The requested wall time exceeds the QoS limit.",
            Some("Lower `--time` or choose a different QoS."),
        ),
        "AssocGrpGRES" | "AssocGrpGPULimit" => (
            "Your account/group has reached a GPU/GRES limit.",
            Some("Reduce GPU count or wait for in-flight jobs to finish."),
        ),
        "ReqNodeNotAvail" => (
            "Requested nodes are unavailable, down, drained, or reserved.",
            Some("Inspect `sinfo` for node state."),
        ),
        "BeginTime" => (
            "Held until a scheduled begin time.",
            None,
        ),
        "Licenses" => (
            "Waiting for a license to free up.",
            None,
        ),
        "ReqNodeUnavail" => (
            "A requested node is currently unavailable.",
            None,
        ),
        _ => ("No human-readable explanation registered for this reason.", None),
    };

    ReasonExplanation {
        code: code.to_string(),
        summary,
        suggestion,
    }
}
