extern crate winapi;
use std::ffi::OsStr;
use std::io;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::process::{Child, ChildStdin, ChildStdout, ChildStderr};
use std::ptr;

use crate::Command;

use winapi::um::shellapi::{SHELLEXECUTEINFOW, ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SEE_MASK_INVOKEIDLIST};
use winapi::um::winuser::{SW_HIDE, SW_NORMAL};
use winapi::shared::minwindef::FALSE;

pub fn spawn_impl(cmd: &Command) -> io::Result<Child> {
    let mut params = String::new();
    for arg in cmd.args.iter() {
        let arg = arg.to_string_lossy();
        params.push(' ');
        if arg.len() == 0 {
            params.push_str("\"\"");
        } else if arg.find(&[' ', '\t', '"'][..]).is_none() {
            params.push_str(&arg);
        } else {
            params.push('"');
            for c in arg.chars() {
                match c {
                    '\\' => params.push_str("\\\\"),
                    '"' => params.push_str("\\\""),
                    c => params.push(c),
                }
            }
            params.push('"');
        }
    }

    let file = OsStr::new(&cmd.command)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let params = OsStr::new(&params)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let current_dir = if let Some(dir) = &cmd.current_dir {
        OsStr::new(&dir)
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>().as_ptr()
    } else {
        ptr::null_mut()
    };

    unsafe {
        let show = if cmd.hide { SW_HIDE } else { SW_NORMAL };

        let mut sei = SHELLEXECUTEINFOW { 
            cbSize: mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_INVOKEIDLIST | SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS,
            lpVerb: ptr::null(),
            lpFile: file.as_ptr(),
            lpParameters: params.as_ptr(),
            nShow: show,
            dwHotKey: 0,
            hInstApp: ptr::null_mut(),
            hMonitor: ptr::null_mut(),
            hProcess: ptr::null_mut(),
            hkeyClass: ptr::null_mut(),
            hwnd: ptr::null_mut(),
            lpClass: ptr::null_mut(),
            lpDirectory: current_dir,
            lpIDList: ptr::null_mut(),
        };

        if ShellExecuteExW(&mut sei) == FALSE || sei.hProcess == ptr::null_mut() {
            return Err(std::io::Error::last_os_error());
        }

        return Ok(mem::transmute((sei.hProcess, None::<ChildStdin>, None::<ChildStdout>, None::<ChildStderr>)));
    }
}
