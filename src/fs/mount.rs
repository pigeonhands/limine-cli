use std::{
    fs,
    path::{Path, PathBuf},
    str::{FromStr, Lines},
};

use strum::{AsRefStr, EnumString};

use crate::Result;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MountError {
    #[error("Failed to mount device {0}. errno: {1}")]
    FailedToMountDevice(PathBuf, nix::errno::Errno),

    #[error("Failed to unount device {device} mounted at {mount_path}. errno: {error_no}")]
    FailedToUnMountDevice {
        device: PathBuf,
        mount_path: PathBuf,
        error_no: nix::errno::Errno,
    },
}

#[derive(Debug, Clone, EnumString, AsRefStr)]
pub enum FSType {
    #[strum(serialize = "vfat")]
    VFat,

    #[strum(serialize = "btrfs")]
    BtrFS,

    #[strum(serialize = "ext4")]
    Ext4,
    Other,
}

#[derive(Debug, Clone)]
pub struct MountedPartition {
    #[allow(unused)]
    fs_type: FSType,
    device: PathBuf,
    mount_location: PathBuf,
    mounted: bool,
}

impl MountedPartition {
    pub fn mount(fs_type: FSType, device: &Path, mount_location: impl AsRef<Path>) -> Result<Self> {
        use nix::mount::{MsFlags, mount};
        let mount_location = mount_location.as_ref();

        mount(
            Some(device),
            mount_location,
            Some(fs_type.as_ref()),
            MsFlags::empty(),
            None::<&str>,
        )
        .map_err(|e| MountError::FailedToMountDevice(device.into(), e))?;

        Ok(Self {
            fs_type,
            device: device.into(),
            mount_location: mount_location.into(),
            mounted: true,
        })
    }

    pub fn unmount(&mut self) -> Result<()> {
        use nix::mount::umount;
        if !self.mounted {
            return Ok(());
        }

        umount(&self.mount_location).map_err(|e| MountError::FailedToUnMountDevice {
            device: self.device.clone(),
            mount_path: self.mount_location.clone(),
            error_no: e,
        })?;
        self.mounted = false;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.mount_location
    }
}

impl Drop for MountedPartition {
    fn drop(&mut self) {
        if let Err(e) = self.unmount() {
            eprintln!("{:?}", e);
        }
    }
}

#[derive(Debug)]
pub struct Mount<'a> {
    pub device: &'a str,
    pub mount_location: &'a Path,
    pub fs_type: FSType,
    pub flags: &'a str,
}

impl<'a> Mount<'a> {
    pub fn get_mounts() -> Result<MountFile> {
        MountFile::read()
    }
}

pub struct MountFile(String);

impl MountFile {
    pub fn read() -> Result<Self> {
        Ok(fs::read_to_string("/proc/mounts").map(Self)?)
    }

    pub fn iter(&self) -> MountFileIterator<'_> {
        MountFileIterator(self.0.lines())
    }
}
pub struct MountFileIterator<'a>(Lines<'a>);
impl<'a> Iterator for MountFileIterator<'a> {
    type Item = Mount<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(line) = self.0.next() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("#") {
                continue;
            }

            let mut split = line.split_whitespace();

            let (Some(device), Some(mount_location), Some(fs_type), Some(flags)) =
                (split.next(), split.next(), split.next(), split.next())
            else {
                continue;
            };

            return Some(Mount {
                device,
                mount_location: Path::new(mount_location),
                fs_type: FSType::from_str(fs_type).unwrap_or(FSType::Other),
                flags,
            });
        }
        None
    }
}
