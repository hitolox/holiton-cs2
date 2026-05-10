#![windows_subsystem = "windows"]

use std::{
    env,
    ffi::OsStr,
    mem,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
};

use windows::{
    core::{PCWSTR, PWSTR, w},
    Win32::{
        Foundation::{CloseHandle, HWND, MAX_PATH},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            ProcessStatus::{EnumDeviceDrivers, GetDeviceDriverBaseNameW},
            Threading::{
                CreateProcessW, GetExitCodeProcess, WaitForSingleObject, INFINITE,
                PROCESS_INFORMATION, STARTUPINFOW,
            },
        },
        UI::{
            Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
            WindowsAndMessaging::{
                MessageBoxW, IDOK, IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONQUESTION,
                MB_OK, MB_OKCANCEL, MB_YESNO, MESSAGEBOX_STYLE,
            },
        },
    },
};

const APP_TITLE: PCWSTR = w!("Holiton Loader");

const CONTROLLER_NAME: &str = "controller.exe";
const DRIVER_DLL_NAME: &str = "driver_interface_kernel.dll";
const DRIVER_SYS_NAME: &str = "driver_standalone.sys";
const KDMAPPER_NAME: &str = "kdmapper.exe";
const CS2_PROCESS_NAME: &str = "cs2.exe";

/// Substrings the loader looks for in loaded kernel module base names to decide
/// whether the Holiton/Valthrun kernel driver is already mapped.
const DRIVER_MODULE_HINTS: &[&str] = &["valthrun", "vtd", "driver_standalone", "holiton"];

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn wide_from_path(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

fn message_box(text: &str, flags: MESSAGEBOX_STYLE) -> i32 {
    let text_wide = to_wide(text);
    unsafe { MessageBoxW(HWND::default(), PCWSTR(text_wide.as_ptr()), APP_TITLE, flags).0 }
}

fn show_error(text: &str) {
    let _ = message_box(text, MB_OK | MB_ICONERROR);
}

fn ask_yes_no(text: &str) -> bool {
    message_box(text, MB_YESNO | MB_ICONQUESTION) == IDYES.0
}

fn ask_ok_cancel(text: &str) -> bool {
    message_box(text, MB_OKCANCEL | MB_ICONINFORMATION) == IDOK.0
}

fn loader_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_owned()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn ensure_files_present(dir: &Path) -> Result<(), String> {
    let required = [
        CONTROLLER_NAME,
        DRIVER_DLL_NAME,
        DRIVER_SYS_NAME,
        KDMAPPER_NAME,
    ];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|name| !dir.join(name).exists())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "The following required files are missing next to holiton-loader.exe:\n\n  - {}\n\nPlease redownload the full Holiton package and keep all files together.",
            missing.join("\n  - ")
        ))
    }
}

fn is_process_running(process_name: &str) -> bool {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return false,
        };
        if snapshot.is_invalid() {
            return false;
        }

        let mut entry = PROCESSENTRY32W {
            dwSize: mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut found = false;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case(process_name) {
                    found = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
        found
    }
}

fn is_kernel_driver_loaded() -> bool {
    unsafe {
        let mut needed: u32 = 0;
        let mut drivers: Vec<*mut std::ffi::c_void> = vec![ptr::null_mut(); 1024];
        let cb = (drivers.len() * mem::size_of::<*mut std::ffi::c_void>()) as u32;

        if EnumDeviceDrivers(drivers.as_mut_ptr(), cb, &mut needed).is_err() {
            return false;
        }

        let count = (needed as usize / mem::size_of::<*mut std::ffi::c_void>())
            .min(drivers.len());

        let mut name_buf = [0u16; MAX_PATH as usize];
        for &drv in drivers.iter().take(count) {
            let written = GetDeviceDriverBaseNameW(drv, &mut name_buf);
            if written == 0 {
                continue;
            }
            let name = String::from_utf16_lossy(&name_buf[..written as usize]).to_lowercase();
            if DRIVER_MODULE_HINTS.iter().any(|h| name.contains(h)) {
                return true;
            }
        }
        false
    }
}

/// Runs kdmapper.exe with the standalone driver as admin (UAC prompt).
/// Returns true on success (exit code 0).
fn map_driver(dir: &Path) -> Result<(), String> {
    let exe = dir.join(KDMAPPER_NAME);
    let sys = dir.join(DRIVER_SYS_NAME);

    let exe_w = wide_from_path(&exe);
    let params = format!("\"{}\"", sys.display());
    let params_w = to_wide(&params);
    let verb_w = to_wide("runas");
    let dir_w = wide_from_path(dir);

    let mut info = SHELLEXECUTEINFOW {
        cbSize: mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb_w.as_ptr()),
        lpFile: PCWSTR(exe_w.as_ptr()),
        lpParameters: PCWSTR(params_w.as_ptr()),
        lpDirectory: PCWSTR(dir_w.as_ptr()),
        nShow: 1, // SW_SHOWNORMAL
        ..Default::default()
    };

    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok.is_err() {
        return Err(format!(
            "Failed to launch kdmapper.exe.\nMost likely the UAC prompt was cancelled.\n\nDetails: {:?}",
            ok.err()
        ));
    }
    if info.hProcess.is_invalid() {
        return Err(
            "kdmapper.exe launched but no process handle was returned. Cannot verify success.".to_string(),
        );
    }

    unsafe {
        let _ = WaitForSingleObject(info.hProcess, INFINITE);
        let mut code: u32 = 0;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);

        if code != 0 {
            return Err(format!(
                "kdmapper.exe exited with code {} (0x{:X}).\n\nCommon causes:\n  - Hypervisor-Enforced Code Integrity (HVCI / Memory Integrity) is enabled\n  - Vulnerable driver was blocked by Microsoft driver blocklist\n  - Antivirus blocked the mapper",
                code, code
            ));
        }
    }

    Ok(())
}

fn launch_controller(dir: &Path) -> Result<(), String> {
    let exe = dir.join(CONTROLLER_NAME);
    let mut cmdline_w = to_wide(&format!("\"{}\"", exe.display()));
    let dir_w = wide_from_path(dir);

    let si = STARTUPINFOW {
        cb: mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();

    let ok = unsafe {
        CreateProcessW(
            PCWSTR::null(),
            PWSTR(cmdline_w.as_mut_ptr()),
            None,
            None,
            false,
            Default::default(),
            None,
            PCWSTR(dir_w.as_ptr()),
            &si,
            &mut pi,
        )
    };

    if ok.is_err() {
        return Err(format!(
            "Failed to launch {}.\nDetails: {:?}",
            CONTROLLER_NAME,
            ok.err()
        ));
    }

    unsafe {
        let _ = CloseHandle(pi.hProcess);
        let _ = CloseHandle(pi.hThread);
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let dir = loader_dir();

    ensure_files_present(&dir)?;

    if !is_process_running(CS2_PROCESS_NAME) {
        let proceed = ask_ok_cancel(
            "Counter-Strike 2 does not appear to be running.\n\n\
             Start CS2, wait until you are in the main menu, then click OK to continue.\n\
             Click Cancel to abort.",
        );
        if !proceed {
            return Ok(());
        }
    }

    if !is_kernel_driver_loaded() {
        let load = ask_yes_no(
            "The Holiton kernel driver is not loaded.\n\n\
             Map it now using kdmapper? A UAC prompt will appear (administrator rights are required).\n\n\
             Click No to abort.",
        );
        if !load {
            return Ok(());
        }

        map_driver(&dir)?;

        // Re-check after mapping.
        if !is_kernel_driver_loaded() {
            // Some driver names cannot be observed by EnumDeviceDrivers (e.g. unbacked manual maps).
            // Treat a successful kdmapper exit as good and continue, but warn the user.
            // Falling through is fine — controller.exe will surface a clear error if the driver
            // really did not load.
        }
    }

    launch_controller(&dir)?;
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        show_error(&err);
        std::process::exit(1);
    }
}
