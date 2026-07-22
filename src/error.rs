use std::{io, path::PathBuf};

use thiserror::Error;

use crate::{
    commands::install::InstallError,
    fs::{boot::BootError, device::DeviceError, efi::EfiError, mount::MountError},
};

pub type Result<T> = core::result::Result<T, LimeineCliError>;

#[derive(Debug, Error)]
pub enum LimeineCliError {
    #[error("The machines architecture is not supported by limine-cli")]
    UnsupportedArchitecture,

    #[error("Failed to aquire lockfile: {0}")]
    AlreadyRunning(PathBuf),
    #[error("IO Error: {0}")]
    IOError(#[from] io::Error),

    #[error("Configuration is invalid. {0}")]
    InvalidConfig(#[source] toml::de::Error),

    #[error("Could not write configuration to {0}. {1}")]
    FailedToWriteConfig(PathBuf, #[source] std::io::Error),

    #[error("No permission to access {path}. ({error})")]
    NoPermission {
        path: PathBuf,
        #[source]
        error: io::Error,
    },

    #[error(
        "You must supply the boot device if snapper has not been installed and there is no exactly 1 boot partion on the same drive as root partition."
    )]
    NoConfigLocation,

    #[error("Mount error: {0}")]
    MountError(#[from] MountError),

    #[error("Device error: {0}")]
    DeviceError(#[from] DeviceError),

    #[error("Boot partition error: {0}")]
    BootError(#[from] BootError),

    #[error("Efi error: {0}")]
    EfiError(#[from] EfiError),

    #[error("Efi error: {0}")]
    InstallError(#[from] InstallError),
}
