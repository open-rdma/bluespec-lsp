# BSV Language Server (bsv-language-server)

Note: LLM-led implementation.

Bluespec SystemVerilog (BSV) Language Server implemented in Rust with a VS Code client.

Repository layout (important parts):
- `src/` — Rust language-server implementation (uses `tower-lsp`).
- `client/` — VS Code extension client (TypeScript). Entry point: `client/extension.ts`.
- `target/` — build output for the Rust server (binary will appear in `target/release/`).
- `syntaxes/` — TextMate grammar files used by the extension.
- `package.json` — VS Code extension manifest and build scripts.

Prerequisites
- Rust + Cargo (for server)
- Node.js + npm (for building the VS Code client)
- VS Code (for extension development/debug)

Build and run (development)

1. Build the Rust language server (release recommended):

```bash
cd bsv-language-server
cargo build --release
# binary will be at target/release/bsv-language-server
```

2. Install and compile the extension client:

```bash
# from repository root (bsv-language-server)
npm install
npm run compile   # compiles client/ TypeScript to client/out
```

3. Launch the extension in VS Code (Dev host):
- Open the `bsv-language-server` folder in VS Code.
- Open the Run and Debug view (left sidebar).
- Select `Launch Extension` (or press F5). VS Code will open a new Extension Development Host window.
- In the new window, open or create a `.bsv` file and try features: symbol outline, hover, completion, and `Go to Definition` (F12 or right-click → "Go to Definition").

Notes about where the extension finds the server
- The client tries to use a configured path `bsv.languageServer.path` (see `package.json` configuration). If empty, it falls back to `../target/release/bsv-language-server` relative to the client bundle path, or the `PATH`-installed binary named `bsv-language-server`.

Useful npm scripts (from `package.json`):
- `npm run compile` — compile the TypeScript client (`client/out/extension.js`).
- `npm run watch` — continuous compile during client development.

Developing tips
- When iterating on the Rust server, rebuild `cargo build` and then restart the extension host (stop and relaunch from the debugger) or use the `bsv.restartServer` command from the Command Palette.
- Use the Output panel and select "BSV Language Server" to view server logs.

Configuration
- `bsv.languageServer.path` — explicit path to the server executable
- `bsv.languageServer.trace.server` — tracing level: `off`, `messages`, `verbose`
- `bsv.languageServer.enable` — enable/disable the language server

License and contributions
- See repository LICENSE files. Contributions welcome via PRs and issues.
