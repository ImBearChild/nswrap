//! This crate is aiming at providing an friendly interface
//! of linux container technologies. These technologies includes
//! system calls like `namespaces(7)` and `clone(2)`.
//! It can be use as a low-level library
//! to configure and execute program inside linux containers.
//!
//! The `Wrap` has mostly same API as std::process::Command.
//! In addition `Wrap` contains methods to configure linux
//! namespaces, chroots and more part specific to linux.

use std::ffi::{OsStr, OsString};
pub mod config;
pub mod error;
pub mod util;
extern crate xdg;
use crate::error::Error;

/// Default value is 80k, taken from older musl-libc
/// https://wiki.musl-libc.org/functional-differences-from-glibc.html
const STACK_SIZE: usize = 81920;

pub type WrapCb<'a> = Box<dyn FnMut() -> isize + 'a>;

/// Main class of spawn process and execute functions.
#[derive(Default)]
pub struct Wrap<'a> {
    process: Option<config::Process>,
    root: Option<config::Root>,
    namespaces: Vec<config::Namespace>,
    mounts: Vec<config::Mount>,

    callbacks: Vec<WrapCb<'a>>,
}

/// The reference to the running child.
pub struct Child {
    pid: nix::unistd::Pid,
}

pub struct ExitStatus {
    wait_status: nix::sys::wait::WaitStatus,
}

impl<'a> Wrap<'a> {
    /// Create a new instance with defualt.
    pub fn new() -> Self {
        Default::default()
    }

    /// Set the path to the program that will be executed.
    ///
    /// If the user only wants to execute the callback functions,
    /// this function does not have to be called.
    pub fn program<S: AsRef<OsStr>>(self, program: S) -> Self {
        todo!()
    }

    pub fn set_program(self, proc: config::Process) -> Self {
        todo!()
    }

    /// Add a callback to run in the child before execute the program.
    ///
    /// This function can be called multiple times, all functions will be
    /// called after `clone(2)` and environment setup, in the same order
    /// as they were added.
    /// The return value of last function can be retrieved
    /// from `ExitStatus` if no `program` is executed.
    pub fn callback(mut self, cb: WrapCb<'a>) -> Self {
        self.callbacks.push(cb);
        self
    }

    /// Executes the callbacks and program in a child process,
    /// returning a handle to it.
    pub fn spawn(mut self) -> Result<Child, Error> {
        use nix::sched::CloneFlags;
        let mut p: Box<[u8; STACK_SIZE]> = Box::new([0; STACK_SIZE]);
        //let mut num = 0;
        let pid = match nix::sched::clone(
            Box::new(|| {
                let mut ret = 0;
                for _i in 0..self.callbacks.len() {
                    ret = self.callbacks.pop().unwrap()();
                }
                return ret;
            }),
            &mut *p,
            {
                let mut n = nix::sched::CloneFlags::empty();
                n.set(CloneFlags::CLONE_NEWUSER, true); // TODO: Setup Flags correctly
                n
            },
            Some(libc::SIGCHLD),
        ) {
            Ok(it) => it,
            Err(err) => return Err(Error::NixErrno(err)),
        };
        Ok(Child { pid })
    }

    /// Set new `namespace(7)` for child process.
    /// ```
    /// use nswrap::Wrap;
    /// use nswrap::config;
    /// let wrap = Wrap::new().callback(Box::new(|| {print!("Cool!");return 0})).namespace(config::NamespaceType::User);
    /// wrap.spawn().unwrap().wait().unwrap();
    /// ```
    pub fn namespace(self, typ: config::NamespaceType) -> Self {
        self.add_namespace(config::Namespace { typ, fd: None })
    }

    /// Set namespace for child process. You can use file descriptors to
    /// enter existing namespace.
    pub fn add_namespace(mut self, ns: config::Namespace) -> Self {
        self.namespaces.push(ns);
        self
    }

    /// Add some mount points and file path that application usually needs.
    /// 
    /// This will crate a new mount namespace.
    ///
    /// The Linux ABI includes both syscalls and several special file paths.
    /// Applications expecting a Linux environment will very likely expect
    /// these file paths to be set up correctly.
    /// Please refer to Linux parts of OCI Runtime Specification for
    /// more information.
    pub fn linux_abi_fs(self) -> Self {
        todo!()
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
            _ => None
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
    fn thread_unshare() {
        use std::fs::File;
        use std::io::prelude::*;
        make_test_dir();
        let cb = || {
            use std::fs::File;
            use config::NamespaceType;
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
        let wrap = Wrap::new()
            .callback(Box::new(cb))
            .namespace(config::NamespaceType::User);
        wrap.spawn().unwrap().wait().unwrap();

        // Check Result
        let mut file = File::open(_TMP_DIR1.to_owned() + "/foo.txt").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "Hello, world!");
    }

    #[test]
    fn tmpfs_root(){
        let cb = || {
            use std::fs::File;
            use std::path::Path;
            use config::NamespaceType;
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
        let wrap = Wrap::new()
            .callback(Box::new(cb))
            .namespace(config::NamespaceType::User);
        let ret = wrap.spawn().unwrap().wait().unwrap().code().unwrap();
        assert_eq!(32,ret);
    }
}
