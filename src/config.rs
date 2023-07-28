use getset::{CopyGetters, Getters, Setters};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Default, Clone, Copy)]
pub enum NamespaceType {
    Mount,
    Cgroup,
    Uts,
    Ipc,
    #[default]
    User,
    Pid,
    Network,
    Time,
}

#[derive(Default, Clone, Copy)]
pub enum NamespaceItem {
    #[default]
    None,
    Unshare,
    Enter(std::os::fd::RawFd),
}

#[derive(Getters, Setters, CopyGetters, Default, Clone)]
pub struct NamespaceSet {
    pub(crate) user: NamespaceItem,
    pub(crate) mount: NamespaceItem,
    pub(crate) cgroup: NamespaceItem,
    pub(crate) uts: NamespaceItem,
    pub(crate) ipc: NamespaceItem,
    pub(crate) pid: NamespaceItem,
    pub(crate) network: NamespaceItem,
    //pub(crate) time: NamespaceItem,
}

#[derive(Builder, Getters, Setters, CopyGetters, Default, Clone)]
pub struct Root {
    #[getset(get = "pub", set = "pub")]
    path: PathBuf,

    #[getset(get = "pub", set = "pub")]
    readonly: Option<bool>,
}

#[derive(Builder, Getters, Setters, CopyGetters, Default, Clone)]
pub struct Mount {
    #[getset(get = "pub", set = "pub")]
    destination: PathBuf,
    // Path values for bind mounts are either absolute or relative to the
    // bundle. A mount is a bind mount if it has either bind or rbind in the options.
    #[getset(get = "pub", set = "pub")]
    typ: Option<String>,
    #[getset(get = "pub", set = "pub")]
    source: Option<PathBuf>,
    #[getset(get = "pub", set = "pub")]
    options: Option<Vec<String>>,
}

#[derive(Builder, Getters, Setters, CopyGetters, Default, Clone)]
/// Process contains information to start a specific application inside the
/// container.
pub struct Process {
    #[getset(get = "pub", set = "pub")]
    /// User specifies user information for the process.
    user: Option<User>,

    #[getset(get = "pub", set = "pub")]
    pub(crate) bin: OsString,

    #[getset(get = "pub", set = "pub")]
    /// Args specifies the arguments for the application to
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

#[derive(Builder, Getters, Setters, CopyGetters, Default, Clone)]
pub struct User {
    #[getset(get_copy = "pub", set = "pub")]
    /// UID is the user id.
    uid: u32,

    #[getset(get_copy = "pub", set = "pub")]
    /// GID is the group id.
    gid: u32,
}

#[derive(Builder, Getters, Setters, CopyGetters, Default, Clone)]
/// LinuxIDMapping specifies UID/GID mappings.
pub struct IdMap {
    #[getset(get_copy = "pub", set = "pub")]
    /// HostID is the starting UID/GID on the host to be mapped to
    /// `container_id`.
    pub(crate) host_id: u32,
    #[getset(get_copy = "pub", set = "pub")]
    /// ContainerID is the starting UID/GID in the container.
    pub(crate) container_id: u32,

    #[getset(get_copy = "pub", set = "pub")]
    /// Size is the number of IDs to be mapped.
    pub(crate) size: u32,
}

pub enum IdMapPreset {
    Root,
    Current,
    Auto,
}
