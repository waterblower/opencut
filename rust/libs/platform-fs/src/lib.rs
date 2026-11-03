//! Native file-system operations that need platform APIs beyond `std::fs`, such as
//! moving files to the Trash or Recycle Bin so the user can restore them.

use anyhow::Result;
use std::path::Path;

/// Moves the file or folder at `path` to the Trash.
#[cfg(target_os = "macos")]
pub fn move_to_trash(path: &Path) -> Result<()> {
    use anyhow::anyhow;
    use objc2_foundation::{NSFileManager, NSString, NSURL};

    let path = path
        .to_str()
        .ok_or_else(|| anyhow!("the path is not valid UTF-8"))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|error| anyhow!("{error}"))
}

/// Moves the file or folder at `path` to the Recycle Bin. When Windows cannot recycle it,
/// it asks before deleting permanently; declining is an error.
#[cfg(target_os = "windows")]
pub fn move_to_trash(path: &Path) -> Result<()> {
    use anyhow::bail;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
        FOF_WANTNUKEWARNING, SHFILEOPSTRUCTW, SHFileOperationW,
    };
    use windows::core::PCWSTR;

    let absolute_path = std::path::absolute(path)?;
    let wide: Vec<u16> = absolute_path.as_os_str().encode_wide().collect();
    // 工程目录经 canonicalize 后带 \\?\ 前缀，SHFileOperationW 不接受，需还原为普通路径。
    let verbatim: Vec<u16> = r"\\?\".encode_utf16().collect();
    let verbatim_unc: Vec<u16> = r"\\?\UNC\".encode_utf16().collect();
    let mut from: Vec<u16> = if wide.starts_with(&verbatim_unc) {
        [
            r"\\".encode_utf16().collect(),
            wide[verbatim_unc.len()..].to_vec(),
        ]
        .concat()
    } else if wide.starts_with(&verbatim) {
        wide[verbatim.len()..].to_vec()
    } else {
        wide
    };
    // pFrom 是以双 NUL 结尾的路径列表。
    from.extend([0, 0]);
    // 无法放入回收站时（如网络驱动器、超大文件）先询问用户，不静默永久删除；拒绝即中止。
    let flags =
        FOF_ALLOWUNDO | FOF_WANTNUKEWARNING | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT;
    let mut operation = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        fFlags: flags.0 as u16,
        ..Default::default()
    };
    // SAFETY: `from` outlives the call and is double-NUL terminated.
    let status = unsafe { SHFileOperationW(&mut operation) };
    if status != 0 {
        bail!("moving to the Recycle Bin failed with code {status:#x}");
    }
    if operation.fAnyOperationsAborted.as_bool() {
        bail!("moving to the Recycle Bin was aborted");
    }
    Ok(())
}

/// Unsupported on this platform; always an error.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn move_to_trash(_path: &Path) -> Result<()> {
    use anyhow::anyhow;

    Err(anyhow!(
        "moving files to the system Trash is not supported on this platform"
    ))
}
