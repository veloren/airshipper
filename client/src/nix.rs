use crate::{
    ClientError, Result,
    consts::{SERVER_CLI_FILE, VOXYGEN_FILE},
};
use std::{ffi::OsString, path::Path};

const OS_RELEASE: &str = "/etc/os-release";

/// Get patcher for patching voxygen.
fn get_voxygen_patcher() -> Option<OsString> {
    std::env::var_os("VELOREN_VOXYGEN_PATCHER")
}

/// Get patcher for patching server-cli.
fn get_server_patcher() -> Option<OsString> {
    std::env::var_os("VELOREN_SERVER_CLI_PATCHER")
}

/// Check if we are on NixOS.
pub fn is_nixos() -> Result<bool> {
    let os_release = Path::new(OS_RELEASE);
    Ok(os_release.exists() && std::fs::read_to_string(os_release)?.contains("ID=nixos"))
}

/// Patches an executable file. Required for NixOS.
///
/// Note: it's synchronous!
pub fn patch(profile_directory: &Path, file: &str) -> Result<()> {
    tracing::info!("Patching an executable file for NixOS");

    let patcher = match file {
        VOXYGEN_FILE => get_voxygen_patcher().ok_or_else(|| {
            ClientError::Custom("patcher binary was not detected.".to_string())
        })?,
        SERVER_CLI_FILE => get_server_patcher().ok_or_else(|| {
            ClientError::Custom("patcher binary was not detected.".to_string())
        })?,
        _ => return Err(ClientError::Custom("Unknown file to patch".to_string())),
    };

    // Patch the file
    tracing::info!("Executing {patcher:?} on directory {profile_directory:?}");
    let output = std::process::Command::new(patcher)
        .current_dir(profile_directory)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Return error if patcher fails
    if !output.status.success() {
        return Err(ClientError::Custom(format!(
            "Failed to patch file for NixOS: patcher output:\nstderr: {stderr}\nstdout: \
             {stdout}",
        )));
    } else {
        tracing::info!("Patched executable file:\n{stdout}");
    }

    Ok(())
}
