use crate::error::Error;
use bitflags::bitflags;
use libc::c_int;
use linux_raw_sys::general::{
    CLONE_FILES, CLONE_FS, CLONE_NEWCGROUP, CLONE_NEWIPC, CLONE_NEWNET, CLONE_NEWNS, CLONE_NEWPID,
    CLONE_NEWTIME, CLONE_NEWUSER, CLONE_NEWUTS, CLONE_SYSVSEM,
};
use std::os::fd::RawFd;

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
        const NEWCGROUP = CLONE_NEWCGROUP;
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
        /// `CLONE_NEWUTS`
        const NEWUTS = CLONE_NEWUTS;
    }
}

/// disassociate parts of the process execution context
///
/// See also [unshare(2)](https://man7.org/linux/man-pages/man2/unshare.2.html)
pub fn unshare(flags: CloneFlags) -> Result<(), Error> {
    let res = unsafe { libc::unshare(flags.bits() as i32) };

    if res == -1 {
        Err(Error::OsErrno(unsafe { *libc::__errno_location().clone() }))
    } else {
        Ok(())
    }
}

/// reassociate thread with a namespace
///
/// See also [setns(2)](https://man7.org/linux/man-pages/man2/setns.2.html)
pub fn setns(fd: RawFd, nstype: CloneFlags) -> Result<(), Error> {
    let res = unsafe { libc::setns(fd, nstype.bits() as i32) };

    if res == -1 {
        Err(Error::OsErrno(unsafe { *libc::__errno_location().clone() }))
    } else {
        Ok(())
    }
}

/// Type for the function executed by [`clone`].
pub type CloneCb<'a> = Box<dyn FnMut() -> isize + 'a>;

/// `clone` create a child process
/// ([`clone(2)`](https://man7.org/linux/man-pages/man2/clone.2.html))
///
/// `stack` is a reference to an array which will hold the stack of the new
/// process.  Unlike when calling `clone(2)` from C, the provided stack
/// address need not be the highest address of the region.  Nix will take
/// care of that requirement.  The user only needs to provide a reference to
/// a normally allocated buffer.
pub unsafe fn clone(
    mut cb: CloneCb,
    stack: &mut [u8],
    flags: CloneFlags,
    signal: Option<c_int>,
) -> Result<u32, Error> {
    extern "C" fn callback(data: *mut CloneCb) -> c_int {
        let cb: &mut CloneCb = unsafe { &mut *data };
        (*cb)() as c_int
    }

    let res = unsafe {
        let combined = { flags.bits() as i32 } | signal.unwrap_or(0);
        let ptr = stack.as_mut_ptr().add(stack.len());
        let ptr_aligned = ptr.sub(ptr as usize % 16);
        libc::clone(
            std::mem::transmute(callback as extern "C" fn(*mut Box<dyn FnMut() -> isize>) -> i32),
            ptr_aligned as *mut libc::c_void,
            combined,
            &mut cb as *mut _ as *mut libc::c_void,
        )
    };

    if res == -1 {
        Err(Error::OsErrno(unsafe { *libc::__errno_location().clone() }))
    } else {
        Ok(res as u32)
    }
}

#[cfg(test)]
mod test {
    use crate::util::{unshare, CloneFlags};


    #[test]
    fn correctly_return_os_error() {
        use std::thread;

        let thread_join_handle = thread::spawn(move || {
            unshare(CloneFlags::NEWUSER).unwrap_err()
        });
        match thread_join_handle.join().unwrap() {
            crate::error::Error::OsErrno(num) => assert_eq!(22, num),
            _ => panic!(),
        }
    }
}