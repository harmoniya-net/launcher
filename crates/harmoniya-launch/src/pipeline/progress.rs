//! Progress bookkeeping for the launch pipeline: accumulates raw
//! `opys_runtime::InstallProgress` callbacks into the UI-facing
//! [`LaunchProgress`] snapshots (porting the web SSE handler's logic).

use std::collections::HashMap;

use opys_runtime::InstallProgress;

use harmoniya_api::now_ms;

use super::{LaunchFile, LaunchProgress, Phase};

/// Throttle interval for per-file byte updates, matching the web `BYTE_THROTTLE_MS`.
const BYTE_THROTTLE_MS: u64 = 250;

/// Accumulates raw `InstallProgress` callbacks into a `LaunchProgress` snapshot.
#[derive(Default)]
pub(super) struct Tracker {
    files: HashMap<String, FileState>,
    download_fetched: u32,
    download_total: u32,
    /// Current phase — read by the parent module to classify a terminal error.
    pub(super) phase: Option<Phase>,
}

struct FileState {
    bytes: u64,
    total: Option<u64>,
    last_emit: u64,
}

impl Tracker {
    fn percent(&self) -> u8 {
        let Some(phase) = self.phase else { return 0 };
        let (lo, hi) = phase.range();
        let pct = if phase != Phase::Download {
            lo
        } else if self.download_total == 0 {
            hi
        } else {
            lo + (hi - lo) * (self.download_fetched as f32 / self.download_total as f32)
        };
        pct.round().clamp(0., 100.) as u8
    }

    fn snapshot(&self) -> LaunchProgress {
        let mut files: Vec<LaunchFile> = self
            .files
            .iter()
            .map(|(path, s)| LaunchFile { path: path.clone(), bytes: s.bytes, total: s.total })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        LaunchProgress { phase: self.phase.unwrap_or(Phase::Resolve), percent: self.percent(), files }
    }

    /// Apply one raw progress event. Returns `Some(snapshot)` when the UI should
    /// re-render (mirrors which events the web handler emits on).
    pub(super) fn apply(&mut self, p: InstallProgress) -> Option<LaunchProgress> {
        match p {
            InstallProgress::Resolve | InstallProgress::Pointer { .. } => {
                self.phase = Some(Phase::Resolve);
                Some(self.snapshot())
            }
            InstallProgress::Download { fetched, total, .. } => {
                self.phase = Some(Phase::Download);
                self.download_fetched = fetched;
                self.download_total = total;
                Some(self.snapshot())
            }
            InstallProgress::DownloadStart { path, total } => {
                self.phase = Some(Phase::Download);
                self.files.insert(
                    path,
                    FileState { bytes: 0, total: (total > 0).then_some(total), last_emit: 0 },
                );
                Some(self.snapshot())
            }
            InstallProgress::DownloadBytes { path, bytes } => {
                let now = now_ms();
                let s = self.files.get_mut(&path)?;
                s.bytes = bytes;
                if now.saturating_sub(s.last_emit) < BYTE_THROTTLE_MS {
                    return None;
                }
                s.last_emit = now;
                Some(self.snapshot())
            }
            InstallProgress::DownloadDone { path } => {
                self.files.remove(&path);
                None
            }
            InstallProgress::Verify => {
                self.phase = Some(Phase::Verify);
                self.files.clear();
                Some(self.snapshot())
            }
            InstallProgress::Extract { .. } => {
                self.phase = Some(Phase::Extract);
                Some(self.snapshot())
            }
            InstallProgress::Sweep { .. } => None,
        }
    }
}
