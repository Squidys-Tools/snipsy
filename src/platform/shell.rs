use anyhow::Result;
use std::path::Path;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub fn reveal_in_explorer(path: &Path) {
    let _ = std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn();
}

pub fn open_path(path: &Path) {
    unsafe {
        let p = HSTRING::from(path.as_os_str());
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            &HSTRING::from("open"),
            &p,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Blocks inside a modal drag loop until the drop completes.
pub fn drag_file_out(hwnd: HWND, path: &Path) -> Result<()> {
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{DROPEFFECT_COPY, DROPEFFECT_MOVE};
    use windows::Win32::UI::Shell::{
        BHID_DataObject, IShellItem, SHCreateItemFromParsingName, SHDoDragDrop,
    };
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None)?;
        let data: IDataObject = item.BindToHandler(None, &BHID_DataObject)?;
        let _ = SHDoDragDrop(Some(hwnd), &data, None, DROPEFFECT_COPY | DROPEFFECT_MOVE);
    }
    Ok(())
}
