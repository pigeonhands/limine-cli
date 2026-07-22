use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{
    Result,
    cli::InstallArgs,
    config::Config,
    fs::{
        boot::{BootError, LiminePaths},
        mount::FSType,
    },
};

static LIMEINE_DATA_DIR: &str = "/usr/share/limine";

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("The machines architecture is not supported by limine-cli")]
    UnsupportedArchitecture,

    #[error("Could not find limine efi firmware file at {0}")]
    LimineEfiFileNotFound(PathBuf),

    #[error("Failed boot query: {0}")]
    BootError(#[from] BootError),
}
fn efi_firmware_file() -> core::result::Result<&'static str, InstallError> {
    #[cfg(target_arch = "x86_64")]
    return Ok("BOOTX64.EFI");

    #[allow(unreachable_code)]
    Err(InstallError::UnsupportedArchitecture)
}
pub fn run(config: &Config, _args: &InstallArgs) -> Result<()> {
    let limine_data_dir = Path::new(LIMEINE_DATA_DIR);

    let limine_efi = limine_data_dir.join(efi_firmware_file()?);

    if !limine_efi.is_file() {
        Err(InstallError::LimineEfiFileNotFound(limine_efi.into()))?;
    }

    let limine_paths = if let Some(limine_device) = &config.limemine_block_device {
        LiminePaths::from_device(FSType::VFat, &limine_device, false)?;
    } else {
        LiminePaths::discover()?;
    };

    Ok(())
}
