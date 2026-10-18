use super::model::DiscoveryLocations;
use crate::error::AppError;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

pub fn locations() -> Result<DiscoveryLocations, AppError> {
    #[cfg(target_os = "linux")]
    {
        Ok(linux::locations())
    }
    #[cfg(target_os = "windows")]
    {
        Ok(windows::locations())
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Err(AppError::new("platformUnsupported"))
    }
}
