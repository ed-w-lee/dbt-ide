use tower_lsp::{
    jsonrpc::Result,
    lsp_types::{
        InitializeParams, InitializeResult, ServerCapabilities, TextDocumentSyncCapability,
        TextDocumentSyncKind,
    },
    Client, LanguageServer,
};

#[derive(Debug)]
struct Backend {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
