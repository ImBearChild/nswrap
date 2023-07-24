extern crate nix;
use nix::sched::CloneFlags;

use crate::error::Error;
pub enum Namespaces {
    Cgroup,
    IPC,
    Mount,
    Network,
    User,
    UTS,
}

pub fn unshare(ns: Vec<Namespaces>) -> Result<(), Error>{
    let mut flags = CloneFlags::empty();
    for i in ns {
        match i {
            Namespaces::Cgroup => {
                flags.set(CloneFlags::CLONE_NEWCGROUP, true);
            }
            Namespaces::IPC => {
                flags.set(CloneFlags::CLONE_NEWIPC, true);
            }
            Namespaces::Mount => {
                flags.set(CloneFlags::CLONE_NEWNS, true);
            }
            Namespaces::Network => {
                flags.set(CloneFlags::CLONE_NEWNET, true);
            }
            Namespaces::User => {
                flags.set(CloneFlags::CLONE_NEWUSER, true);
            },
            Namespaces::UTS => {
                flags.set(CloneFlags::CLONE_NEWUTS, true);
            },
        }
    }
    match nix::sched::unshare(flags) {
        Ok(_) => {Ok(())},
        Err(e) => {Err(Error::UnshareFailed(e))},
    }
}
