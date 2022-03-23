use crate::{
    data::{Driver, Navigator},
    Result,
};
use directories::ProjectDirs;
use eyre::WrapErr;
use nanoserde::{DeJson, SerJson};
use std::{
    fmt,
    fs::{self, File},
    io::{Error as IoError, ErrorKind, Read, Write},
    path::{Path, PathBuf},
};

static APPLICATION: &str = env!("CARGO_PKG_NAME");
static CONFIG_FILE: &str = concat!(env!("CARGO_PKG_NAME"), "_config.json");

#[derive(Debug, Default, DeJson, SerJson)]
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
    load_from(&file).wrap_err_with(|| {
        format!(
            concat!(
                "The file `{}` could not be read.\n",
                "Please make sure that the config file is accessible and is properly formatted.",
            ),
            file.display()
        )
    })
}

fn load_from(file: &Path) -> Result<Config, Error> {
    match File::open(file) {
        Ok(mut cfg) => {
            let size = file.metadata().map(|m| m.len() as usize + 1).unwrap_or(0);
            let mut data = String::with_capacity(size);
            let _ = cfg.read_to_string(&mut data).map_err(Error::FileRead)?;
            let cfg = DeJson::deserialize_json(&data).map_err(Error::ConfigRead)?;
            Ok(cfg)
        }
        Err(e) => match e.kind() {
            ErrorKind::NotFound => Ok(Config::default()),
            ErrorKind::PermissionDenied => Err(Error::FileRead(e)),
            _ => Err(Error::Io(e)),
        },
    }
}

pub fn store(config: Config) -> Result<()> {
    let data = SerJson::serialize_json(&config);
    let data = data.into_bytes();
    let file = config_file().ok_or(Error::NoConfigDirectory)?;
    store_to(&file, data, true).wrap_err_with(|| {
        format!(
            concat!(
                "The file `{}` could not be written.\n",
                "Please make sure that the config file is accessible and writable.\n",
                "Config: {:?}"
            ),
            file.display(),
            config
        )
    })
}

fn store_to(file: &Path, data: Vec<u8>, create_parent: bool) -> Result<(), Error> {
    let mut f = match File::create(file) {
        Ok(f) => f,
        Err(e) => {
            return match e.kind() {
                ErrorKind::NotFound if create_parent => {
                    if let Some(parent) = file.parent() {
                        fs::create_dir_all(parent).map_err(Error::FileWrite)?;
                    }
                    store_to(file, data, false)
                }
                ErrorKind::PermissionDenied => Err(Error::FileWrite(e)),
                _ => Err(Error::Io(e)),
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
    FileRead(std::io::Error),
    FileWrite(std::io::Error),
    ConfigRead(nanoserde::DeJsonErr),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NoConfigDirectory => write!(f, "The configuration directoy could not be found"),
            Error::FileRead(_) => write!(f, "Could not read the configuration file"),
            Error::FileWrite(_) => write!(f, "Could not write the configuration file"),
            Error::ConfigRead(_) => write!(f, "Could not read the configuration data"),
            Error::Io(_) => write!(f, "IO error"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::FileRead(e) => Some(e),
            Error::FileWrite(e) => Some(e),
            Error::ConfigRead(e) => Some(e),
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::Io(e)
    }
}
