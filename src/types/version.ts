/** Mirror of the Rust `VersionInfo` DTO (server/src/dto.rs). */
export interface VersionInfo {
  version: string
  git_sha: string
  built_at: string
}
