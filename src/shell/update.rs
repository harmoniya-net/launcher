use anyhow::Result;

/// Best-effort GitLab-Releases self-update. self_update is sync; this is dispatched
/// onto a blocking worker thread by the caller. The GitLab project is public, so
/// release assets download without auth; CI publishes one asset per target triple
/// (which `self_update` matches against the running binary's target).
///
/// Returns `Ok(true)` if a newer version was installed (caller should suggest restart).
pub fn run_blocking() -> Result<bool> {
    let status = self_update::backends::gitlab::Update::configure()
        .repo_owner("harmoniya")
        .repo_name("launcher")
        .bin_name("harmoniya-launcher")
        .current_version(env!("CARGO_PKG_VERSION"))
        .show_download_progress(false)
        .no_confirm(true)
        .build()?
        .update()?;
    Ok(status.updated())
}
