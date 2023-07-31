use nswrap;
use nswrap::*;

use rustix::fd::{FromRawFd, OwnedFd};


const _TMP_DIR: &str = "/tmp/nswrap.test/";
const _TMP_DIR1: &str = "/tmp/nswrap.test/test-1";
const _TMP_DIR2: &str = "/tmp/nswrap.test/test-2";


#[test]
fn command_return_code() {
    let mut wrap = nswrap::Wrap::new_program("/bin/sh");
    wrap.arg("-c");
    wrap.arg("exit 25");
    let status = wrap.status().unwrap();
    assert_eq!(status.code().unwrap(),25);
}

#[test]
fn command_return_code_2() {
    let mut wrap = nswrap::Wrap::new_program("/bin/sh");
    wrap.args(vec!["-c","exit 25"]);
    let status = wrap.status().unwrap();
    assert_eq!(status.code().unwrap(),25);
}



fn make_test_dir() {
    use std::fs;
    fs::remove_dir_all(_TMP_DIR);
    fs::create_dir_all(_TMP_DIR1).unwrap();
    fs::create_dir_all(_TMP_DIR2).unwrap();
}

#[test]
// Create a bind mount and write files into it.
// just to make sure mount namespace works correctly.
fn bind_mount() {
    use std::fs::File;
    use std::io::prelude::*;
    make_test_dir();
    let cb = || {
        use std::fs::File;
        util::unshare(util::CloneFlags::NEWNS).unwrap();
        rustix::fs::mount(_TMP_DIR1, _TMP_DIR2, "", rustix::fs::MountFlags::BIND, "").unwrap();
        let mut file = File::create(_TMP_DIR2.to_owned() + "/foo.txt").unwrap();
        std::io::Write::write_all(&mut file, b"Hello, world!").unwrap();
        return 0;
    };
    let mut wrap = Wrap::new();
    wrap.callback(cb).unshare(config::NamespaceType::User);
    wrap.spawn().unwrap().wait().unwrap();

    // Check Result
    let mut file = File::open(_TMP_DIR1.to_owned() + "/foo.txt").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    assert_eq!(contents, "Hello, world!");
}

#[test]
fn callback_return_value() {
    let cb = || {
        return 16;
    };
    let mut wrap = Wrap::new();
    wrap.callback(cb).unshare(config::NamespaceType::User);
    let ret = wrap.spawn().unwrap().wait().unwrap().code().unwrap();
    assert_eq!(16, ret);
}

#[test]
fn callback_return_value_in_thread() {
    use std::thread;

    let thread_join_handle = thread::spawn(move || {
        let cb = || {
            return 16;
        };
        let mut wrap = Wrap::new();
        wrap.callback(cb).unshare(config::NamespaceType::User);
        let ret = wrap.spawn().unwrap().wait().unwrap().code().unwrap();
        ret
    });

    assert_eq!(thread_join_handle.join().unwrap(), 16);
}

/// https://github.com/rust-lang/rust/issues/79740
#[test]
#[should_panic]
fn panic_in_thread() {
    use std::thread;

    let thread_join_handle = thread::spawn(move || {
        let cb = || panic!();
        let mut wrap = Wrap::new();
        wrap.callback(cb).unshare(config::NamespaceType::User);
        let ret = wrap.status().unwrap();
        ret
        // println!("{:?}", ret.wait_status)
    });
    assert_eq!(thread_join_handle.join().unwrap().success(), true)
}

#[test]
fn tmpfs_root_sandbox_mnt() {
    let cb = || {
        use std::path::Path;
        let p = Path::new("/bin/sh");
        match p.exists() {
            true => return 16,
            false => return 32,
        };
    };
    let mut binding = Wrap::new();
    let wrap = binding
        .callback(cb)
        .unshare(config::NamespaceType::User)
        .unshare(config::NamespaceType::Mount)
        .sandbox_mnt(true)
        .id_map_preset(config::IdMapPreset::Current);
    let ret = wrap.spawn().unwrap().wait().unwrap().code().unwrap();
    assert_eq!(32, ret);
}

#[test]
fn raw_child_pipe() {
    let (read_end, write_end) = rustix::pipe::pipe().unwrap();
    let cb = move || {
        let mut newfd = unsafe { OwnedFd::from_raw_fd(16) };
        rustix::io::dup2(write_end, &mut newfd).unwrap(); // Old fd will be dropped!
        rustix::io::write(newfd, b"16").unwrap();
        return 42;
    };
    let mut binding = Wrap::new();
    let wrap = binding
        .callback(cb)
        .unshare(config::NamespaceType::User)
        .unshare(config::NamespaceType::Mount)
        .sandbox_mnt(true)
        .id_map_preset(config::IdMapPreset::Current);
    let ret = wrap.spawn();
    let ret = ret.unwrap().wait().unwrap().code().unwrap();
    assert_eq!(ret, 42);
    let mut buf: [u8; 2] = *b"00";
    rustix::io::read(read_end, &mut buf).unwrap();
    assert_eq!(buf, *b"16");
}