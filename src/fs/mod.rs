pub mod boot;
pub mod device;
pub mod efi;
pub mod mount;

use nix::fcntl::{Flock, FlockArg};
use std::fs::File;

use crate::{LimeineCliError, Result};
pub const LOCK_FILE: &str = "/run/limine-cli.lock";

#[must_use = "lock is released when this is dropped — bind it to a variable"]
pub struct LockFile(#[allow(unused)] Flock<File>);

impl LockFile {
    pub fn aquire() -> Result<Self> {
        let lock_file = match File::create(LOCK_FILE) {
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(LimeineCliError::NoPermission {
                    path: LOCK_FILE.into(),
                    error: e,
                });
            }
            Ok(file) => file,
            Err(e) => Err(e)?,
        };
        let lock = Flock::lock(lock_file, FlockArg::LockExclusiveNonblock)
            .map_err(|_| LimeineCliError::AlreadyRunning(LOCK_FILE.into()))?;

        Ok(Self(lock))
    }
}
