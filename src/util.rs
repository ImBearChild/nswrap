extern crate nix;
use crate::error::Error;
use bitflags::bitflags;
use libc::c_int;
use linux_raw_sys::general::{
    CLONE_FILES, CLONE_FS, CLONE_NEWCGROUP, CLONE_NEWIPC, CLONE_NEWNET, CLONE_NEWNS, CLONE_NEWPID,
    CLONE_NEWTIME, CLONE_NEWUSER, CLONE_NEWUTS, CLONE_SYSVSEM,
};

pub fn get_uid() -> u32 {
    nix::unistd::Uid::current().into()
}
pub fn get_gid() -> u32 {
    nix::unistd::Gid::current().into()
}

pub fn get_pid() -> i32 {
    nix::unistd::Pid::this().into()
}

bitflags! {
    /// `CLONE_*` for use with [`unshare`].
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct CloneFlags: u32 {
        /// `CLONE_FILES`.
        const FILES = CLONE_FILES;
        /// `CLONE_FS`.
        const FS = CLONE_FS;
        /// `CLONE_NEWCGROUP`.
        const NWCGROUP = CLONE_NEWCGROUP;
        /// `CLONE_NEWIPC`.
        const NEWIPC = CLONE_NEWIPC;
        /// `CLONE_NEWNET`.
        const NEWNET = CLONE_NEWNET;
        /// `CLONE_NEWNS`.
        const NEWNS = CLONE_NEWNS;
        /// `CLONE_NEWPID`.
        const NEWPID = CLONE_NEWPID;
        /// `CLONE_NEWTIME`.
        const NEWTIME = CLONE_NEWTIME;
        /// `CLONE_NEWUSER`.
        const NEWUSER = CLONE_NEWUSER;
        /// `CLONE_SYSVSEM`.
        const SYSVSEM = CLONE_SYSVSEM;
    }
}

pub fn  clone<F>(
    cb: F,
    stack: &mut [u8],
    flags: CloneFlags,
    signal: Option<c_int>,
) -> Result<u32, Error>
where
    F: FnOnce() -> isize,
{
    // box the closure data
    let bf = Box::new(cb);
    // leave it on the heap
    let p = Box::into_raw(bf) as *mut libc::c_void;
    let combined = flags.bits() as i32 | signal.unwrap_or(0);
    let res = unsafe {
        let ptr = stack.as_mut_ptr().add(stack.len());
        let ptr_aligned = ptr.sub(ptr as usize % 16);
        libc::clone(
            std::mem::transmute(p),
            ptr_aligned as *mut libc::c_void,
            combined,
            0 as *mut libc::c_void,
        )
    };
    if res == -1 {
        Err({ Error::OsErrno(unsafe {
            *libc::__errno_location().clone()
        }) })
    } else {
        Ok(res as u32)
    }
}
