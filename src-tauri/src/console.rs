//! Console handling for Windows.
//!
//! The release executable is a GUI program, so it starts without a console.
//! When it is asked to print (`--version`) or to run in the terminal
//! (`--headless`), it has to get one first. On other systems these are no-ops.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    const STD_INPUT_HANDLE: u32 = -10i32 as u32;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const FILE_TYPE_UNKNOWN: u32 = 0;

    #[link(name = "kernel32")]
    extern "system" {
        fn AllocConsole() -> i32;
        fn AttachConsole(process_id: u32) -> i32;
        fn GetStdHandle(std_handle: u32) -> *mut c_void;
        fn GetFileType(handle: *mut c_void) -> u32;
    }

    /// True if the handle is a console, pipe or file we can use.
    fn usable(std_handle: u32) -> bool {
        // SAFETY: plain Win32 queries; both accept any handle value.
        unsafe {
            let handle = GetStdHandle(std_handle);
            !handle.is_null() && handle as isize != -1 && GetFileType(handle) != FILE_TYPE_UNKNOWN
        }
    }

    pub fn attach_for_output() {
        if !usable(STD_OUTPUT_HANDLE) {
            // SAFETY: no arguments to get wrong; failure (no parent console) is fine.
            unsafe {
                AttachConsole(ATTACH_PARENT_PROCESS);
            }
        }
    }

    pub fn ensure_interactive() {
        // Redirected input and output (pipes, files, Git Bash) are used as they are.
        if usable(STD_INPUT_HANDLE) && usable(STD_OUTPUT_HANDLE) {
            return;
        }
        // A GUI program started from cmd or PowerShell cannot share that
        // terminal's input (the shell keeps reading it), so open our own.
        // SAFETY: no arguments; if it fails we simply have no console.
        unsafe {
            AllocConsole();
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn attach_for_output() {}
    pub fn ensure_interactive() {}
}

/// Make `println!` visible when started from a terminal.
pub fn attach_for_output() {
    imp::attach_for_output();
}

/// Make sure there is a console to read commands from and print to.
pub fn ensure_interactive() {
    imp::ensure_interactive();
}
