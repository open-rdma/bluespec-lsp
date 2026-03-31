# Bluespec LSP Workspace

Note: LLM-led implementation.

This workspace contains two related projects that together provide a Bluespec SystemVerilog (BSV) language experience:

- `tree-sitter-bsv` — a Tree-sitter grammar and parser for BSV.
- `bsv-language-server` — a Rust-based Language Server implementation with a VS Code client extension.

Getting started (development)

1. Open this workspace folder (`bluespec_lsp`) in VS Code.
2. See each subproject for build instructions:
   - `tree-sitter-bsv`: run `tree-sitter generate` and `tree-sitter test` to validate the grammar.
   - `bsv-language-server`: build the Rust server (`cargo build --release`) and compile the client (`npm install && npm run compile`).
3. To iterate on the extension: open `bsv-language-server` in VS Code and use Run and Debug → `Launch Extension` to start an Extension Development Host. Open a `.bsv` file there to test features (hover, completion, document symbols, go-to-definition).

Where to look next
- `tree-sitter-bsv/src/` — grammar and parser sources.
- `bsv-language-server/src/` — server implementation (Rust).
- `bsv-language-server/client/` — VS Code extension client (TypeScript).

Contributing
- Please open issues or PRs for grammar fixes, LSP features, or client improvements.
