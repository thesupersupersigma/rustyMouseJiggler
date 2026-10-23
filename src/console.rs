use windows_sys::Win32::Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows_sys::Win32::System::Console::{
    AttachConsole, FreeConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
};

/// Attach to the launching console (the exe is a GUI-subsystem app) so --help and errors are visible.
pub fn attach() -> bool {
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return false;
        }
        let conout: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
        for std in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let h = GetStdHandle(std);
            // Leave handles alone if the user redirected them to a file/pipe.
            if h.is_null() || h == INVALID_HANDLE_VALUE {
                let c = CreateFileW(conout.as_ptr(), GENERIC_WRITE, FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut());
                SetStdHandle(std, c);
            }
        }
        true
    }
}

pub fn free() {
    unsafe { FreeConsole() };
}
