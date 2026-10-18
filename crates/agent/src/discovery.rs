//! Private v1 rendezvous. The OS lock, rather than a PID, determines ownership.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discovery {
    pub version: u32,
    pub instance_id: uuid::Uuid,
    pub url: String,
    pub token: String,
}
pub fn runtime_dir() -> io::Result<PathBuf> {
    plotium_core::settings::config_dir()
        .map(|p| p.join("mcp"))
        .ok_or_else(|| io::Error::other("Could not locate Plotium user configuration directory"))
}
pub fn connector_path() -> io::Result<PathBuf> {
    plotium_core::settings::data_local_dir()
        .map(|p| {
            p.join("bin").join(if cfg!(windows) {
                "plotium-mcp.exe"
            } else {
                "plotium-mcp"
            })
        })
        .ok_or_else(|| io::Error::other("Could not locate Plotium user data directory"))
}
fn private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(dir)?;
        validate_owner(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(windows)]
    {
        fs::create_dir_all(dir)?;
        crate::windows_private::restrict(dir)?;
    }
    #[cfg(not(any(unix, windows)))]
    return Err(io::Error::other(
        "Local MCP discovery is unsupported on this platform",
    ));
    Ok(())
}
#[cfg(unix)]
fn validate_owner(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::symlink_metadata(path)?;
    // SAFETY: geteuid takes no pointers and has no preconditions.
    if meta.file_type().is_symlink() || meta.uid() != unsafe { libc::geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "MCP discovery must be owned by the current user and must not be a symlink",
        ));
    }
    Ok(())
}
impl Discovery {
    pub fn read(dir: &Path) -> io::Result<Self> {
        let path = dir.join("service.json");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for item in [dir, path.as_path()] {
                validate_owner(item)?;
                if fs::metadata(item)?.permissions().mode() & 0o077 != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "MCP discovery is accessible to other users",
                    ));
                }
            }
        }
        #[cfg(windows)]
        for item in [dir, path.as_path()] {
            crate::windows_private::validate(item)?;
        }
        let file = File::open(path)?;
        let value: Self = serde_json::from_reader(std::io::Read::take(file, 8192))?;
        let address = value
            .url
            .strip_prefix("http://")
            .and_then(|s| s.strip_suffix("/mcp"))
            .and_then(|s| s.parse::<std::net::SocketAddr>().ok());
        if value.version != 1
            || !address.is_some_and(|a| a.ip() == std::net::Ipv4Addr::LOCALHOST && a.port() != 0)
            || value.token.len() != 64
            || !value.token.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(io::Error::other("Invalid MCP v1 discovery file"));
        }
        Ok(value)
    }
}
pub(crate) struct Lease {
    dir: PathBuf,
    _lock: File,
    instance: Option<uuid::Uuid>,
}
impl Lease {
    #[cfg(test)]
    pub(crate) fn instance_for_test(&mut self, instance: Option<uuid::Uuid>) {
        self.instance = instance;
    }
    pub fn acquire(dir: PathBuf) -> io::Result<Self> {
        private_dir(&dir)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let lock = options.open(dir.join("service.lock"))?;
        lock.try_lock().map_err(|_| io::Error::other("Another Plotium instance owns MCP for this user; close its connection to release it"))?;
        Ok(Self {
            dir,
            _lock: lock,
            instance: None,
        })
    }
    pub fn publish(&mut self, discovery: &Discovery) -> io::Result<()> {
        let mut temp = tempfile::NamedTempFile::new_in(&self.dir)?;
        // Windows file ownership follows the token default, not the directory
        // owner. Restrict the actual file before writing or publishing a token.
        #[cfg(windows)]
        crate::windows_private::restrict(temp.path())?;
        serde_json::to_writer(&mut temp, discovery)?;
        temp.flush()?;
        temp.as_file().sync_all()?;
        temp.persist(self.dir.join("service.json"))
            .map_err(|e| e.error)?;
        self.instance = Some(discovery.instance_id);
        Ok(())
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(instance) = self.instance {
            match Discovery::read(&self.dir) {
                Ok(value) if value.instance_id == instance => {
                    if let Err(error) = fs::remove_file(self.dir.join("service.json")) {
                        eprintln!("Could not remove Plotium MCP discovery: {error}");
                    }
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    eprintln!("Could not inspect Plotium MCP discovery on shutdown: {error}")
                }
            }
        }
    }
}
