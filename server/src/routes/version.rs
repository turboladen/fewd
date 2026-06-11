use axum::Json;

use crate::dto::VersionInfo;

/// Report what build is running. The values are baked in at compile time by
/// `build.rs`, so this also answers "is prod running the latest deploy?" —
/// the diagnostic the Settings UI surfaces (fewd-0vp).
pub async fn version_info() -> Json<VersionInfo> {
    Json(VersionInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_sha: env!("FEWD_GIT_SHA").to_string(),
        built_at: env!("FEWD_BUILT_AT").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn version_info_reports_compile_time_metadata() {
        let axum::Json(info) = version_info().await;

        assert_eq!(
            info.version,
            env!("CARGO_PKG_VERSION"),
            "version should mirror the crate version"
        );
        assert!(
            !info.git_sha.is_empty(),
            "git_sha should be embedded at build time (or 'unknown' outside a git checkout)"
        );
        assert!(
            !info.built_at.is_empty(),
            "built_at should be embedded at build time"
        );
    }

    /// Pins the wire field names the TypeScript `VersionInfo` mirror depends on.
    #[test]
    fn version_info_serializes_with_stable_field_names() {
        let info = crate::dto::VersionInfo {
            version: "0.1.0".to_string(),
            git_sha: "abc1234".to_string(),
            built_at: "2026-06-11".to_string(),
        };
        let json = serde_json::to_string(&info).expect("serialize VersionInfo");
        for field in ["\"version\"", "\"git_sha\"", "\"built_at\""] {
            assert!(
                json.contains(field),
                "wire contract must include {field}: {json}"
            );
        }
    }
}
