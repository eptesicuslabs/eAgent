//! Platform-specific utilities — process management, shell detection, path searching.

use anyhow::Result;
use tracing::info;

/// Get the default shell for the current platform.
pub fn default_shell() -> &'static str {
    #[cfg(windows)]
    {
        "powershell.exe"
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL")
            .ok()
            .and_then(|s| {
                // Leak the string so we can return a &'static str
                // This is fine because this is called rarely.
                None // Fall through to default
            })
            .unwrap_or("/bin/bash")
    }
}

/// Get the default shell as an owned String (for non-static lifetime).
pub fn default_shell_owned() -> String {
    #[cfg(windows)]
    {
        "powershell.exe".to_string()
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    }
}

/// Kill a process tree (the process and all its descendants).
pub async fn kill_process_tree(pid: u32) -> Result<()> {
    #[cfg(windows)]
    {
        kill_process_tree_windows(pid)?;
    }
    #[cfg(not(windows))]
    {
        kill_process_tree_unix(pid)?;
    }
    info!(%pid, "Killed process tree");
    Ok(())
}

#[cfg(windows)]
fn kill_process_tree_windows(pid: u32) -> Result<()> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::*;
    use windows_sys::Win32::System::Threading::*;

    unsafe {
        // Open the process
        let process = OpenProcess(PROCESS_TERMINATE | PROCESS_SET_QUOTA, 0, pid);
        if process.is_null() {
            // Process may already be dead
            return Ok(());
        }

        // Try to create a job object and terminate through it
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if !job.is_null() {
            AssignProcessToJobObject(job, process);
            TerminateJobObject(job, 1);
            CloseHandle(job);
        } else {
            // Fallback: just terminate the process directly
            TerminateProcess(process, 1);
        }

        CloseHandle(process);
    }

    Ok(())
}

#[cfg(not(windows))]
fn kill_process_tree_unix(pid: u32) -> Result<()> {
    // Send SIGKILL to the entire process group
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    Ok(())
}

/// Find an executable on the system PATH.
pub fn find_on_path(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }

            #[cfg(windows)]
            {
                // Try common Windows extensions
                for ext in &[".exe", ".cmd", ".bat"] {
                    let with_ext = dir.join(format!("{}{}", name, ext));
                    if with_ext.exists() {
                        return Some(with_ext);
                    }
                }
            }

            None
        })
    })
}

/// Set up a process group for a child process (pre-spawn).
/// On Windows, this sets CREATE_NEW_PROCESS_GROUP.
/// On Linux, this sets the pgid.
#[cfg(windows)]
pub fn setup_process_group(cmd: &mut tokio::process::Command) {
    #[allow(unused_imports)]
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
    cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(windows))]
pub fn setup_process_group(cmd: &mut tokio::process::Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }
}
