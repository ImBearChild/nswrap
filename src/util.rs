extern crate nix;
use nix::sched::CloneFlags;

use crate::error::Error;
use crate::config::NamespaceType;


pub fn unshare(ns: Vec<NamespaceType>) -> Result<(), Error>{
    let mut flags = CloneFlags::empty();
    for i in ns {
        match i {
            NamespaceType::Cgroup => {
                flags.set(CloneFlags::CLONE_NEWCGROUP, true);
            }
            NamespaceType::Ipc => {
                flags.set(CloneFlags::CLONE_NEWIPC, true);
            }
            NamespaceType::Mount => {
                flags.set(CloneFlags::CLONE_NEWNS, true);
            }
            NamespaceType::Network => {
                flags.set(CloneFlags::CLONE_NEWNET, true);
            }
            NamespaceType::User => {
                flags.set(CloneFlags::CLONE_NEWUSER, true);
            },
            NamespaceType::Uts => {
                flags.set(CloneFlags::CLONE_NEWUTS, true);
            },
            NamespaceType::Pid => {
                flags.set(CloneFlags::CLONE_NEWPID, true);
            },
            _ => {}
        }
    }
    match nix::sched::unshare(flags) {
        Ok(_) => {Ok(())},
        Err(e) => {Err(Error::UnshareFailed(e))},
    }
}
