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
    // rather than failing the build outside a git checkout.
    let git_sha =
        capture("git", &["rev-parse", "--short=9", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let built_at =
        capture("date", &["-u", "+%Y-%m-%d %H:%M UTC"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=FEWD_GIT_SHA={git_sha}");
    println!("cargo:rustc-env=FEWD_BUILT_AT={built_at}");

    // Re-run when HEAD moves so the SHA stays current. HEAD usually holds a
    // branch ref; commits rewrite that ref file, not HEAD itself, so watch both.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    if let Ok(head) = std::fs::read_to_string("../.git/HEAD") {
        if let Some(branch_ref) = head.trim().strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=../.git/{branch_ref}");
        }
    }
}
