use std::array::IntoIter;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::{FromStr, Lines};

use nix::libc::FS;

use crate::fs::mount::{FSType, MountedPartition};
use crate::{LimeineCliError, Result};

/// Default configuration paths reitive to
/// boot partition.
///
/// First item is the default path.
pub static LIMINE_CONFIG_PATHS: &[&str] = &[
    "/limine.conf",
    "/EFI/limine/limine.conf",
    "/EFI/LIMINE/limine.conf",
    "/EFI/BOOT/limine.conf",
    "/boot/limine/limine.conf",
    "/boot/limine.conf",
    "/limine/limine.conf",
];

/// Default EFI module paths reitive to
/// boot partition.
///
/// First item is the default path
/// Note:
///     /EFI/BOOT/BOOTX64.EFI is not
///     included because it may not be
///     limine.
pub static LIMINE_UEFI_PATHS: &[&str] = &[
    "/EFI/limine/limine_x64.efi",
    "/EFI/LIMINE/limine_x64.efi",
    "/EFI/LIMINE/BOOTX64.EFI",
    "/EFI/BOOT/limine_x64.efi",
];

/// Location for liminecli to mount devices
/// when searching for limine
pub static MOUNT_DIR: &str = "/run/liminecli";

pub enum EspRoot {
    Path(PathBuf),
    Mounted(MountedPartition),
}

impl AsRef<Path> for EspRoot {
    fn as_ref(&self) -> &Path {
        match self {
            Self::Path(p) => p,
            Self::Mounted(m) => m.path(),
        }
    }
}

pub struct LiminePaths {
    pub esp_root: EspRoot,
    pub config_path: PathBuf,
    pub efi_path: PathBuf,
}

impl LiminePaths {
    pub fn discover() -> Result<Self> {
        if let Some(p) = Self::try_from_dir("/boot")? {
            return Ok(p);
        }

        todo!()
    }

    pub fn try_from_dir(base: impl AsRef<Path>) -> Result<Option<Self>> {
        let base = base.as_ref();

        Ok(find_limine_files(base, false)
            .ok()
            .map(|(config_path, efi_path)| Self {
                config_path,
                efi_path,
                esp_root: EspRoot::Path(base.into()),
            }))
    }
    pub fn try_from_device(fs_type: FSType, device: &Path) -> Result<Option<Self>> {
        let mount = MountedPartition::mount(fs_type, device, MOUNT_DIR)?;

        Ok(find_limine_files(mount.path(), false)
            .ok()
            .map(|(config_path, efi_path)| Self {
                config_path,
                efi_path,
                esp_root: EspRoot::Mounted(mount),
            }))
    }
}
fn find_limine_files(base: &Path, must_exist: bool) -> Result<(PathBuf, PathBuf)> {
    fn find_file(base: &Path, files: &[&str]) -> Option<PathBuf> {
        for file in files {
            let path = base.join(file);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    let res = match (
        must_exist,
        find_file(base, LIMINE_CONFIG_PATHS),
        find_file(base, LIMINE_UEFI_PATHS),
    ) {
        (_, Some(c), Some(e)) => (c, e),
        (_, Some(c), None) => (c, base.join(LIMINE_UEFI_PATHS[0])),
        (_, None, Some(e)) => (base.join(LIMINE_CONFIG_PATHS[0]), e),
        (false, _, _) => (
            base.join(LIMINE_CONFIG_PATHS[0]),
            base.join(LIMINE_UEFI_PATHS[0]),
        ),
        (_, _, _) => {
            return Err(LimeineCliError::NoConfigLocation);
        }
    };
    Ok(res)
}
