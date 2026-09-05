//! Selects the owner of MCP OAuth refresh and credential persistence.

/// MCP OAuth policy pinned for the lifetime of a connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum McpOAuthRefreshMode {
    /// Keep localcode's existing refresh and persistence path.
    #[default]
    Legacy,
    /// Let RMCP coordinate refresh through localcode's credential store.
    Coordinated,
}
