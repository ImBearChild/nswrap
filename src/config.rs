use getset::{CopyGetters, Getters, Setters};
use std::path::PathBuf;

#[derive(Getters, Setters, CopyGetters, Default, Clone)]
pub struct Root {
    #[getset(get = "pub", set = "pub")]
    path: PathBuf,

    #[getset(get = "pub", set = "pub")]
    readonly: Option<bool>,
}

#[derive(Getters, Setters, CopyGetters, Default, Clone)]
pub struct Mount {
    #[getset(get = "pub", set = "pub")]
    destination: PathBuf,
    #[getset(get = "pub", set = "pub")]
    typ: Option<String>,
    #[getset(get = "pub", set = "pub")]
    source: Option<PathBuf>,
    #[getset(get = "pub", set = "pub")]
    options: Option<Vec<String>>,
}

#[derive(Getters, Setters, CopyGetters, Default, Clone)]
/// Process contains information to start a specific application inside the
/// container.
pub struct Process {
    #[getset(get = "pub", set = "pub")]
    /// User specifies user information for the process.
    user: Option<User>,

    #[getset(get = "pub", set = "pub")]
    /// Args specifies the binary and arguments for the application to
    /// execute.
    args: Vec<String>,

    #[getset(get = "pub", set = "pub")]
    /// Env populates the process environment for the process.
    env: Option<Vec<String>>,

    #[getset(get = "pub", set = "pub")]
    /// Cwd is the current working directory for the process and must be
    /// relative to the container's root.
    cwd: PathBuf,
}

#[derive(Getters, Setters, CopyGetters, Default, Clone)]
pub struct User {
    #[getset(get_copy = "pub", set = "pub")]
    /// UID is the user id.
    uid: u32,

    #[getset(get_copy = "pub", set = "pub")]
    /// GID is the group id.
    gid: u32,
}

#[derive(Getters, Setters, CopyGetters, Default, Clone)]
/// LinuxIDMapping specifies UID/GID mappings.
pub struct LinuxIdMapping {
    #[getset(get_copy = "pub", set = "pub")]
    /// HostID is the starting UID/GID on the host to be mapped to
    /// `container_id`.
    host_id: u32,
    #[getset(get_copy = "pub", set = "pub")]
    /// ContainerID is the starting UID/GID in the container.
    container_id: u32,

    #[getset(get_copy = "pub", set = "pub")]
    /// Size is the number of IDs to be mapped.
    size: u32,
}
