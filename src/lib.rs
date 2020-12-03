//! This library implements basic support for running a command in an elevated context.
//!
//! In particular this runs a command through "sudo" or other platform equivalents.
//!
//! ## Basic Usage
//!
//! The library provides a single struct called `Command` which largely follows the
//! API of `std::process::Command`.  However it does not support capturing output or
//! gives any guarantees for the working directory or environment.  This is because
//! the platform APIs do not have support for that either in some cases.
//!
//! In particular the working directory is always the system32 folder on windows and
//! the environment variables are always the ones of the initial system session on
//! OS X if the GUI mode is used.
//!
//! ```rust,no_run
//! use runas::Command;
//!
//! let status = Command::new("rm")
//!     .arg("/usr/local/my-app")
//!     .status()
//!     .unwrap();
//! ```
//!
//! ## Platform Support
//!
//! The following platforms are supported:
//!
//! * Windows: always GUI mode
//! * OS X: GUI and CLI mode
//! * Linux: CLI mode

use std::ffi::{OsStr, OsString};
use std::io;
use std::process::{Child, ExitStatus};

#[cfg(target_os = "macos")]
mod impl_darwin;
#[cfg(unix)]
mod impl_unix;
#[cfg(windows)]
mod impl_windows;

/// A process builder for elevated execution, providing fine-grained control
/// over how a new process should be spawned.
///
/// A default configuration can be
/// generated using `Command::new(program)`, where `program` gives a path to the
/// program to be executed. Additional builder methods allow the configuration
/// to be changed (for example, by adding arguments) prior to spawning:
///
/// ```rust,no_run
/// use runas::Command;
///
/// let child = if cfg!(target_os = "windows") {
///     Command::new("cmd")
///             .args(&["/C", "echo hello"])
///             .spawn()
///             .expect("failed to execute process")
/// } else {
///     Command::new("sh")
///             .arg("-c")
///             .arg("echo hello")
///             .spawn()
///             .expect("failed to execute process")
/// };
///
/// let hello = child.wait();
/// ```
///
/// `Command` can be reused to spawn multiple processes. The builder methods
/// change the command without needing to immediately spawn the process.
///
/// ```rust,no_run
/// use runas::Command;
///
/// let mut echo_hello = Command::new("sh");
/// echo_hello.arg("-c")
///           .arg("echo hello");
/// let hello_1 = echo_hello.spawn().expect("failed to execute process");
/// let hello_2 = echo_hello.spawn().expect("failed to execute process");
/// ```
///
/// Similarly, you can call builder methods after spawning a process and then
/// spawn a new process with the modified settings.
///
/// ```rust,no_run
/// use runas::Command;
///
/// let mut list_dir = Command::new("ls");
///
/// // Execute `ls` in the current directory of the program.
/// list_dir.status().expect("process failed to execute");
///
/// println!();
///
/// // Change `ls` to execute in the root directory.
/// list_dir.current_dir("/");
///
/// // And then execute `ls` again but in the root directory.
/// list_dir.status().expect("process failed to execute");
/// ```
pub struct Command {
    command: OsString,
    args: Vec<OsString>,
    current_dir: Option<OsString>,
    force_prompt: bool,
    hide: bool,
    gui: bool,
}

impl Command {
    /// Constructs a new `Command` for launching the program at
    /// path `program`, with the following default configuration:
    ///
    /// * No arguments to the program
    /// * Program to be visable
    /// * Not launched from a GUI context
    /// * Inherit the current process's environment
    /// * Inherit the current process's working directory
    /// * Inherit stdin/stdout/stderr for `spawn` or `status`, but create pipes for `output`
    ///
    /// Builder methods are provided to change these defaults and
    /// otherwise configure the process.
    ///
    /// If `program` is not an absolute path, the `PATH` will be searched in
    /// an OS-defined way.
    ///
    /// The search path to be used may be controlled by setting the
    /// `PATH` environment variable on the Command,
    /// but this has some implementation limitations on Windows
    /// (see issue #37519).
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```rust,no_run
    /// use runas::Command;
    ///
    /// Command::new("sh")
    ///         .spawn()
    ///         .expect("sh command failed to start");
    /// ```
    pub fn new<S: AsRef<OsStr>>(program: S) -> Command {
        Command {
            command: program.as_ref().to_os_string(),
            args: vec![],
            current_dir: None,
            hide: false,
            gui: false,
            force_prompt: true,
        }
    }

    /// Adds an argument to pass to the program.
    ///
    /// Only one argument can be passed per use. So instead of:
    ///
    /// ```rust,no_run
    /// # runas::Command::new("sh")
    /// .arg("-C /path/to/repo")
    /// # ;
    /// ```
    ///
    /// usage would be:
    ///
    /// ```rust,no_run
    /// # runas::Command::new("sh")
    /// .arg("-C")
    /// .arg("/path/to/repo")
    /// # ;
    /// ```
    ///
    /// To pass multiple arguments see [`args`].
    ///
    /// [`args`]: Command::args
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use runas::Command;
    ///
    /// Command::new("ls")
    ///         .arg("-l")
    ///         .arg("-a")
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// Adds multiple arguments to pass to the program.
    ///
    /// To pass a single argument see [`arg`].
    ///
    /// [`arg`]: Command::arg
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use runas::Command;
    ///
    /// Command::new("ls")
    ///         .args(&["-l", "-a"])
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn args<S: AsRef<OsStr>>(&mut self, args: &[S]) -> &mut Command {
        for arg in args {
            self.arg(arg);
        }
        self
    }

    /// Sets the working directory for the child process.
    ///
    /// # Platform-specific behavior
    ///
    /// If the program path is relative (e.g., `"./script.sh"`), it's ambiguous
    /// whether it should be interpreted relative to the parent's working
    /// directory or relative to `current_dir`. The behavior in this case is
    /// platform specific and unstable, and it's recommended to use
    /// [`canonicalize`] to get an absolute program path instead.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use runas::Command;
    ///
    /// Command::new("ls")
    ///         .current_dir("/bin")
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    ///
    /// [`canonicalize`]: std::fs::canonicalize
    pub fn current_dir<P: AsRef<std::path::Path>>(&mut self, dir: P) -> &mut Command {
        self.current_dir = Some(dir.as_ref().as_os_str().into());
        self
    }

    /// Controls the visibility of the program on supported platforms.
    /// 
    /// The default is to launch the program visible.
    /// 
    /// # Examples
    ///
    /// ```rust,no_run
    /// use runas::Command;
    ///
    /// let status = Command::new("/bin/cat")
    ///                      .arg("file.txt")
    ///                      .disable_prompt()
    ///                      .status()
    ///                      .expect("failed to execute process");
    ///
    /// assert!(status.success());
    /// ```
    pub fn show(&mut self, val: bool) -> &mut Command {
        self.hide = !val;
        self
    }

    /// Controls the GUI context.  The default behavior is to assume that the program is
    /// launched from a command line (not using a GUI).  This primarily controls how the
    /// elevation prompt is rendered.  On some platforms like Windows the elevation prompt
    /// is always a GUI element.
    ///
    /// If the preferred mode is not available it falls back to the other automatically.
    pub fn gui(&mut self, val: bool) -> &mut Command {
        self.gui = val;
        self
    }


    /// Disabling the force prompt would allow the successive use of elevated commands on unix platforms
    /// without prompting for a password after each command.
    /// 
    /// By default, the user will be prompted on each successive command.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use runas::Command;
    ///
    /// let status = Command::new("/bin/cat")
    ///                      .arg("file.txt")
    ///                      .disable_prompt()
    ///                      .status()
    ///                      .expect("failed to execute process");
    ///
    /// assert!(status.success());
    /// 
    /// //The user won't be prompted for a password on the second run.
    /// status = Command::new("/bin/ps")
    ///                      .disable_prompt()
    ///                      .status()
    ///                      .expect("failed to execute process");
    ///
    /// assert!(status.success());
    /// ```
    pub fn disable_force_prompt(&mut self) -> &mut Command {
        self.force_prompt = false;
        self
    }

    /// Executes the command as a child process, returning a handle to it.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use runas::Command;
    ///
    /// Command::new("ls")
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn spawn(&mut self) -> io::Result<Child> {
        #[cfg(all(unix, target_os = "macos"))]
        use crate::impl_darwin::spawn_impl;
        #[cfg(all(unix, not(target_os = "macos")))]
        use impl_unix::spawn_impl;
        #[cfg(windows)]
        use impl_windows::spawn_impl;
        spawn_impl(&self)
    }

    /// Executes a command as a child process, waiting for it to finish and
    /// collecting its exit status.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use runas::Command;
    ///
    /// let status = Command::new("/bin/cat")
    ///                      .arg("file.txt")
    ///                      .status()
    ///                      .expect("failed to execute process");
    ///
    /// println!("process exited with: {}", status);
    ///
    /// assert!(status.success());
    /// ```
    pub fn status(&mut self) -> io::Result<ExitStatus> {
        self.spawn()?.wait()
    }
}
