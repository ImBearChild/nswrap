//! This crate is aiming at providing an friendly interface
//! of linux container technologies. These technologies includes
//! system calls like `namespaces(7)` and `clone(2)`.
//! It can be use as a low-level library to configure and
//! execute program and closure inside linux containers.
//!
//! The `Wrap` follows a similar builder pattern to std::process::Command.
//! In addition `Wrap` contains methods to configure linux
//! namespaces, chroots and more part specific to linux.
#[macro_use]
extern crate derive_builder;
use getset::{CopyGetters, Getters, Setters};

use std::{
    collections::VecDeque,
    ffi::{OsStr, OsString},
    os::fd::RawFd,
};
pub mod config;
pub mod core;
pub mod error;
pub mod util;
extern crate xdg;

use crate::error::Error;

/// Boxed closure to execute in child process
pub type WrapCbBox<'a> = Box<dyn FnOnce() -> isize + 'a>;

/// Main class of spawn process and execute functions.
#[derive(Getters, Setters, CopyGetters, Default)]
pub struct Wrap<'a> {
    process: Option<config::Process>,
    root: Option<config::Root>,

    namespaces: VecDeque<config::Namespace>,
    mounts: Vec<config::Mount>,
    uid_maps: Vec<config::IdMap>,
    gid_maps: Vec<config::IdMap>,
    callbacks: VecDeque<WrapCbBox<'a>>,

    sandbox_mnt: bool,
    in_child: bool,
}

/// The reference to the running child.
pub struct Child {
    pid: nix::unistd::Pid,
}

/// Exit status of the child.
pub struct ExitStatus {
    wait_status: nix::sys::wait::WaitStatus,
}

/// Core implementation
impl<'a> Wrap<'a> {
    /// Create a new instance with defualt.
    pub fn new() -> Self {
        Default::default()
    }

    pub fn new_cmd<S: AsRef<OsStr>>(program: S) -> Self {
        let mut s = Self::new();
        let mut config = config::Process::default();
        config.set_bin(OsString::from(program.as_ref()));
        s.set_process(config);
        s
    }

    /// Executes the callbacks and program in a child process,
    /// returning a handle to it.
    pub fn spawn(&mut self) -> Result<Child, Error> {
        self.spwan_inner()
    }

    /// Add a callback to run in the child before execute the program.
    ///
    /// This function can be called multiple times, all functions will be
    /// called after `clone(2)` and environment setup, in the same order
    /// as they were added.
    /// The return value of last function can be retrieved
    /// from `ExitStatus` if no `program` is executed.
    ///
    /// # Notes and Safety
    ///
    /// This closure will be run in the context of the child process after a
    /// `clone(2)`. This primarily means that any modifications made to
    /// memory on behalf of this closure will **not** be visible to the
    /// parent process.
    ///
    /// For further details on this topic, please refer to the
    /// [Rust Std Lib Dcoument], related [github issue of nix library],
    /// [github issue of rust]
    /// and the equivalent documentation for any targeted
    /// platform, especially the requirements around *async-signal-safety*.
    ///
    /// [Rust Std Lib Dcoument]:
    ///     https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#tymethod.pre_exec
    /// [github issue of nix library]:
    ///     https://github.com/nix-rust/nix/issues/360#issuecomment-359271308
    /// [github issue of rust]:
    ///     https://github.com/rust-lang/rust/issues/39575
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
    ///
    /// The order in which this method is called will affect the result.
    /// Refer to [`Self::add_namespace()`] for more information.
    pub fn unshare(&mut self, typ: config::NamespaceType) -> &mut Self {
        self.add_namespace(config::Namespace { typ, fd: None })
    }

    /// Reassociate child process with a namespace.
    ///
    /// The order in which this method is called will affect the result.
    ///
    /// # Panics
    /// 
    /// This method *can not* be called once [`Self::unshare()`] is called,
    /// otherwise it will panic.
    /// Refer to [`Self::add_namespace()`] for more information.
    pub fn nsenter(&mut self, typ: config::NamespaceType, pidfd: RawFd) -> &mut Self {
        self.add_namespace(config::Namespace {
            typ,
            fd: Some(pidfd),
        })
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
    /// Set namespace for child process. This method accept structre
    /// including file descriptors to enter existing namespace.
    ///
    /// # Panics
    /// 
    /// Generally speaking, Wrap will execute `setns(2)` 
    /// and `clone(2)` (or `unshre(2)`, 
    /// depending on the implementation detail) in the order 
    /// you add call [`Self::nsenter()`] or [`Self::add_namespace()`]. 
    /// 
    /// Child can enter a existing namespace and create 
    /// new namespace in it (making nested namespace), but it makes no sense 
    /// to enter a namespace inside a new namespace.
    /// So this method will panic.
    /// 
    /// ```should_panic
    /// use nswrap::{Wrap,config};
    /// let mut w = Wrap::new();
    /// w.unshare(config::NamespaceType::Cgroup);
    /// w.nsenter(config::NamespaceType::Cgroup, 114);
    /// ```
    ///
    /// Use `self.unshare()` if you only want new namespace.
    pub fn add_namespace(&mut self, ns: config::Namespace) -> &mut Self {
        if ns.fd().is_some() {
            match self.namespaces.back() {
                Some(n) => match n.fd() {
                    Some(_) => (),
                    None => panic!("Method misuse!"),
                },
                None => (),
            }
        }

        self.namespaces.push_back(ns);
        self
    }

    /// Set the program that will be executed.
    ///
    /// If the user only wants to execute the callback functions,
    /// this function does not have to be called.
    pub fn set_process(&mut self, proc: config::Process) -> &mut Self {
        self.process = Some(proc);
        self
    }

    pub fn set_root(&mut self, root: config::Root) -> &mut Self {
        self.root = Some(root);
        self
    }

    /// Add mount point
    pub fn add_mount(&mut self, mnt: config::Mount) -> &mut Self {
        self.mounts.push(mnt);
        self
    }

    /// Add uidmap
    pub fn add_uid_map(&mut self, id_map: config::IdMap) -> &mut Self {
        self.uid_maps.push(id_map);
        self
    }
    /// Add gidmap
    pub fn add_gid_map(&mut self, id_map: config::IdMap) -> &mut Self {
        self.gid_maps.push(id_map);
        self
    }
}

impl Child {
    pub fn wait(&mut self) -> Result<ExitStatus, Error> {
        match nix::sys::wait::waitpid(self.pid, None) {
            Ok(r) => Ok(ExitStatus::new(r)),
            Err(err) => Err(Error::NixErrno(err)),
        }
    }
}

impl ExitStatus {
    pub fn new(wait_status: nix::sys::wait::WaitStatus) -> Self {
        Self { wait_status }
    }

    pub fn code(&self) -> Option<i32> {
        match self.wait_status {
            nix::sys::wait::WaitStatus::Exited(_, ret) => Some(ret),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    const _TMP_DIR: &str = "/tmp/nswrap.test/";
    const _TMP_DIR1: &str = "/tmp/nswrap.test/test-1";
    const _TMP_DIR2: &str = "/tmp/nswrap.test/test-2";

    fn make_test_dir() {
        use std::fs;
        fs::remove_dir_all(_TMP_DIR);
        fs::create_dir_all(_TMP_DIR1).unwrap();
        fs::create_dir_all(_TMP_DIR2).unwrap();
    }

    #[test]
    fn bind_mount() {
        use std::fs::File;
        use std::io::prelude::*;
        make_test_dir();
        let cb = || {
            use config::NamespaceType;
            use std::fs::File;
            let mut v = Vec::new();
            v.push(NamespaceType::Mount);
            util::unshare(v).unwrap();
            let mut flags = nix::mount::MsFlags::empty();
            flags.set(nix::mount::MsFlags::MS_BIND, true);
            nix::mount::mount(Some(_TMP_DIR1), _TMP_DIR2, Some(""), flags, Some("")).unwrap();
            let mut file = File::create(_TMP_DIR2.to_owned() + "/foo.txt").unwrap();
            std::io::Write::write_all(&mut file, b"Hello, world!").unwrap();
            return 0;
        };
        let mut wrap = Wrap::new();
        wrap.callback(cb).unshare(config::NamespaceType::User);
        wrap.spawn().unwrap().wait().unwrap();

        // Check Result
        let mut file = File::open(_TMP_DIR1.to_owned() + "/foo.txt").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "Hello, world!");
    }

    #[test]
    fn callback_return_value() {
        let cb = || {
            return 16;
        };
        let mut wrap = Wrap::new();
        wrap.callback(cb).unshare(config::NamespaceType::User);
        let ret = wrap.spawn().unwrap().wait().unwrap().code().unwrap();
        assert_eq!(16, ret);
    }

    #[test]
    fn callback_return_value_in_thread() {
        use std::thread;

        let thread_join_handle = thread::spawn(move || {
            let cb = || {
                return 16;
            };
            let mut wrap = Wrap::new();
            wrap.callback(cb).unshare(config::NamespaceType::User);
            let ret = wrap.spawn().unwrap().wait().unwrap().code().unwrap();
            ret
        });

        assert_eq!(thread_join_handle.join().unwrap(), 16);
    }

    #[test]
    fn tmpfs_root() {
        let cb = || {
            use config::NamespaceType;
            use std::path::Path;
            let mut v = Vec::new();
            v.push(NamespaceType::Mount);
            util::unshare(v).unwrap();
            let mut flags = nix::mount::MsFlags::empty();
            nix::mount::mount(Some(""), "/", Some("tmpfs"), flags, Some("")).unwrap();
            let p = Path::new("/bin/sh");
            match p.exists() {
                true => return 16,
                false => return 32,
            };
        };
        let mut binding = Wrap::new();
        let wrap = binding
            .callback(cb)
            .unshare(config::NamespaceType::User)
            .sandbox_mnt(true)
            .id_map_preset(config::IdMapPreset::Current);
        let ret = wrap.spawn().unwrap().wait().unwrap().code().unwrap();
        assert_eq!(32, ret);
    }

    #[test]
    #[should_panic]
    fn misuse_add_namespace() {
        let mut w = Wrap::new();
        w.unshare(config::NamespaceType::Cgroup);
        w.nsenter(config::NamespaceType::Cgroup, 114);
    }
}
