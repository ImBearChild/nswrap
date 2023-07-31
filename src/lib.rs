/*!
This crate is aiming at providing an friendly interface
of linux container technologies.
These technologies include system calls like `namespaces(7)` and `clone(2)`.
It can be use as a low-level library to configure and
execute program and closure inside linux containers.

The `Wrap` follows a similar builder pattern to std::process::Command.
In addition, `Wrap` contains methods to configure linux namespaces,
chroots, mount points, and more part specific to linux.
 */
#![deny(unsafe_op_in_unsafe_fn)]
#[macro_use]
extern crate derive_builder;
use getset::{CopyGetters, Getters, Setters};

use std::{
    collections::VecDeque,
    ffi::{OsStr, OsString},
    os::{fd::RawFd, unix::process::ExitStatusExt},
};
pub mod config;
pub mod core;
pub mod error;
pub mod util;
extern crate xdg;

use crate::error::Error;

pub use crate::core::WrapCbBox;

/// Main class of spawn process and execute functions.
#[derive(Getters, Setters, CopyGetters, Default)]
pub struct Wrap<'a> {
    process: config::Process,
    root: Option<config::Root>,

    mounts: Vec<config::Mount>,
    uid_maps: Vec<config::IdMap>,
    gid_maps: Vec<config::IdMap>,
    callbacks: VecDeque<WrapCbBox<'a>>,

    namespace_nsenter: config::NamespaceSet,
    namespace_unshare: config::NamespaceSet,

    sandbox_mnt: bool,
}

/// The reference to the running child.
pub struct Child {
    pid: rustix::process::Pid,
}

/// Exit status of the child.
pub struct ExitStatus {
    wait_status: rustix::process::WaitStatus,
    std_exit_status: std::process::ExitStatus,
}

/// Core implementation
impl<'a> Wrap<'a> {
    /// Create a new instance with default.
    pub fn new() -> Self {
        Default::default()
    }

    /// Create a new instance with program to execute.
    pub fn new_program<S: AsRef<OsStr>>(program: S) -> Self {
        let mut s = Self::new();
        let mut config = config::Process::default();
        config.set_bin(OsString::from(program.as_ref()));
        s.set_process(config);
        s
    }

    /// Set command to execute
    pub fn program<S: AsRef<OsStr>>(&mut self, program: S) -> &mut Self {
        self.process.set_bin((&program).into());
        self
    }

    /// Adds an argument to pass to the program.
    ///
    /// Only one argument can be passed per use.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.process.args.push((&arg).into());
        self
    }

    /// Adds multiple arguments to pass to the program.
    ///
    /// To pass a single argument see [`Self::arg()`].
    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg.as_ref());
        }
        self
    }

    /// Executes the callbacks and program in a child process,
    /// returning a handle to it.
    ///
    /// This instance of Wrap will not be consumed, but it's
    /// queue of callback functions will be empty.
    pub fn spawn(&mut self) -> Result<Child, Error> {
        let mut wrapcore = core::WrapInner {
            process: self.process.clone(),
            root: self.root.clone(),
            mounts: self.mounts.clone(),
            uid_maps: self.uid_maps.clone(),
            gid_maps: self.gid_maps.clone(),
            callbacks: VecDeque::new(),
            namespace_nsenter: self.namespace_nsenter.clone(),
            namespace_unshare: self.namespace_unshare.clone(),
            sandbox_mnt: self.sandbox_mnt.clone(),
        };
        wrapcore.callbacks.append(&mut self.callbacks);
        Self::spawn_inner(wrapcore)
    }

    /// Executes the command and callback functions in a child process,
    /// waiting for it to finish and collecting its status.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    pub fn status(&mut self) -> Result<ExitStatus, Error> {
        self.spawn()?.wait()
    }

    /**
    Add a callback to run in the child before execute the program.

    This function can be called multiple times, all functions will be
    called after `clone(2)` and environment setup, in the same order
    as they were added.
    The return value of last function can be retrieved
    from `ExitStatus` if no `program` is executed.

    # Notes and Safety

    This closure will be run in the context of the child process after a
    `clone(2)`. This primarily means that any modifications made to
    memory on behalf of this closure will **not** be visible to the
    parent process.

    This method will not cause memory corruption,
    but it will be risky when interact with thread-related components.
    For example, it's possible to create a deadlock using `std::sync::Mutex`.
    Because `clone(2)` clone whole process,
    they do not share any modified memory area.
    Child process is not thread, and should not be threat like thread.

    Use `pipe(2)` or other IPC method to communicate with child process.

    For further details on this topic, please refer to the
    [Rust Std Lib Document], related [GitHub issue of nix library],
    [GitHub issue of rust]
    and the equivalent documentation for any targeted
    platform, especially the requirements around *async-signal-safety*.

    [Rust Std Lib Document]:
        https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#tymethod.pre_exec
    [GitHub issue of nix library]:
        https://github.com/nix-rust/nix/issues/360#issuecomment-359271308
    [GitHub issue of rust]:
        https://github.com/rust-lang/rust/issues/39575
    */
    pub fn callback<F>(&mut self, cb: F) -> &mut Self
    where
        F: FnOnce() -> isize + Send + 'static,
    {
        self.callbacks.push_back(Box::new(cb));
        self
    }

    /// Set new `namespace(7)` for child process.
    ///
    /// ```
    /// use nswrap::Wrap;
    /// use nswrap::config;
    /// let mut wrap = Wrap::new();
    /// wrap.callback(|| {print!("Cool!");return 5})
    ///     .unshare(config::NamespaceType::User);
    /// wrap.spawn().unwrap().wait().unwrap();
    /// ```
    pub fn unshare(&mut self, typ: config::NamespaceType) -> &mut Self {
        self.add_namespace(typ, config::NamespaceItem::Unshare)
    }

    /// Reassociate child process with a namespace.
    ///
    /// The order in which this method is called will affect the result.
    pub fn nsenter(&mut self, typ: config::NamespaceType, pidfd: RawFd) -> &mut Self {
        self.add_namespace(typ, config::NamespaceItem::Enter(pidfd))
    }

    /// Add some mount points and file path that application usually needs.
    ///
    /// This will require a mount namespace.
    ///
    /// The Linux ABI includes both syscalls and several special file paths.
    /// Applications expecting a Linux environment will very likely expect
    /// these file paths to be set up correctly.
    /// Please refer to Linux parts of OCI Runtime Specification for
    /// more information.
    pub fn abi_fs(self) -> Self {
        todo!()
    }

    /// Sets user id mappings for new process.
    ///
    /// Each call to this function will add an item in `/proc/{pid}/uid_map`.
    pub fn uid_map(&mut self, host_id: u32, container_id: u32, size: u32) -> &mut Self {
        self.add_uid_map(config::IdMap {
            host_id,
            container_id,
            size,
        })
    }

    /// Sets group id mappings for new process.
    ///
    /// Each call to this function will add an item in `/proc/{pid}/gid_map`.
    pub fn gid_map(&mut self, host_id: u32, container_id: u32, size: u32) -> &mut Self {
        self.add_gid_map(config::IdMap {
            host_id,
            container_id,
            size,
        })
    }

    /// Use some preset to set id mapping in container.
    pub fn id_map_preset(&mut self, set: config::IdMapPreset) -> &mut Self {
        match set {
            config::IdMapPreset::Root => {
                self.uid_map(util::get_uid(), 0, 1);
                self.gid_map(util::get_gid(), 0, 1)
            }
            config::IdMapPreset::Current => {
                self.uid_map(util::get_uid(), util::get_uid(), 1);
                self.gid_map(util::get_gid(), util::get_gid(), 1)
            }
            config::IdMapPreset::Auto => todo!(),
        }
    }

    /// Simulate brwrap's behaviour, use a tmpfs as root dir
    /// inside namespace.
    ///
    /// This is required if a user what to use `Wrap` for mountpoint
    /// management
    pub fn sandbox_mnt(&mut self, opt: bool) -> &mut Self {
        self.sandbox_mnt = opt;
        self
    }
}

/// Public builder pattern method
impl<'a> Wrap<'_> {
    fn add_namespace(
        &mut self,
        typ: config::NamespaceType,
        ns: config::NamespaceItem,
    ) -> &mut Self {
        let mut set;
        match ns {
            config::NamespaceItem::None => return self,
            config::NamespaceItem::Unshare => set = &mut self.namespace_unshare,
            config::NamespaceItem::Enter(_) => set = &mut self.namespace_nsenter,
        }
        match typ {
            config::NamespaceType::Mount => set.mount = ns,
            config::NamespaceType::Cgroup => set.cgroup = ns,
            config::NamespaceType::Uts => set.uts = ns,
            config::NamespaceType::Ipc => set.ipc = ns,
            config::NamespaceType::User => set.user = ns,
            config::NamespaceType::Pid => set.pid = ns,
            config::NamespaceType::Network => set.network = ns,
            config::NamespaceType::Time => unimplemented!(),
        }
        return self;
    }

    /// Set the program that will be executed.
    ///
    /// If the user only wants to execute the callback functions,
    /// this function does not have to be called.
    fn set_process(&mut self, proc: config::Process) -> &mut Self {
        self.process = proc.into();
        self
    }

    fn set_root(&mut self, root: config::Root) -> &mut Self {
        self.root = Some(root);
        self
    }

    /// Add mount point
    fn add_mount(&mut self, mnt: config::Mount) -> &mut Self {
        self.mounts.push(mnt);
        self
    }

    /// Add uidmap
    fn add_uid_map(&mut self, id_map: config::IdMap) -> &mut Self {
        self.uid_maps.push(id_map);
        self
    }
    /// Add gidmap
    fn add_gid_map(&mut self, id_map: config::IdMap) -> &mut Self {
        self.gid_maps.push(id_map);
        self
    }
}

impl Child {
    pub fn wait(&mut self) -> Result<ExitStatus, Error> {
        match rustix::process::waitpid(Some(self.pid), rustix::process::WaitOptions::empty()) {
            Ok(r) => Ok(ExitStatus::new(r.unwrap())),
            Err(err) => Err(Error::OsErrno(err.raw_os_error())),
        }
    }
}

impl ExitStatus {
    pub fn new(wait_status: rustix::process::WaitStatus) -> Self {
        Self {
            wait_status,
            std_exit_status: std::process::ExitStatus::from_raw(
                wait_status.as_raw().try_into().unwrap(),
            ),
        }
    }

    pub fn code(&self) -> Option<i32> {
        match self.wait_status.exit_status() {
            Some(r) => Some(i32::try_from(r).unwrap()),
            None => None,
        }
    }

    pub fn success(&self) -> bool {
        self.std_exit_status.success()
    }

    pub fn signal(&self) -> Option<i32> {
        self.std_exit_status.signal()
    }
    pub fn core_dumped(&self) -> bool {
        self.std_exit_status.core_dumped()
    }
    pub fn stopped_signal(&self) -> Option<i32> {
        self.std_exit_status.stopped_signal()
    }
    pub fn continued(&self) -> bool {
        self.std_exit_status.continued()
    }
}

#[cfg(test)]
mod tests {


}
