use zed_extension_api::{self as zed, settings::LspSettings};

const LANGUAGE_SERVER: &str = "googlesql_lsp";

/// Checks if a given path exists and it is a file
fn file_exists(path: &str) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

/// Converts a version string (`vX.Y.Z`) into a vector of numbers (`[X, Y, Z]`). Helpful for sorting
fn version_to_numeric(version: &str) -> Vec<u32> {
    version
        .trim_start_matches('v')
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// Finds the newest binary already on disk
fn latest_local_binary(binary_name: &str) -> Option<String> {
    let dir = std::fs::read_dir(".")
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|dir| file_exists(&format!("{}/{}", dir, binary_name)))
        .max_by_key(|dir| version_to_numeric(dir))?;
    Some(format!("{}/{}", dir, binary_name))
}

/// Makes sure the latest release binary is on disk and returns its path
fn update_binary(
    language_server_id: &zed::LanguageServerId,
    binary_name: &str,
) -> zed::Result<String> {
    zed::set_language_server_installation_status(
        language_server_id,
        &zed::LanguageServerInstallationStatus::CheckingForUpdate,
    );
    let release = zed::latest_github_release(
        "Oddys/googlesql-lsp",
        zed::GithubReleaseOptions {
            require_assets: true,
            pre_release: false,
        },
    )?;
    eprintln!("[GoogleSQL] Release: {:?}", release);

    let asset_name = format!("{}.tar.gz", binary_name);
    let url = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .map(|asset| asset.download_url.clone())
        .ok_or(format!("Asset not found: {}", asset_name))?;

    let binary_dir = release.version;
    let binary_path = format!("{}/{}", binary_dir, binary_name);
    if file_exists(&binary_path) {
        eprintln!("[GoogleSQL] Latest release binary is already downloaded");
        return Ok(binary_path);
    }

    eprintln!("[GoogleSQL] Downloading release binary...");
    zed::set_language_server_installation_status(
        language_server_id,
        &zed::LanguageServerInstallationStatus::Downloading,
    );
    let installed = zed::download_file(&url, &binary_dir, zed::DownloadedFileType::GzipTar)
        .and_then(|()| {
            if !file_exists(&binary_path) {
                return Err(format!("{} is missing from {}", binary_name, asset_name));
            }
            zed::make_file_executable(&binary_path)
        });
    // Checked before the cleanup below, so that a bad release keeps the older versions
    if let Err(err) = installed {
        // A partial extraction must not be picked up as the newest local binary
        std::fs::remove_dir_all(&binary_dir).ok();
        return Err(err);
    }

    // Cleans up old binary versions, if any
    if let Ok(entries) = std::fs::read_dir(".") {
        for entry in entries.flatten() {
            if entry.file_name() != binary_dir.as_str() {
                std::fs::remove_dir_all(entry.path()).ok();
            }
        }
    }

    Ok(binary_path)
}

struct GoogleSqlLspExtension {
    /// Used when the user restarts the language server without closing Zed
    cached_binary_path: Option<String>,
}

impl zed::Extension for GoogleSqlLspExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        // `lsp.googlesql.binary.path` in Zed settings
        let local_binary = LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|s| s.binary);
        let local_path = local_binary.as_ref().and_then(|b| b.path.clone());
        if let Some(path) = local_path {
            eprintln!("[GoogleSQL] Using local binary: {}", path);
            return Ok(zed::Command {
                command: path,
                args: local_binary.and_then(|b| b.arguments).unwrap_or_default(),
                env: worktree.shell_env(),
            });
        }

        let (os, arch) = zed::current_platform();

        let os: &str = match os {
            zed::Os::Mac => "macos",
            zed::Os::Linux => "linux",
            zed::Os::Windows => return Err("Windows is not supported. Please use WSL".to_string()),
        };

        let arch: &str = match arch {
            zed::Architecture::X8664 => "x86_64",
            zed::Architecture::Aarch64 => "aarch64",
            zed::Architecture::X86 => return Err("x86 is not supported".to_string()),
        };

        let cached_binary_path = self.cached_binary_path.as_ref();
        if let Some(binary_path) = cached_binary_path.filter(|path| file_exists(path)) {
            eprintln!("[GoogleSQL] Using cached binary: {}", binary_path);
            return Ok(zed::Command {
                command: binary_path.clone(),
                args: Vec::new(),
                env: Vec::new(),
            });
        }

        let binary_name = format!("{}_{}_{}", LANGUAGE_SERVER, os, arch);
        let binary_path = match update_binary(language_server_id, &binary_name) {
            Ok(binary_path) => {
                self.cached_binary_path = Some(binary_path.clone());
                binary_path
            }
            Err(err) => {
                // Not cached, so that a server restart retries the update
                let binary_path = latest_local_binary(&binary_name).ok_or(err.clone())?;
                eprintln!(
                    "[GoogleSQL] Update failed ({}). Falling back to the already installed binary: {}",
                    err, binary_path
                );
                binary_path
            }
        };

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::None,
        );

        Ok(zed::Command {
            command: binary_path,
            args: Vec::new(),
            env: Vec::new(),
        })
    }
}

zed::register_extension!(GoogleSqlLspExtension);

#[cfg(test)]
mod tests {
    use super::version_to_numeric;

    #[test]
    fn versions_sort_numerically() {
        assert!(version_to_numeric("v0.10.0") > version_to_numeric("v0.9.0"));
        assert!(version_to_numeric("v1.0.0") > version_to_numeric("v0.99.99"));
        assert!(version_to_numeric("v1.2") > version_to_numeric("v1.1.1"));
    }
}
