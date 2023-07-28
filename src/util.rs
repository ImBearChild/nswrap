extern crate nix;
use nix::sched::CloneFlags;

use crate::error::Error;
use crate::config::NamespaceType;

pub fn get_uid() -> u32 {
    nix::unistd::Uid::current().into()
}
pub fn get_gid() -> u32 {
    nix::unistd::Gid::current().into()
}

pub fn get_pid() -> i32 {
    nix::unistd::Pid::this().into()
}

