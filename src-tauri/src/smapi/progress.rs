use crate::error::AppError;
use serde::Serialize;
use std::{
    cell::Cell,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

/// Event name the UI listens on.
pub const EVENT: &str = "smapi://progress";

/// Minimum gap between download progress events.
const DOWNLOAD_INTERVAL: Duration = Duration::from_millis(120);

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    Release,
    Download,
    Verify,
    Extract,
    Install,
    Uninstall,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StepState {
    Running,
    Done,
    Failed,
}

/// One progress event. `detail` carries machine-readable notes (a stable error code, or
/// `cached`), never user-facing text: the UI translates it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub step: Step,
    pub state: StepState,
    pub detail: Option<String>,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub elapsed_ms: u64,
}

/// Reports install progress to the UI.
///
/// Reporting is best-effort: a failed emit (for example a closed window) never interrupts
/// the work, and every step is wrapped so failures are still reported before returning.
pub struct Reporter<'a> {
    app: &'a AppHandle,
    last_download: Cell<Option<Instant>>,
}

impl<'a> Reporter<'a> {
    pub fn new(app: &'a AppHandle) -> Self {
        Self {
            app,
            last_download: Cell::new(None),
        }
    }

    /// Run one step, reporting its start and then its result.
    pub fn run<T>(
        &self,
        step: Step,
        work: impl FnOnce() -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let started = Instant::now();
        self.emit(step, StepState::Running, None, 0, None, 0);
        let elapsed = |started: Instant| started.elapsed().as_millis() as u64;
        match work() {
            Ok(value) => {
                self.emit(step, StepState::Done, None, 0, None, elapsed(started));
                Ok(value)
            }
            Err(error) => {
                self.emit(
                    step,
                    StepState::Failed,
                    Some(error.code.to_owned()),
                    0,
                    None,
                    elapsed(started),
                );
                Err(error)
            }
        }
    }

    /// Report a step as completed without work, e.g. a reused download.
    pub fn skipped(&self, step: Step, detail: &str) {
        self.emit(step, StepState::Done, Some(detail.to_owned()), 0, None, 0);
    }

    /// Byte progress for the running download, throttled except for the final update.
    pub fn downloading(&self, downloaded: u64, total: Option<u64>) {
        let now = Instant::now();
        let finished = total.is_some_and(|total| downloaded >= total);
        let due = self
            .last_download
            .get()
            .is_none_or(|last| now.duration_since(last) >= DOWNLOAD_INTERVAL);
        if !due && !finished {
            return;
        }
        self.last_download.set(Some(now));
        self.emit(
            Step::Download,
            StepState::Running,
            None,
            downloaded,
            total,
            0,
        );
    }

    fn emit(
        &self,
        step: Step,
        state: StepState,
        detail: Option<String>,
        downloaded: u64,
        total: Option<u64>,
        elapsed_ms: u64,
    ) {
        let _ = self.app.emit(
            EVENT,
            Progress {
                step,
                state,
                detail,
                downloaded,
                total,
                elapsed_ms,
            },
        );
    }
}
