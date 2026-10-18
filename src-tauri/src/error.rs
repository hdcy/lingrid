use serde::Serialize;

/// Stable codes cross IPC; the UI translates them and can show optional diagnostics.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: &'static str) -> Self {
        Self { code, detail: None }
    }

    pub fn with_detail(code: &'static str, detail: impl ToString) -> Self {
        Self {
            code,
            detail: Some(detail.to_string()),
        }
    }
}
