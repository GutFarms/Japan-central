//! Persist [`PoolConfig`] across reboots.
//!
//! * **esp** — last 4 KiB sector of a 4 MiB flash window (`0x3FF000`)
//! * **host** — `./scrypt-miner-config.bin` in the working directory

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistError {
    Io,
    Corrupt,
}

impl core::fmt::Display for PersistError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PersistError::Io => write!(f, "storage I/O error"),
            PersistError::Corrupt => write!(f, "no valid saved config"),
        }
    }
}

#[cfg(feature = "esp")]
mod esp_flash {
    use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
    use esp_storage::FlashStorage;

    use super::PersistError;
    use crate::config::{PoolConfig, CONFIG_BLOB_SIZE};

    /// Dedicated 4 KiB sector for miner credentials.
    /// Placed at the end of a 4 MiB region so it stays outside the typical
    /// factory app image on T-Display-S3 (4–16 MiB flash).
    pub const CONFIG_FLASH_OFFSET: u32 = 0x003F_F000;
    pub const CONFIG_FLASH_SECTOR: u32 = 4096;

    pub struct ConfigStore<'d> {
        flash: FlashStorage<'d>,
    }

    impl<'d> ConfigStore<'d> {
        pub fn new(flash: esp_hal::peripherals::FLASH<'d>) -> Self {
            Self {
                flash: FlashStorage::new(flash),
            }
        }

        pub fn load(&mut self) -> Result<PoolConfig, PersistError> {
            // Read a full sector-sized window; from_blob accepts v1 (320) and v2 (512).
            let mut blob = [0u8; CONFIG_BLOB_SIZE];
            self.flash
                .read(CONFIG_FLASH_OFFSET, &mut blob)
                .map_err(|_| PersistError::Io)?;
            PoolConfig::from_blob(&blob).map_err(|_| PersistError::Corrupt)
        }

        pub fn save(&mut self, cfg: &PoolConfig) -> Result<(), PersistError> {
            let blob = cfg.to_blob().map_err(|_| PersistError::Corrupt)?;
            NorFlash::erase(
                &mut self.flash,
                CONFIG_FLASH_OFFSET,
                CONFIG_FLASH_OFFSET + CONFIG_FLASH_SECTOR,
            )
            .map_err(|_| PersistError::Io)?;
            NorFlash::write(&mut self.flash, CONFIG_FLASH_OFFSET, &blob).map_err(|_| PersistError::Io)?;
            Ok(())
        }

        pub fn clear(&mut self) -> Result<(), PersistError> {
            NorFlash::erase(
                &mut self.flash,
                CONFIG_FLASH_OFFSET,
                CONFIG_FLASH_OFFSET + CONFIG_FLASH_SECTOR,
            )
            .map_err(|_| PersistError::Io)?;
            Ok(())
        }
    }
}

#[cfg(feature = "esp")]
pub use esp_flash::{ConfigStore, CONFIG_FLASH_OFFSET};

#[cfg(feature = "host")]
mod host_file {
    use std::fs;
    use std::path::Path;

    use super::PersistError;
    use crate::config::{PoolConfig, CONFIG_BLOB_SIZE};

    pub const HOST_CONFIG_PATH: &str = "scrypt-miner-config.bin";

    pub fn load_path(path: impl AsRef<Path>) -> Result<PoolConfig, PersistError> {
        let data = fs::read(path).map_err(|_| PersistError::Io)?;
        // Accept legacy 320-byte v1 files and current 512-byte v2 files.
        if data.len() < 320 {
            return Err(PersistError::Corrupt);
        }
        let mut blob = [0u8; CONFIG_BLOB_SIZE];
        let n = core::cmp::min(data.len(), CONFIG_BLOB_SIZE);
        blob[..n].copy_from_slice(&data[..n]);
        PoolConfig::from_blob(&blob).map_err(|_| PersistError::Corrupt)
    }

    pub fn save_path(path: impl AsRef<Path>, cfg: &PoolConfig) -> Result<(), PersistError> {
        let blob = cfg.to_blob().map_err(|_| PersistError::Corrupt)?;
        fs::write(path, blob).map_err(|_| PersistError::Io)
    }

    pub fn clear_path(path: impl AsRef<Path>) -> Result<(), PersistError> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(PersistError::Io),
        }
    }

    pub fn load() -> Result<PoolConfig, PersistError> {
        load_path(HOST_CONFIG_PATH)
    }

    pub fn save(cfg: &PoolConfig) -> Result<(), PersistError> {
        save_path(HOST_CONFIG_PATH, cfg)
    }

    pub fn clear() -> Result<(), PersistError> {
        clear_path(HOST_CONFIG_PATH)
    }
}

#[cfg(feature = "host")]
pub use host_file::{clear, load, save, HOST_CONFIG_PATH};
