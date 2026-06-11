use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::fs::device::{BlockDevice, DEVICE_PREFIX};
use crate::fs::efi::EfiBootOrder;
use crate::fs::mount::{FSType, Mount, MountFile, MountedPartition};
use crate::{LimeineCliError, Result};

/// Default configuration paths reitive to
/// boot partition.
///
/// First item is the default path.
pub static LIMINE_CONFIG_PATHS: &[&str] = &[
    "limine.conf",
    "EFI/limine/limine.conf",
    "EFI/LIMINE/limine.conf",
    "EFI/BOOT/limine.conf",
    "boot/limine/limine.conf",
    "boot/limine.conf",
    "limine/limine.conf",
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
    "EFI/limine/limine_x64.efi",
    "EFI/LIMINE/limine_x64.efi",
    "EFI/LIMINE/BOOTX64.EFI",
    "EFI/BOOT/limine_x64.efi",
];

/// Location for liminecli to mount devices
/// when searching for limine
pub static MOUNT_DIR: &str = "/run/liminecli";

#[derive(Debug, Error)]
pub enum BootError {
    #[error("Device with root mount contains multiple boot parttions. please specify device.")]
    MultipleBootPartitions,

    #[error("Failed to find device for limine")]
    FailedDiscovery,
}
#[derive(Debug, Clone)]
pub enum EspRoot {
    Path {
        mount_path: PathBuf,
        device: PathBuf,
    },
    Mounted(MountedPartition),
}

impl AsRef<Path> for EspRoot {
    fn as_ref(&self) -> &Path {
        match self {
            Self::Path { mount_path, .. } => mount_path,
            Self::Mounted(m) => m.path(),
        }
    }
}

impl EspRoot {
    pub fn device(&self) -> &Path {
        match self {
            Self::Path { device, .. } => device,
            Self::Mounted(m) => &m.device,
        }
    }
    pub fn block_device(&self) -> Result<BlockDevice> {
        BlockDevice::from_device_path(self.device())
    }
}

#[derive(Debug, Clone)]
pub struct LiminePaths {
    pub esp_root: EspRoot,
    pub config_path: PathBuf,
    pub efi_path: PathBuf,
}

impl LiminePaths {
    pub fn discover() -> Result<Self> {
        if let Some(p) = Self::try_from_mount("/boot")? {
            return Ok(p);
        }

        if let Some(from_efi_boot_entry) = Self::find_from_efi_boot_entry()? {
            return Ok(from_efi_boot_entry);
        }

        if let Some(from_root_boot_partition) = Self::find_from_root_boot_partition()? {
            return Ok(from_root_boot_partition);
        }

        Err(LimeineCliError::BootError(BootError::FailedDiscovery))
    }

    pub fn find_from_efi_boot_entry() -> Result<Option<Self>> {
        let boot_order = EfiBootOrder::read_boot_order()?;

        for item in boot_order.iter() {
            let boot_device = match item.try_read_boot_device() {
                Ok(b) => b,
                Err(_) => continue,
            };
            let boot_device_hard_drive = match boot_device.harddrive {
                Some(s) => s,
                None => continue,
            };
            let boot_device_uuid = match boot_device_hard_drive.partuuid {
                Some(uuid) => uuid,
                None => continue,
            };
            let block_device = BlockDevice::from_part_uuid(boot_device_uuid)?;

            let device = Path::new(DEVICE_PREFIX).join(block_device.name);
            return Self::from_device(FSType::VFat, &device, false);
        }

        Ok(None)
    }

    // try and find a single boot partition on the same device as
    // the root filesystem is mounted.
    pub fn find_from_root_boot_partition() -> Result<Option<Self>> {
        let mounts = Mount::get_mounts()?;
        let root_mount_block_device = {
            let root_device = mounts
                .iter()
                .filter(|m| m.mount_location == "/")
                .map(|m| m.device)
                .next();
            if let Some(root_device) = root_device {
                root_device
            } else {
                return Ok(None);
            }
        };

        let root_device = BlockDevice::from_device_path(root_mount_block_device)?;

        let root_parent = root_device.path.parent();
        let partitions = BlockDevice::enumerate_partitions()?;
        let mut root_boot_partition = None;
        for partition in partitions {
            let partition = partition?;

            if partition.path.parent() != root_parent {
                continue;
            }
            if partition.is_vfat()? {
                if root_boot_partition.is_some() {
                    Err(BootError::MultipleBootPartitions)?;
                }
                root_boot_partition = Some(partition)
            }
        }

        if let Some(root_boot_partition) = root_boot_partition {
            let device = Path::new(DEVICE_PREFIX).join(root_boot_partition.name);
            Self::from_device(FSType::VFat, &device, false)
        } else {
            Ok(None)
        }
    }

    pub fn try_from_mount(base: impl AsRef<Path>) -> Result<Option<Self>> {
        let base = base.as_ref();
        let mount_file = MountFile::read()?;
        let Some(target_mount) = mount_file
            .iter()
            .filter(|m| m.mount_location == base)
            .next()
        else {
            return Ok(None);
        };

        Ok(
            find_limine_files(base, true)?.map(|(config_path, efi_path)| Self {
                config_path,
                efi_path,
                esp_root: EspRoot::Path {
                    mount_path: target_mount.mount_location.into(),
                    device: target_mount.device.into(),
                },
            }),
        )
    }
    pub fn from_device(fs_type: FSType, device: &Path, must_exist: bool) -> Result<Option<Self>> {
        let mount = MountedPartition::mount(fs_type, device, MOUNT_DIR)?;

        Ok(
            find_limine_files(mount.path(), must_exist)?.map(|(config_path, efi_path)| Self {
                config_path,
                efi_path,
                esp_root: EspRoot::Mounted(mount),
            }),
        )
    }
}
fn find_limine_files(base: &Path, must_exist: bool) -> Result<Option<(PathBuf, PathBuf)>> {
    match base.metadata() {
        _ => {}
    };

    fn find_file(base: &Path, files: &[&str]) -> Result<Option<PathBuf>> {
        for file in files {
            let path = base.join(file);
            let metadata = match path.metadata() {
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(LimeineCliError::NoPermission {
                        path: base.into(),
                        error: e,
                    });
                }
                Ok(m) => m,
                Err(_) => continue,
            };

            if metadata.is_file() {
                return Ok(Some(file.into()));
            }
        }
        Ok(None)
    }

    let res = match (
        must_exist,
        find_file(base, LIMINE_CONFIG_PATHS)?,
        find_file(base, LIMINE_UEFI_PATHS)?,
    ) {
        (_, Some(c), Some(e)) => (c, e),
        (_, Some(c), None) => (c, Path::new(LIMINE_UEFI_PATHS[0]).into()),
        (_, None, Some(e)) => (Path::new(LIMINE_CONFIG_PATHS[0]).into(), e),
        (false, _, _) => (
            base.join(LIMINE_CONFIG_PATHS[0]),
            base.join(LIMINE_UEFI_PATHS[0]),
        ),
        (_, _, _) => {
            return Ok(None);
        }
    };
    Ok(Some(res))
}
