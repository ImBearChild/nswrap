use std::{ffi::{OsStr, OsString}, fs::{File, OpenOptions}, os::{unix::prelude::OsStrExt, fd::{AsFd, IntoRawFd}}};

use crate::{config, util};

impl crate::Wrap<'_> {
    pub(crate) fn check_subprogress(&self) {
        assert_eq!(self.in_subprocess, true);
    }
    pub(crate) fn write_id_map<S: AsRef<OsStr>>(file: S, map: &Vec<config::IdMap>) {
        let file = OpenOptions::new().write(true).open(file.as_ref()).unwrap();
        let mut content = OsString::new();
        for i in map{
            content.push(format!("{}",i.container_id()));
            content.push(" ");
            content.push(format!("{}",i.host_id()));
            content.push(" ");
            content.push(format!("{}\n",i.size()));
        }
        nix::unistd::write(file.into_raw_fd(),content.as_bytes()).unwrap();
    }
    pub(crate) fn set_id_map(&self) {
        let pid = util::get_pid();
        Self::write_id_map(format!("/proc/{}/uid_map",pid), &self.uid_maps);

        // Write /proc/pid/setgroups before wite /proc/pid/gid_map, or it will fail.
        // See https://manpages.opensuse.org/Tumbleweed/man-pages/user_namespaces.7.en.html
        let file = OpenOptions::new().write(true).open(format!("/proc/{}/setgroups",pid)).unwrap();
        nix::unistd::write(file.into_raw_fd(),b"deny").unwrap();

        Self::write_id_map(format!("/proc/{}/gid_map",pid), &self.uid_maps);
    }
    pub(crate) fn execute_callbacks(&mut self) -> isize {
        self.check_subprogress();

        let mut ret = 0;
        for _i in 0..self.callbacks.len() {
            ret = self.callbacks.pop().unwrap()();
        }
        return ret;
    }

    /// Crate tmpfs as root, simulate brwrap's behaviour
    ///
    /// Due to kernel bug#183461 ,this can only be called after setup uid
    /// and gid mapping.
    pub(crate) fn set_up_tmpfs_cwd(&self) {
        self.check_subprogress();

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
