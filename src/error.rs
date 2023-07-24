use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unshare failed: `{0}`")]
    UnshareFailed(nix::errno::Errno),
    #[error("unknown data store error")]
    Unknown,
}