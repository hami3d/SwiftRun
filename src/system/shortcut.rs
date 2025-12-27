#![allow(unsafe_op_in_unsafe_fn)]
#![allow(static_mut_refs)]
#![allow(non_snake_case)]

use std::fs;
use std::path::PathBuf;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Shell::*;
use windows::core::*;

pub fn manage_start_menu_shortcut(install: bool) -> Result<()> {
    let mut shortcut_path = get_start_menu_programs_path().ok_or(Error::from(HRESULT(-1)))?;
    shortcut_path.push("SwiftRun.lnk");

    if !install {
        if shortcut_path.exists() {
            let _ = fs::remove_file(shortcut_path);
        }
        return Ok(());
    }

    let exe_path = std::env::current_exe().map_err(|_| Error::from(HRESULT(-1)))?;
    let exe_path_str = exe_path.to_string_lossy().to_string();
    let working_dir = exe_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;

        let path_u16: Vec<u16> = exe_path_str
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        shell_link.SetPath(PCWSTR(path_u16.as_ptr()))?;

        shell_link.SetArguments(w!("--show"))?;

        let work_u16: Vec<u16> = working_dir
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        shell_link.SetWorkingDirectory(PCWSTR(work_u16.as_ptr()))?;

        shell_link.SetDescription(w!("SwiftRun - Powerful Run Dialog"))?;

        let persist_file: IPersistFile = shell_link.cast()?;
        let shortcut_path_u16: Vec<u16> = shortcut_path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        persist_file.Save(PCWSTR(shortcut_path_u16.as_ptr()), true)?;
    }

    Ok(())
}

fn get_start_menu_programs_path() -> Option<PathBuf> {
    if let Ok(app_data) = std::env::var("APPDATA") {
        let mut path = PathBuf::from(app_data);
        path.push("Microsoft");
        path.push("Windows");
        path.push("Start Menu");
        path.push("Programs");
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }
        Some(path)
    } else {
        None
    }
}
