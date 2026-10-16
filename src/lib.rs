use zed_extension_api::{self as zed, settings::LspSettings};

const LANGUAGE_SERVER: &str = "googlesql_lsp";

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
        eprintln!("[GoogleSQL] OS: {:?}, Arch: {:?}", os, arch);

        let os: &str = match os {
            zed::Os::Mac => "macos",
            zed::Os::Linux => "linux",
            zed::Os::Windows => panic!("Windows is not supported. Please use WSL"),
        };

        let arch: &str = match arch {
            zed::Architecture::X8664 => "x86_64",
            zed::Architecture::Aarch64 => "aarch64",
            zed::Architecture::X86 => panic!("x86 is not supported"),
        };
        eprintln!("[GoogleSQL] Platform: {}_{}", os, arch);

        if let Some(binary_path) = &self.cached_binary_path {
            eprintln!("[GoogleSQL] Using cached binary: {}", binary_path);
            return Ok(zed::Command {
                command: binary_path.clone(),
                args: Vec::new(),
                env: Vec::new(),
            });
        }

        let release = zed::latest_github_release(
            "Oddys/googlesql-lsp",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;
        eprintln!("[GoogleSQL] Release: {:?}", release);

        let binary_name = format!("{}_{}_{}", LANGUAGE_SERVER, os, arch);
        let url = release
            .assets
            .iter()
            .find(|asset| asset.name.starts_with(&binary_name))
            .map(|asset| asset.download_url.clone())
            .ok_or(format!("No asset {}", binary_name))?;
        eprintln!("[GoogleSQL] Url: {}", url);

        let binary_dir = release.version;
        let binary_path = format!("{}/{}", binary_dir, binary_name);
        zed::download_file(&url, &binary_dir, zed::DownloadedFileType::GzipTar)?;

        self.cached_binary_path = Some(binary_path.clone());

        Ok(zed::Command {
            command: binary_path,
            args: Vec::new(),
            env: Vec::new(),
        })
    }
}

zed::register_extension!(GoogleSqlLspExtension);
