extern crate winapi;
use std::ffi::OsStr;
use std::io;
use std::mem;
use std::ptr;
use std::os::windows::ffi::OsStrExt;
use std::process::{Child};

use crate::Command;
use std::process::{ChildStdin,ChildStdout,ChildStderr};

use winapi::um::combaseapi::CoInitializeEx;
use winapi::um::objbase::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
use winapi::um::shellapi::{SHELLEXECUTEINFOW, ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS};
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

    let lpVerb = match &cmd.command.to_string_lossy().ends_with(".exe") {
        false => "runas".encode_utf16().collect::<Vec<u16>>().as_ptr(),
        true => ptr::null()
    };

    let file = OsStr::new(&cmd.command)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let params = OsStr::new(&params)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();

    unsafe {
        let show = if cmd.hide { SW_HIDE } else { SW_NORMAL };

        let mut sei = SHELLEXECUTEINFOW { 
            cbSize: mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS,
            lpVerb: lpVerb,
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
            lpDirectory: ptr::null_mut(),
            lpIDList: ptr::null_mut(),
        };

        CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);

        if ShellExecuteExW(&mut sei) == FALSE || sei.hProcess == ptr::null_mut() {
            return Ok(mem::transmute((-1, None::<ChildStdin>, None::<ChildStdout>, None::<ChildStderr>)));
        }

        return Ok(mem::transmute((sei.hProcess, None::<ChildStdin>, None::<ChildStdout>, None::<ChildStderr>)));
    }
}
