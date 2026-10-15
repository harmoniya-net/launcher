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
    /// Finished files, as a count and in bytes, and what the manifest declares
    /// in all. The bar follows bytes when sizes are declared, else files.
    fetched: u32,
    fetch_total: u32,
    fetched_bytes: u64,
    fetch_total_bytes: u64,
    /// Current phase — read by the parent module to classify a terminal error.
    pub(super) phase: Option<Phase>,
}

struct FileState {
    bytes: u64,
    total: Option<u64>,
    last_emit: u64,
}

impl Tracker {
    /// How far the fetch is, 0..=1: bytes finished plus bytes in flight over
    /// the declared total, or files when the manifest declares no sizes.
    fn fetch_fraction(&self) -> f32 {
        if self.fetch_total_bytes > 0 {
            let in_flight: u64 = self.files.values().map(|f| f.bytes).sum();
            ((self.fetched_bytes + in_flight) as f64 / self.fetch_total_bytes as f64) as f32
        } else if self.fetch_total > 0 {
            self.fetched as f32 / self.fetch_total as f32
        } else {
            1.
        }
    }

    fn percent(&self) -> u8 {
        let Some(phase) = self.phase else { return 0 };
        let (lo, hi) = phase.range();
        let pct = match phase {
            Phase::Fetch => lo + (hi - lo) * self.fetch_fraction().clamp(0., 1.),
            _ => lo,
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
    /// re-render.
    pub(super) fn apply(&mut self, p: InstallProgress) -> Option<LaunchProgress> {
        match p {
            InstallProgress::Resolve => {
                self.phase = Some(Phase::Resolve);
                Some(self.snapshot())
            }
            InstallProgress::Download { fetched, total, bytes, total_bytes, .. } => {
                self.phase = Some(Phase::Fetch);
                self.fetched = fetched;
                self.fetch_total = total;
                self.fetched_bytes = bytes;
                self.fetch_total_bytes = total_bytes;
                Some(self.snapshot())
            }
            InstallProgress::DownloadStart { path, total } => {
                self.phase = Some(Phase::Fetch);
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
            // The `Download` event that follows carries the file's bytes into
            // the finished count, so dropping it here doesn't move the bar back.
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
                self.files.clear();
                Some(self.snapshot())
            }
            InstallProgress::Cleanup { removed, directories } => {
                tracing::info!(removed, directories, "cleanup removed stale files");
                self.phase = Some(Phase::Cleanup);
                Some(self.snapshot())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_follows_bytes_in_flight_and_phases_in_order() {
        let mut t = Tracker::default();
        assert_eq!(t.apply(InstallProgress::Resolve).unwrap().percent, 0);
        let p = t
            .apply(InstallProgress::Download {
                fetched: 0,
                total: 2,
                skipped: 0,
                bytes: 0,
                total_bytes: 200,
            })
            .unwrap();
        assert_eq!((p.phase, p.percent), (Phase::Fetch, 5));
        t.apply(InstallProgress::DownloadStart { path: "a".into(), total: 100 });
        let p = t.apply(InstallProgress::DownloadBytes { path: "a".into(), bytes: 100 }).unwrap();
        // Half the bytes of a 5..85 window.
        assert_eq!(p.percent, 45);
        assert!(t.apply(InstallProgress::DownloadDone { path: "a".into() }).is_none());
        let p = t
            .apply(InstallProgress::Download {
                fetched: 1,
                total: 2,
                skipped: 0,
                bytes: 100,
                total_bytes: 200,
            })
            .unwrap();
        assert_eq!(p.percent, 45);
        assert_eq!(t.apply(InstallProgress::Verify).unwrap().phase, Phase::Verify);
        assert_eq!(t.apply(InstallProgress::Extract { count: 1 }).unwrap().phase, Phase::Extract);
        let p = t.apply(InstallProgress::Cleanup { removed: 3, directories: 1 }).unwrap();
        assert_eq!((p.phase, p.percent), (Phase::Cleanup, 99));
    }

    #[test]
    fn a_manifest_without_sizes_counts_files() {
        let mut t = Tracker::default();
        let p = t
            .apply(InstallProgress::Download {
                fetched: 1,
                total: 4,
                skipped: 0,
                bytes: 0,
                total_bytes: 0,
            })
            .unwrap();
        assert_eq!(p.percent, 25);
    }
}
