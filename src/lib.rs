use zed_extension_api;

struct GoogleSqlLspExtension {}

impl zed_extension_api::Extension for GoogleSqlLspExtension {
    fn new() -> Self {
        Self {}
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> zed_extension_api::Result<zed_extension_api::Command> {
        if let Some(path) = worktree.which("googlesql_lsp") {
            Ok(zed_extension_api::Command {
                command: path,
                args: Vec::new(),
                env: Vec::new(),
            })
        } else {
            Err("googlesql_lsp is not on your PATH".to_string())
        }
    }
}

zed_extension_api::register_extension!(GoogleSqlLspExtension);
