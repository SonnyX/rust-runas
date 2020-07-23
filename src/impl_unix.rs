use which;

use std::io;
use std::process;
use std::process::Child;

use crate::Command;
pub fn spawn_impl(cmd: &Command) -> io::Result<Child> {
    match which::which("sudo") {
        Ok(_) => {
            let mut c = process::Command::new("sudo");
            if cmd.force_prompt {
                c.arg("-k");
            }
            c.arg("--").arg(&cmd.command).args(&cmd.args[..]).spawn()
        }
        Err(_) => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Command `sudo` not found",
        )),
    }
}
