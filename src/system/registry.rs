use windows::Win32::System::Registry::*;
use windows::core::*;

pub fn manage_registry_hooks(install: bool) -> Result<()> {
    unsafe {
        // 1. Manage Run key for autostart
        let run_key_path = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let mut h_key = HKEY::default();

        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            run_key_path,
            Some(0),
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            &mut h_key,
        )
        .is_ok()
        {
            if install {
                if let Ok(exe_path) = std::env::current_exe() {
                    let path_str = exe_path.to_string_lossy().to_string();
                    let path_u16: Vec<u16> =
                        path_str.encode_utf16().chain(std::iter::once(0)).collect();
                    let data = std::slice::from_raw_parts(
                        path_u16.as_ptr() as *const u8,
                        path_u16.len() * 2,
                    );
                    RegSetValueExW(h_key, w!("SwiftRun"), Some(0), REG_SZ, Some(data)).ok()?;
                }
            } else {
                let _ = RegDeleteValueW(h_key, w!("SwiftRun"));
            }
            RegCloseKey(h_key);
        }

        // 2. Manage DisabledHotkeys to hijack Win+R
        let explorer_key_path =
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced");
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            explorer_key_path,
            Some(0),
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            &mut h_key,
        )
        .is_ok()
        {
            let val_name = w!("DisabledHotkeys");

            if install {
                // Get existing
                let mut data = [0u16; 128];
                let mut size = (data.len() * 2) as u32;
                let mut current_val = String::new();
                if RegQueryValueExW(
                    h_key,
                    val_name,
                    None,
                    None,
                    Some(data.as_mut_ptr() as _),
                    Some(&mut size),
                )
                .is_ok()
                {
                    current_val =
                        String::from_utf16_lossy(&data[..(size as usize / 2).saturating_sub(1)]);
                }

                if !current_val.contains('R') {
                    current_val.push('R');
                    let val_u16: Vec<u16> = current_val
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    let data_slice = std::slice::from_raw_parts(
                        val_u16.as_ptr() as *const u8,
                        val_u16.len() * 2,
                    );
                    RegSetValueExW(h_key, val_name, Some(0), REG_SZ, Some(data_slice)).ok()?;
                }
            } else {
                // Remove 'R' from DisabledHotkeys
                let mut data = [0u16; 128];
                let mut size = (data.len() * 2) as u32;
                if RegQueryValueExW(
                    h_key,
                    val_name,
                    None,
                    None,
                    Some(data.as_mut_ptr() as _),
                    Some(&mut size),
                )
                .is_ok()
                {
                    let current_val =
                        String::from_utf16_lossy(&data[..(size as usize / 2).saturating_sub(1)]);
                    if current_val.contains('R') {
                        let new_val: String = current_val.chars().filter(|&c| c != 'R').collect();
                        if new_val.is_empty() {
                            let _ = RegDeleteValueW(h_key, val_name);
                        } else {
                            let val_u16: Vec<u16> =
                                new_val.encode_utf16().chain(std::iter::once(0)).collect();
                            let data_slice = std::slice::from_raw_parts(
                                val_u16.as_ptr() as *const u8,
                                val_u16.len() * 2,
                            );
                            let _ =
                                RegSetValueExW(h_key, val_name, Some(0), REG_SZ, Some(data_slice));
                        }
                    }
                }
            }
            RegCloseKey(h_key);
        }
    }
    Ok(())
}
