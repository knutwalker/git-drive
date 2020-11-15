use crate::data::{Driver, Navigator};
use color_eyre::{Help, Result, SectionExt};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    convert::TryInto,
    fmt,
    fs::{self, File},
    io::{Error as IoError, ErrorKind, Read, Write},
    path::{Path, PathBuf},
};

static APPLICATION: &str = env!("CARGO_PKG_NAME");
static CONFIG_FILE: &str = concat!(env!("CARGO_PKG_NAME"), "_config.json");

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub navigators: Vec<Navigator>,
    pub drivers: Vec<Driver>,
}

pub fn config_file() -> Option<PathBuf> {
    let dirs = ProjectDirs::from("de", "knutwalker", APPLICATION)?;
    let mut file = dirs.config_dir().to_path_buf();
    file.push(CONFIG_FILE);
    Some(file)
}

pub fn load() -> Result<Config> {
    let file = match config_file() {
        Some(file) => file,
        None => return Ok(Config::default()),
    };
    load_from(&file)
        .with_section(move || format!("{}", file.display()).header("File:"))
        .with_suggestion(|| {
            format!("Make sure that the config file is accessible and is properly formatted")
        })
}

fn load_from(file: &Path) -> Result<Config, Error> {
    match File::open(file) {
        Ok(mut cfg) => {
            let meta = cfg.metadata().map_err(Error::FileReadError)?;
            let size = meta.len();
            let size: usize = size
                .try_into()
                .map_err(|_| {
                    IoError::new(
                        ErrorKind::Other,
                        format!("The config file is too large: {} bytes", size),
                    )
                })
                .map_err(Error::FileReadError)?;
            let mut data = Vec::with_capacity(size);
            let read = cfg.read_to_end(&mut data).map_err(Error::FileReadError)?;
            let config = serde_json::from_slice(&data[..read]).map_err(Error::ConfigReadError)?;
            Ok(config)
        }
        Err(e) => match e.kind() {
            ErrorKind::NotFound => Ok(Config::default()),
            ErrorKind::PermissionDenied => Err(Error::FileReadError(e)),
            _ => Err(Error::IoError(e)),
        },
    }
}

pub fn store(config: Config) -> Result<()> {
    let data = serde_json::to_vec_pretty(&config).map_err(Error::ConfigWriteError)?;
    let file = config_file().ok_or(Error::NoConfigDirectory)?;
    store_to(&file, data, true)
        .with_section(move || format!("{:?}", config).header("Config:"))
        .with_section(move || format!("{}", file.display()).header("File:"))
        .with_suggestion(|| format!("Make sure that the config file is accessible and writable"))
}

fn store_to(file: &Path, data: Vec<u8>, create_parent: bool) -> Result<(), Error> {
    let mut f = match File::create(file) {
        Ok(f) => f,
        Err(e) => {
            return match e.kind() {
                ErrorKind::NotFound if create_parent => {
                    if let Some(parent) = file.parent() {
                        fs::create_dir_all(parent).map_err(Error::FileWriteError)?;
                    }
                    store_to(file, data, false)
                }
                ErrorKind::PermissionDenied => Err(Error::FileWriteError(e)),
                _ => Err(Error::IoError(e)),
            }
        }
    };
    f.write_all(&data)?;
    f.flush()?;
    Ok(())
}

#[derive(Debug)]
pub enum Error {
    NoConfigDirectory,
    FileReadError(std::io::Error),
    FileWriteError(std::io::Error),
    ConfigReadError(serde_json::Error),
    ConfigWriteError(serde_json::Error),
    IoError(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NoConfigDirectory => write!(f, "The configuration directoy could not be found"),
            Error::FileReadError(_) => write!(f, "Could not read the configuration file"),
            Error::FileWriteError(_) => write!(f, "Could not write the configuration file"),
            Error::ConfigReadError(_) => write!(f, "Could not read the configuration data"),
            Error::ConfigWriteError(_) => write!(f, "Could not write the configuration data"),
            Error::IoError(_) => write!(f, "IO error"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::FileReadError(e) => Some(e),
            Error::FileWriteError(e) => Some(e),
            Error::ConfigReadError(e) => Some(e),
            Error::ConfigWriteError(e) => Some(e),
            Error::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::IoError(e)
    }
}
