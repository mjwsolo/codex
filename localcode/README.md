# localcode profile for this fork

This branch adapts codex as localcode's agent front end:

- **Local inference only.** `localcode/config.toml` points codex at localcode's
  bundled turboquant `llama-server` over the Responses API (`/v1/responses`),
  with an isolated `CODEX_HOME` so a user's own `~/.codex` is never touched.
- **No account.** Auth is a static env key against the local server; the
  ChatGPT login path is never taken.
- **Asks before anything risky** via codex's own approval system
  (`approval_policy = "on-request"`, `sandbox_mode = "workspace-write"`),
  which replaces the approval layer localcode ships in its Python TUI.
- **Branding**: the exec banner and TUI welcome say localcode.

The launcher and end-to-end journey tests live in the localcode repo on the
`integrate/codex-frontend` branch (`codex-agent/`).
