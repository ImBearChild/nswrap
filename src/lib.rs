use unshare::Namespaces;
pub mod config;
pub mod error;
pub mod unshare;
extern crate xdg;
use crate::error::Error;


#[derive(derive_builder::Builder)]
pub struct Wrap {
    /// rootfs for container, when not exist, a tmpfs will be used
    #[builder(setter(into, strip_option), default)]
    root: Option<config::Root>,

    #[builder(setter(into, strip_option), default)]
    mounts: Option<Vec<config::Mount>>,

    #[builder(setter(into, strip_option), default)]
    process: config::Process,

    #[builder(setter(into, strip_option), default)]
    hostname: Option<String>,

    /// If `uid_mappings` is `None`,
    #[builder(setter(into, strip_option), default)]
    uid_mappings: Option<Vec<config::LinuxIdMapping>>,

    #[builder(setter(into, strip_option), default)]
    gid_mappings: Option<Vec<config::LinuxIdMapping>>,
}

impl Wrap {
    pub fn start(self) -> Result<(),Error> {
        let mut namespaces = Vec::new();
        if let Some(_) = &self.gid_mappings {
            namespaces.push(Namespaces::User);
        }
        if let Some(_) = &self.uid_mappings {
            namespaces.push(Namespaces::User);
        }
        unshare::unshare(namespaces)?;
        let mut cmd = std::process::Command::new(&self.process.args()[0]);
        cmd.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warpbuilder_def() {
        WrapBuilder::default().build().unwrap();
    }

    #[test]
    fn bin_bash_echo() {
        let mut root = config::Root::default();
        root.set_path("/".into()).set_readonly(false.into());
        let mut process = config::Process::default();
        process.set_args(vec![String::from("/bin/sh"),String::from("-c"),String::from("exit 114")].into());
        WrapBuilder::default().root(root).process(process).build().unwrap().start();
    }
}
