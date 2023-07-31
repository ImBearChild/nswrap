use nswrap;

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