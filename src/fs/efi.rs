use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use bitflags::bitflags;
use thiserror::Error;

type Result<T> = core::result::Result<T, EfiError>;

#[derive(Debug, Error)]
pub enum EfiError {
    #[error("Failed to open efi var {0}: {1}")]
    EfiVarDoesNotExist(String, #[source] io::Error),
    #[error("Failed to read efi var {0}: {1}")]
    InvalidEfiVar(String, #[source] io::Error),

    #[error("Efi var is not a load option")]
    NotEfiLoadOption,
    #[error("Efi node is not a path type")]
    NotEfiNodepath,

    #[error("Invalid efi boot partition format: {0}")]
    InvalidEfiPartitionFormat(u8),
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EfiVarAttrs: u32 {
        const NON_VOLATILE                          = 0x00000001;
        const BOOT_SERVICE_ACCESS                   = 0x00000002;
        const RUNTIME_ACCESS                        = 0x00000004;
        const HARDWARE_ERROR_RECORD                 = 0x00000008;
        const AUTHENTICATED_WRITE_ACCESS            = 0x00000010;
        const TIME_BASED_AUTHENTICATED_WRITE_ACCESS = 0x00000020;
        const APPEND_WRITE                          = 0x00000040;
    }
}

const EFI_VARS: &str = "/sys/firmware/efi/efivars";
const EFI_GLOBAL_GUID: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";

#[derive(Debug, Clone)]
pub struct EfiVar {
    pub attrs: EfiVarAttrs,
    pub data: Vec<u8>,
}

impl EfiVar {
    pub fn read_global(name: &str) -> Result<Self> {
        Self::read_var(&format!("{}-{}", name, EFI_GLOBAL_GUID))
    }
    pub fn read_var(name: &str) -> Result<Self> {
        let efi_var_path = Path::new(EFI_VARS).join(name);

        let mut efi_var_file = fs::File::open(efi_var_path)
            .map_err(|e| EfiError::EfiVarDoesNotExist(name.into(), e))?;

        let attrs = {
            let mut buff = [0u8; 4];
            efi_var_file
                .read_exact(&mut buff)
                .map_err(|e| EfiError::InvalidEfiVar(name.into(), e))?;
            EfiVarAttrs::from_bits_retain(u32::from_le_bytes(buff))
        };

        let data = {
            let mut buff = Vec::new();
            efi_var_file
                .read_to_end(&mut buff)
                .map_err(|e| EfiError::InvalidEfiVar(name.into(), e))?;
            buff
        };

        Ok(Self { attrs, data })
    }
}

#[derive(Debug, Clone)]
pub struct EfiBootOrder(EfiVar);

impl EfiBootOrder {
    pub fn read_boot_order() -> Result<Self> {
        Ok(Self(EfiVar::read_global("BootOrder")?))
    }

    pub fn iter<'a>(&self) -> impl Iterator<Item = EfiBootOrderItem> {
        let data = &self.0.data;
        let (u16_chunks, _) = data.as_chunks::<2>();
        u16_chunks
            .into_iter()
            .map(|x| u16::from_le_bytes(*x))
            .map(EfiBootOrderItem)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EfiBootOrderItem(pub u16);

impl EfiBootOrderItem {
    pub fn try_read_boot_device(&self) -> Result<EfiBootDevice> {
        EfiBootDevice::read_entry(self.0)
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EfiLoadOptionAttrs: u32 {
        const ACTIVE                = 0x00000001;
        const FORCE_RECONNECT       = 0x00000002;
        const HIDDEN                = 0x00000008;
        const CATEGORY_BOOT        = 0x00000000;
        const CATEGORY_APP         = 0x00000100;
    }
}

#[derive(Debug, Clone)]
pub struct EfiLoadOption<'a> {
    pub attributes: EfiLoadOptionAttrs,
    pub description: String,
    pub device_path: &'a [u8],
    pub optional_data: &'a [u8],
}

impl<'a> EfiLoadOption<'a> {
    pub const MIN_DATA_SIZE: usize = 6;
    pub fn parse(data: &'a [u8]) -> Result<Option<Self>> {
        if data.len() < Self::MIN_DATA_SIZE {
            return Ok(None);
        }

        let attributes = {
            let buff: &[u8; 4] = &data[..4].try_into().expect("should not fail");
            EfiLoadOptionAttrs::from_bits_retain(u32::from_le_bytes(*buff))
        };

        let device_path_length = {
            let buff: &[u8; 2] = &data[4..6].try_into().unwrap();
            u16::from_le_bytes(*buff) as usize
        };

        let desc_buff = &data[6..];

        let (description, description_len) = {
            let (u16_chunks, _) = desc_buff.as_chunks::<2>();

            let u16_buffer: Vec<_> = u16_chunks
                .iter()
                .copied()
                .map(u16::from_le_bytes)
                .take_while(|x| *x != 0)
                .collect();

            (
                String::from_utf16_lossy(&u16_buffer),
                (u16_buffer.len() + 1) * 2,
            )
        };

        if desc_buff.len() < description_len + device_path_length {
            return Ok(None);
        }

        let device_buffer = &desc_buff[description_len..];
        let device_path = &device_buffer[..device_path_length];
        let optional_data = &device_buffer[device_path_length..];

        Ok(Some(Self {
            attributes,
            description,
            device_path: device_path,
            optional_data: optional_data,
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PartitionFormat {
    Mbr = 0x01,
    Gpt = 0x02,
}

impl TryFrom<u8> for PartitionFormat {
    type Error = EfiError;

    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            0x01 => Ok(Self::Mbr),
            0x02 => Ok(Self::Gpt),
            other => Err(EfiError::InvalidEfiPartitionFormat(other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DevicePathNodeHardDrive {
    pub partition_number: u32,
    pub partition_start: u64,
    pub partition_size: u64,
    pub partuuid: Option<uuid::Uuid>,
    pub partition_format: PartitionFormat,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DevicePathNode<'a> {
    // UTF16 string path
    FilePath(&'a [u8]),
    HardDrive(DevicePathNodeHardDrive),
    Acpi {
        hid: u32,
        uid: u32,
    },
    Pci {
        function: u8,
        device: u8,
    },
    End,
    Unknown {
        node_type: u8,
        sub_type: u8,
        data: &'a [u8],
    },
}

impl<'a> DevicePathNode<'a> {
    pub const MIN_DATA_SIZE: usize = 4;
    pub fn read_nodes(device_path_buffer: &'a [u8]) -> DevicePathNodeIterator<'a> {
        DevicePathNodeIterator::new(device_path_buffer)
    }

    pub fn try_read_single(data: &'a [u8]) -> Result<Option<(usize, Self)>> {
        if data.len() < Self::MIN_DATA_SIZE {
            return Ok(None);
        }

        let node_type = data[0];
        let sub_type = data[1];
        let length = u16::from_le_bytes([data[2], data[3]]) as usize;

        if length < 4 || length > data.len() {
            return Ok(None);
        }
        let node_data = &data[4..length];

        let node = match (node_type, sub_type) {
            // End of Hardware Device Path
            (0x7F, 0xFF) => DevicePathNode::End,

            // Media File Path (type=4, subtype=4)
            (0x04, 0x04) => {
                let (u16_chunks, _) = node_data.as_chunks::<2>();
                let null_terminator = match u16_chunks
                    .iter()
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .position(|c| c == 0)
                {
                    Some(pos) => pos,
                    None => return Ok(None),
                };
                DevicePathNode::FilePath(&node_data[..(null_terminator * 2)])
            }

            // Hard Drive Media (type=4, subtype=1)
            (0x04, 0x01) if node_data.len() >= 38 => {
                let partition_number = u32::from_le_bytes(node_data[0..4].try_into().unwrap());
                let partition_start = u64::from_le_bytes(node_data[4..12].try_into().unwrap());
                let partition_size = u64::from_le_bytes(node_data[12..20].try_into().unwrap());
                let sig_bytes = &node_data[20..36];
                let partition_format = PartitionFormat::try_from(node_data[36])?;
                let signature_type = node_data[37];

                let partuuid = if signature_type == 0x02 {
                    Some(uuid::Uuid::from_bytes_le(sig_bytes.try_into().unwrap()))
                } else {
                    None
                };

                DevicePathNode::HardDrive(DevicePathNodeHardDrive {
                    partition_number,
                    partition_start,
                    partition_size,
                    partuuid,
                    partition_format,
                })
            }

            // ACPI (type=2, subtype=1)
            (0x02, 0x01) if node_data.len() >= 8 => {
                let hid =
                    u32::from_le_bytes([node_data[0], node_data[1], node_data[2], node_data[3]]);
                let uid =
                    u32::from_le_bytes([node_data[4], node_data[5], node_data[6], node_data[7]]);
                DevicePathNode::Acpi { hid, uid }
            }

            // PCI (type=3, subtype=1)
            (0x03, 0x01) if node_data.len() >= 2 => DevicePathNode::Pci {
                function: node_data[0],
                device: node_data[1],
            },

            _ => DevicePathNode::Unknown {
                node_type,
                sub_type,
                data: node_data,
            },
        };

        Ok(Some((length, node)))
    }
}

pub struct DevicePathNodeIterator<'a> {
    index: usize,
    buffer: &'a [u8],
}

impl<'a> DevicePathNodeIterator<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer, index: 0 }
    }
}

impl<'a> Iterator for DevicePathNodeIterator<'a> {
    type Item = Result<DevicePathNode<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer.len() <= self.index + DevicePathNode::MIN_DATA_SIZE {
            return None;
        }

        let node = DevicePathNode::try_read_single(&self.buffer[self.index..]).transpose()?;
        match node {
            Ok((len, node)) => {
                self.index += len;
                Some(Ok(node))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

/// This is a EfiLoadOption that has
/// at least DevicePathNode::Path and
/// optionally a DevicePathNode::HardDrive
#[derive(Debug, PartialEq)]
pub struct EfiBootDevice {
    pub boot_num: u16,
    pub attrs: EfiVarAttrs,
    pub load_options: EfiLoadOptionAttrs,
    pub path: PathBuf,
    pub harddrive: Option<DevicePathNodeHardDrive>,
}

impl EfiBootDevice {
    pub fn read_entry(boot_num: u16) -> Result<Self> {
        let var = EfiVar::read_global(&format!("Boot{:04X}", boot_num))?;
        let load_option = EfiLoadOption::parse(&var.data)?.ok_or(EfiError::NotEfiLoadOption)?;

        let path_nodes = DevicePathNode::read_nodes(load_option.device_path);

        let mut file_path = None;
        let mut harddrive = None;

        for node in path_nodes.filter_map(|f| f.ok()) {
            match node {
                DevicePathNode::FilePath(path) => file_path = Some(path),
                DevicePathNode::HardDrive(drive) => harddrive = Some(drive),
                _ => continue,
            }
        }

        let file_path = file_path.ok_or(EfiError::NotEfiNodepath)?;

        let (u16_buffer, _) = file_path.as_chunks::<2>();
        let str_buffer: Vec<_> = u16_buffer.iter().copied().map(u16::from_le_bytes).collect();

        Ok(Self {
            boot_num,
            harddrive,
            attrs: var.attrs,
            load_options: load_option.attributes,
            path: PathBuf::from(String::from_utf16_lossy(&str_buffer)),
        })
    }
}
