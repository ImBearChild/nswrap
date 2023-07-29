use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unshare failed: `{0}`")]
    UnshareFailed(i32),
    #[error("Clone failed: `{0}`")]
    CloneFailed(i32),
    #[error("Unix API lib failed: `{0}`")]
    OsErrno(i32),
    #[error("unknown data store error")]
    Unknown,
}
