extern crate winapi;
use std::ffi::OsStr;
use std::io;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::process::ExitStatus;
use std::os::windows::process::ExitStatusExt;

use crate::Command;

pub fn runas_impl(cmd: &Command) -> io::Result<ExitStatus> {
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

    unsafe {
        use winapi::um::combaseapi::{CoInitializeEx, CoUninitialize};
        use winapi::um::objbase::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
        use winapi::um::shellapi::{SHELLEXECUTEINFOW, ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS};
        use winapi::um::winbase::INFINITE;
        use winapi::um::winuser::{SW_HIDE, SW_NORMAL};
        use winapi::shared::winerror::SUCCEEDED;
        use winapi::shared::minwindef::{DWORD, TRUE};
        use winapi::um::synchapi::WaitForSingleObject;
        use winapi::um::processthreadsapi::GetExitCodeProcess;
        use std::ptr;

        let show = if cmd.hide { SW_HIDE } else { SW_NORMAL };

        let mut code : DWORD = 0;
        let mut sei = SHELLEXECUTEINFOW { 
            cbSize: mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS,
            lpVerb: (*"runas\0").encode_utf16().collect::<Vec<u16>>().as_ptr(),
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

        let coinitializeex_result = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        
        let mut succesfull = ShellExecuteExW(&mut sei) == TRUE && sei.hProcess != ptr::null_mut();

        if succesfull {
            WaitForSingleObject(sei.hProcess, INFINITE);
            succesfull = GetExitCodeProcess(sei.hProcess, &mut code) != 0;
        }
        
        if SUCCEEDED(coinitializeex_result) {
            CoUninitialize();
        }
        
        if succesfull {
            Ok(ExitStatus::from_raw(code))
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}
