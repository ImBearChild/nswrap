#![doc(hidden)]

use std::{
    ffi::{OsStr, OsString},
    fs::OpenOptions,
    os::{fd::IntoRawFd, unix::prelude::OsStrExt},
};

use crate::{config, util, Child, Error};
use nix::sched::CloneFlags;

/// Default value is 120k
///
/// https://wiki.musl-libc.org/functional-differences-from-glibc.html
const STACK_SIZE: usize = 122880;

impl crate::Wrap<'_> {
    pub(crate) fn spwan_inner(&mut self) -> Result<Child, Error> {
        
        self.apply_nsenter();
        let mut p: Box<[u8; STACK_SIZE]> = Box::new([0; STACK_SIZE]);
        let pid = match nix::sched::clone(
            Box::new(|| {
                // Drop mmap and fd?

                // setup a flag here, prevent calling some method without clone(2)
                self.in_child = true;

                if (self.uid_maps.len() + self.gid_maps.len()) > 0 {
                    self.set_id_map();
                }

                match self.sandbox_mnt {
                    true => self.set_up_tmpfs_cwd(),
                    false => (),
                }

                self.execute_callbacks()
            }),
            &mut *p,
            CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS,
            // TODO: Real follow flags
            Some(libc::SIGCHLD),
        ) {
            Ok(it) => it,
            Err(err) => return Err(Error::NixErrno(err)),
        };
        Ok(Child { pid })
    }

    pub(crate) fn apply_nsenter(&mut self) {
        for i in 0..self.namespaces.len() {
            match self.namespaces[i].fd {
                Some(f) => {
                    let i = self.namespaces.pop_front().unwrap();
                    assert_eq!(f,i.fd().unwrap());
                    nix::sched::setns(
                        f,
                        match i.typ() {
                            config::NamespaceType::Mount => CloneFlags::CLONE_NEWNS,
                            config::NamespaceType::Cgroup => CloneFlags::CLONE_NEWCGROUP,
                            config::NamespaceType::Uts => CloneFlags::CLONE_NEWUTS,
                            config::NamespaceType::Ipc => CloneFlags::CLONE_NEWIPC,
                            config::NamespaceType::User => CloneFlags::CLONE_NEWUSER,
                            config::NamespaceType::Pid => CloneFlags::CLONE_NEWPID,
                            config::NamespaceType::Network => CloneFlags::CLONE_NEWNET,
                            config::NamespaceType::Time => todo!(),
                        },
                    );
                }
                None => break,
            }
        }
    }

    pub(crate) fn check_is_child(&self) {
        assert_eq!(self.in_child, true);
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
        nix::unistd::write(file.into_raw_fd(), content.as_bytes()).unwrap();
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
        nix::unistd::write(file.into_raw_fd(), b"deny").unwrap();

        Self::write_id_map(format!("/proc/{}/gid_map", pid), &self.uid_maps);
    }

    pub(crate) fn execute_callbacks(&mut self) -> isize {
        self.check_is_child();

        let mut ret = 0;
        for _i in 0..self.callbacks.len() {
            ret = self.callbacks.pop_front().unwrap()();
        }
        return ret;
    }

    /// Crate tmpfs as root, simulate brwrap's behaviour
    ///
    /// Due to kernel bug#183461 ,this can only be called after setup uid
    /// and gid mapping.
    pub(crate) fn set_up_tmpfs_cwd(&self) {
        self.check_is_child();

        use nix::mount::{mount, MsFlags};
        use nix::unistd::pivot_root;
        use std::env::set_current_dir;
        use std::fs::DirBuilder;
        use std::os::unix::fs::DirBuilderExt;

        let tmp_path = "/tmp";
        //
        mount(
            Some(""),
            "/",
            Some(""),
            MsFlags::MS_SILENT | MsFlags::MS_SLAVE | MsFlags::MS_REC,
            Some(""),
        )
        .unwrap();

        mount(
            Some("tmpfs"),
            tmp_path,
            Some("tmpfs"),
            MsFlags::MS_NODEV | MsFlags::MS_NOSUID,
            Some(""),
        )
        .unwrap();

        set_current_dir(tmp_path).unwrap();

        let mut dir = DirBuilder::new();
        dir.mode(0o755);
        dir.create("/tmp/newroot").unwrap();
        dir.create("oldroot").unwrap();
        mount(
            Some("newroot"),
            "newroot",
            Some(""),
            MsFlags::MS_SILENT | MsFlags::MS_MGC_VAL | MsFlags::MS_BIND | MsFlags::MS_REC,
            Some(""),
        )
        .unwrap();

        pivot_root(tmp_path, "oldroot").unwrap();
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn test() {
        crate::Wrap::new_cmd("/bin/sh");
    }
}
