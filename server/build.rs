use std::process::Command;

/// Run a command and return its trimmed stdout, or `None` if it fails
/// (e.g. building from a source tarball with no `.git`, or git not installed).
fn capture(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn main() {
    println!("cargo:rerun-if-changed=../dist");

    // Embed the git SHA + build date so the deployed binary can report exactly
    // what it was built from (`GET /api/version`). Falls back to "unknown"
    // rather than failing the build outside a git checkout, with a warning so
    // the operator can tell "missing git" apart from a tarball build. A dirty
    // working tree gets a "-dirty" suffix — a clean-looking SHA from a build
    // that included uncommitted changes would mislead the deploy-lag check.
    let git_sha = match capture("git", &["rev-parse", "--short=9", "HEAD"]) {
        Some(sha) => {
            let dirty = capture("git", &["status", "--porcelain"]).is_some();
            if dirty {
                format!("{sha}-dirty")
            } else {
                sha
            }
        }
        None => {
            println!("cargo:warning=fewd-server: git SHA unavailable (no git or no .git); /api/version will report 'unknown'");
            "unknown".into()
        }
    };
    let built_at =
        capture("date", &["-u", "+%Y-%m-%d %H:%M UTC"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=FEWD_GIT_SHA={git_sha}");
    println!("cargo:rustc-env=FEWD_BUILT_AT={built_at}");

    // Re-run when HEAD moves so the SHA stays current. HEAD usually holds a
    // branch ref; commits rewrite that ref file, not HEAD itself, so watch
    // both. Resolve the real paths through `--git-path` rather than assuming
    // `../.git/...` — in a git worktree `.git` is a pointer file, and a watch
    // on a nonexistent path makes cargo treat the script as always-dirty
    // (rebuilding every time) while the intended watch never fires.
    if let Some(head_path) = capture("git", &["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head_path}");
        if let Ok(head) = std::fs::read_to_string(&head_path) {
            if let Some(branch_ref) = head.trim().strip_prefix("ref: ") {
                if let Some(ref_path) = capture("git", &["rev-parse", "--git-path", branch_ref]) {
                    println!("cargo:rerun-if-changed={ref_path}");
                }
            }
        }
    }
}
