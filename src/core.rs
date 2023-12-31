#![doc(hidden)]

use std::{
    collections::VecDeque,
    ffi::{OsStr, OsString},
    fs::OpenOptions,
    os::unix::prelude::OsStrExt,
};

use crate::util::CloneFlags;
use crate::{config, util, Child, Error};
//use getset::{CopyGetters, Getters, Setters};

/// Default stack size
///
/// https://wiki.musl-libc.org/functional-differences-from-glibc.html
const STACK_SIZE: usize = 122880;

/// Boxed closure to execute in child process
pub type WrapCbBox<'a> = Box<dyn FnOnce() -> isize + 'a>;

impl crate::Wrap<'_> {
    pub(crate) fn spawn_inner(mut wrap: WrapInner) -> Result<Child, Error> {
        let mut p: Box<[u8; STACK_SIZE]> = Box::new([0; STACK_SIZE]);

        let pid = match unsafe {
            crate::util::clone(
                Box::new(move || -> isize { wrap.run_child() }),
                &mut *p,
                util::CloneFlags::empty(),
                Some(libc::SIGCHLD),
            )
        } {
            Ok(it) => it,
            Err(e) => return Err(e),
        };

        Ok(Child {
            pid: unsafe { rustix::process::Pid::from_raw_unchecked(pid.try_into().unwrap()) },
        })
    }
}

//#[derive(Getters, Setters, CopyGetters, Default)]
pub(crate) struct WrapInner<'a> {
    pub(crate) process: config::Process,
    pub(crate) root: Option<config::Root>,

    pub(crate) mounts: Vec<config::Mount>,
    pub(crate) uid_maps: Vec<config::IdMap>,
    pub(crate) gid_maps: Vec<config::IdMap>,
    pub(crate) callbacks: VecDeque<WrapCbBox<'a>>,

    pub(crate) namespace_nsenter: config::NamespaceSet,
    pub(crate) namespace_unshare: config::NamespaceSet,

    pub(crate) sandbox_mnt: bool,
}

impl WrapInner<'_> {
    fn run_child(&mut self) -> isize {
        self.apply_nsenter();
        self.apply_unshare();

        // Drop mmap and fd?

        if (self.uid_maps.len() + self.gid_maps.len()) > 0 {
            self.set_id_map();
        }

        if self.sandbox_mnt {
            self.set_up_tmpfs_cwd();
        }

        let ret = self.execute_callbacks();

        if !self.process.bin.is_empty() {
            self.execute_process(); // exec ,no return
        }

        return ret;
    }

    pub(crate) fn execute_process(&mut self) {
        use std::os::unix::process::CommandExt;
        use std::process::Command;

        let mut cmd = Command::new(self.process.bin());
        cmd.args(self.process.args());

        match &self.process.cwd {
            Some(cwd) => {cmd.current_dir(cwd);},
            None => (),
        }

        if self.process.env_no_inheriting {
            //cmd.env_clear();
        }

        // Set up envvar
        for (key, val) in &self.process.env {
            match val {
                config::EnvVarItem::Set(val) => {
                    cmd.env(key, val);
                }
                config::EnvVarItem::Clean => {
                    cmd.env_remove(key);
                }
            }
        }

        cmd.exec();
    }

    pub(crate) fn apply_nsenter(&mut self) {
        Self::apply_namespace_item(self.namespace_nsenter.user, CloneFlags::NEWUSER);
        Self::apply_namespace_item(self.namespace_nsenter.mount, CloneFlags::NEWNS);
        Self::apply_namespace_item(self.namespace_nsenter.cgroup, CloneFlags::NEWCGROUP);
        Self::apply_namespace_item(self.namespace_nsenter.uts, CloneFlags::NEWUTS);
        Self::apply_namespace_item(self.namespace_nsenter.ipc, CloneFlags::NEWIPC);
        Self::apply_namespace_item(self.namespace_nsenter.pid, CloneFlags::NEWPID);
        Self::apply_namespace_item(self.namespace_nsenter.network, CloneFlags::NEWNET);
    }

    pub(crate) fn apply_unshare(&mut self) {
        Self::apply_namespace_item(self.namespace_unshare.user, CloneFlags::NEWUSER);
        Self::apply_namespace_item(self.namespace_unshare.mount, CloneFlags::NEWNS);
        Self::apply_namespace_item(self.namespace_unshare.cgroup, CloneFlags::NEWCGROUP);
        Self::apply_namespace_item(self.namespace_unshare.uts, CloneFlags::NEWUTS);
        Self::apply_namespace_item(self.namespace_unshare.ipc, CloneFlags::NEWIPC);
        Self::apply_namespace_item(self.namespace_unshare.pid, CloneFlags::NEWPID);
        Self::apply_namespace_item(self.namespace_unshare.network, CloneFlags::NEWNET);
    }

    fn apply_namespace_item(ns: config::NamespaceItem, flag: CloneFlags) {
        match ns {
            config::NamespaceItem::None => (),
            config::NamespaceItem::Unshare => {
                crate::util::unshare(flag).unwrap();
            }
            config::NamespaceItem::Enter(fd) => {
                crate::util::setns(fd, flag).unwrap();
            }
        }
    }

    pub(crate) fn write_id_map<S: AsRef<OsStr>>(file: S, map: &Vec<config::IdMap>) {
        let file = OpenOptions::new().write(true).open(file.as_ref()).unwrap();
        let mut content = OsString::new();
        for i in map {
            content.push(format!("{}", i.container_id()));
            content.push(" ");
            content.push(format!("{}", i.host_id()));
            content.push(" ");
            content.push(format!("{}\n", i.size()));
        }
        rustix::io::write(file, content.as_bytes()).unwrap();
    }

    pub(crate) fn set_id_map(&self) {
        let pid = util::get_pid();
        Self::write_id_map(format!("/proc/{}/uid_map", pid), &self.uid_maps);

        // Write /proc/pid/setgroups before wite /proc/pid/gid_map, or it will fail.
        // See https://manpages.opensuse.org/Tumbleweed/man-pages/user_namespaces.7.en.html
        let file = OpenOptions::new()
            .write(true)
            .open(format!("/proc/{}/setgroups", pid))
            .unwrap();
        rustix::io::write(file, b"deny").unwrap();

        Self::write_id_map(format!("/proc/{}/gid_map", pid), &self.uid_maps);
    }

    pub(crate) fn execute_callbacks(&mut self) -> isize {
        let mut ret = 0;
        for _i in 0..self.callbacks.len() {
            ret = self.callbacks.pop_front().unwrap()();
        }
        return ret;
    }

    /**
    Create tmpfs as root, simulate brwrap's behaviour.

    Due to kernel bug#183461 ,this can only be called after setup uid
    and gid mapping.
    */
    pub(crate) fn set_up_tmpfs_cwd(&self) {
        use nix::unistd::pivot_root;
        use rustix::fs::change_mount;
        use rustix::fs::mount;
        use rustix::fs::MountFlags;
        use rustix::fs::MountPropagationFlags;
        use std::env::set_current_dir;
        use std::fs::DirBuilder;
        use std::os::unix::fs::DirBuilderExt;

        let tmp_path = "/tmp";
        //
        change_mount(
            "/",
            MountPropagationFlags::SLAVE | MountPropagationFlags::REC,
            // TODO: Fix MountPropagationFlags::SILENT
        )
        .unwrap();

        mount(
            "tmpfs",
            tmp_path,
            "tmpfs",
            MountFlags::NODEV | MountFlags::NOSUID,
            "",
        )
        .unwrap();

        set_current_dir(tmp_path).unwrap();

        let mut dir = DirBuilder::new();
        dir.mode(0o755);
        dir.create("/tmp/newroot").unwrap();
        dir.create("oldroot").unwrap();
        mount(
            "newroot",
            "newroot",
            "",
            MountFlags::SILENT | MountFlags::BIND | MountFlags::REC,
            "",
        )
        .unwrap();

        pivot_root(tmp_path, "oldroot").unwrap(); // todo: Clean this!
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn test() {
        crate::Wrap::new_program("/bin/sh");
    }
}
