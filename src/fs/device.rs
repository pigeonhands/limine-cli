use crate::Result;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use strum::EnumString;
use thiserror::Error;

const BLOCK_DEVICE_PREFIX: &str = "/sys/block";
const BLOCK_CLASS_DEVICE_PREFIX: &str = "/sys/class/block";
const DISK_DEVICE_PREFIX: &str = "/dev/disk";
const DEVICE_PREFIX: &str = "/dev";

const DISK_ITER_PARTITONS_DIR: &str = "/dev/disk/by-partuuid";

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("Device directory does not exist {0}.")]
    CannotEnumerate(PathBuf),

    #[error("Could not find device {0}.")]
    DeviceNotFound(PathBuf),

    #[error("Invalid device path {0}.")]
    InvalidDevicePath(PathBuf),

    #[error("Invalid disk device path {0}. Should be a vfat disk partition.")]
    InvalidDiskDevicePath(PathBuf),
    #[error("Invalid block device path {0}. Should be a vfat disk partition.")]
    InvalidBlockDevicePath(PathBuf),
    #[error("Invalid disk device {0}. Should be a vfat disk partition. {1}")]
    InvalidBlockDevice(PathBuf, io::Error),
}

#[derive(Debug, Copy, Clone, Default, EnumString)]
pub enum DeviceType {
    #[default]
    Other,

    #[strum(serialize = "partition")]
    Partition,

    #[strum(serialize = "disk")]
    Disk,
}

/// A device that lives in /sys/class/block
pub struct BlockDevice {
    pub path: PathBuf,
    pub name: String,
    pub part_uuid: String,
    pub device_type: DeviceType,
}

impl BlockDevice {
    pub fn enumerate_partitions() -> Result<impl Iterator<Item = Result<Self>>> {
        let disks = Path::new(DISK_ITER_PARTITONS_DIR);
        let items = disks
            .read_dir()
            .map_err(|_| DeviceError::CannotEnumerate(DISK_ITER_PARTITONS_DIR.into()))?;

        Ok(items.filter_map(|f| {
            let device_dir = f.ok()?;
            Some(Self::from_block_path(device_dir.path()))
        }))
    }
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if path.starts_with(DISK_DEVICE_PREFIX) {
            return Self::from_disk_path(path);
        }

        if path.starts_with(BLOCK_CLASS_DEVICE_PREFIX) || path.starts_with(BLOCK_DEVICE_PREFIX) {
            return Self::from_block_path(path);
        }

        Self::from_device_path(path)
    }

    pub fn from_disk_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.starts_with(DISK_DEVICE_PREFIX) {
            Err(DeviceError::InvalidDiskDevicePath(path.to_path_buf()))?;
        }

        let dev_path =
            fs::canonicalize(path).map_err(|_| DeviceError::DeviceNotFound(path.to_path_buf()))?;

        Ok(Self::from_device_path(dev_path)
            .map_err(|_| DeviceError::InvalidDiskDevicePath(path.to_path_buf()))?)
    }
    pub fn from_device_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.parent() != Some(Path::new(DEVICE_PREFIX)) {
            Err(DeviceError::InvalidDevicePath(path.to_path_buf()))?;
        }
        let base_name = path
            .file_name()
            .ok_or_else(|| DeviceError::DeviceNotFound(path.to_path_buf()))?;

        Ok(
            Self::from_block_path(Path::new(BLOCK_CLASS_DEVICE_PREFIX).join(base_name))
                .map_err(|_| DeviceError::InvalidBlockDevicePath(path.to_path_buf()))?,
        )
    }

    pub fn from_block_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let path = if path.starts_with(BLOCK_DEVICE_PREFIX) {
            let base_name = path
                .file_name()
                .ok_or_else(|| DeviceError::DeviceNotFound(path.to_path_buf()))?;
            Path::new(BLOCK_CLASS_DEVICE_PREFIX).join(base_name)
        } else {
            path.to_path_buf()
        };

        if !path.starts_with(BLOCK_CLASS_DEVICE_PREFIX) || !path.is_dir() {
            Err(DeviceError::InvalidBlockDevicePath(path.to_path_buf()))?;
        }

        Ok(Self::parse_device(&path)
            .map_err(|e| DeviceError::InvalidBlockDevice(path.to_path_buf(), e))?)
    }

    fn parse_device(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();

        let uevent_file = BufReader::new(File::open(path.join("uevent"))?);

        let mut disk_name = None;
        let mut disk_type = None;
        let mut disk_partuuid = None;
        for line in uevent_file.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let (key, val) = line
                .split_once("=")
                .ok_or_else(|| io::Error::other("bad udev"))?;

            match key {
                "DEVNAME" => disk_name = Some(val.into()),
                "DEVTYPE" => disk_type = Some(val.to_string()),
                "PARTUUID" => disk_partuuid = Some(val.into()),
                _ => continue,
            }
        }

        Ok(Self {
            path: path.into(),
            name: disk_name.ok_or_else(|| io::Error::other("No device name in udev"))?,
            part_uuid: disk_partuuid.ok_or_else(|| io::Error::other("No part uuid in udev"))?,
            device_type: DeviceType::from_str(
                disk_type
                    .as_ref()
                    .ok_or_else(|| io::Error::other("no device type in udev"))?,
            )
            .unwrap_or_default(),
        })
    }
}
