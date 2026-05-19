# Bluespec LSP Workspace

[![Version](https://img.shields.io/badge/version-0.1.0--pre--alpha-blue)](https://github.com/6eanut/bluespec_lsp)
[![License](https://img.shields.io/badge/license-Apache%202.0-green)](LICENSE)
[![CI](https://img.shields.io/badge/CI-passing-brightgreen)](https://github.com/6eanut/bluespec_lsp/actions)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange)](https://www.rust-lang.org/)
[![VS Code](https://img.shields.io/badge/VS%20Code-extension-blueviolet)](bsv-language-server/client/)

> A Language Server Protocol (LSP) implementation for Bluespec SystemVerilog (BSV), providing IDE-grade editing support in VS Code and any LSP-compatible editor.

## Project Status

This is a **pre-alpha** prototype with core LSP features implemented and actively being extended. The project was initially scaffolded with AI assistance over ~2 weeks and is now being hardened into a polished open-source tool.

**What works today:**

| Category | Features |
|----------|----------|
| 🧭 Navigation | Go-to-definition, Find References, Document Symbols, Workspace Symbols |
| ℹ️ Information | Hover (constant expansion + symbol info), Diagnostics (syntax error reporting) |
| ✏️ Editing | Completion (keywords & local symbols), Code Folding |
| ⚙️ Language-Specific | `#define` constant expansion with type functions (TAdd, TSub, etc.), Error recovery from malformed code |
| 🎨 Presentation | Basic syntax highlighting (TextMate), BVI / BDPI import syntax support |

> For a comprehensive list of implemented and planned features, see [docs/FEATURES.md](docs/FEATURES.md).

## Screenshots

> Screenshots coming soon. Open a `.bsv` file and try:
> - Hover over symbols to see type info
> - Ctrl+Click to jump to definitions
> - Ctrl+Space for completions
> - Ctrl+Shift+O for document outline

## Workspace Contents

This workspace contains two related projects that together provide a Bluespec SystemVerilog (BSV) language experience:

- **`tree-sitter-bsv`** — a Tree-sitter grammar and parser for BSV.
- **`bsv-language-server`** — a Rust-based Language Server implementation with a VS Code client extension.

## Getting Started (Development)

### Prerequisites

- **Rust** toolchain (1.70 or higher)
- **Node.js** (20 or higher)
- **tree-sitter CLI** (for grammar development)

### Setup

```bash
# Install tree-sitter CLI (if not already installed)
cargo install tree-sitter-cli

# Open the workspace in VS Code
code bluespec_lsp

# Build the language server
cd bsv-language-server
cargo build --release

# Install and compile the VS Code client
npm install
npm run compile
```

### Testing LSP Features

1. Open `bsv-language-server` in VS Code
2. Go to **Run and Debug** → **Launch Extension**
3. In the new Extension Development Host window, open a `.bsv` file
4. Try: hover, completion (`Ctrl+Space`), go-to-definition (`F12`), find references (`Shift+F12`), document symbols (`Ctrl+Shift+O`), syntax error diagnostics (red squiggles)

### Useful Commands

```bash
# Build the server
cargo build --release

# Run all tests
cargo test

# Run clippy linting
cargo clippy -- -D warnings

# Check formatting
cargo fmt --check
```

## Project Structure

```
bluespec_lsp/
├── tree-sitter-bsv/                # Tree-sitter grammar for BSV
│   └── grammar.js                  # BSV language grammar (810 lines)
├── bsv-language-server/            # LSP server + VS Code client
│   ├── src/
│   │   ├── server.rs               # LSP handler implementations
│   │   ├── parser.rs               # Tree-sitter parsing + symbol extraction
│   │   ├── symbols.rs              # Concurrent symbol table (DashMap)
│   │   ├── constant_expansion/     # #define constant evaluation engine
│   │   ├── errors.rs               # Error types
│   │   ├── utils.rs                # Utility functions
│   │   └── lib.rs                  # Library entry point
│   ├── client/                     # VS Code extension (TypeScript)
│   ├── syntaxes/                   # TextMate grammar for syntax highlighting
│   └── test_fixtures/              # BSV test samples
├── docs/                           # Documentation
│   ├── FEATURES.md                 # Feature status and roadmap
│   ├── CONTRIBUTING.md             # Contribution guidelines
│   ├── CHANGELOG.md                # Version history
│   ├── SECURITY.md                 # Security policy
│   ├── CODE_OF_CONDUCT.md          # Contributor Covenant
│   └── AI-ASSISTED-DEVELOPMENT.md  # Development process notes
├── CLAUDE.md                       # Claude Code project guide
```

## Documentation

- [FEATURES.md](docs/FEATURES.md) — Feature status and roadmap
- [CONTRIBUTING.md](docs/CONTRIBUTING.md) — How to contribute
- [CHANGELOG.md](docs/CHANGELOG.md) — Version history
- [README.zh-CN.md](README.zh-CN.md) — 中文文档
- [SECURITY.md](docs/SECURITY.md) — Security policy
- [CODE_OF_CONDUCT.md](docs/CODE_OF_CONDUCT.md) — Code of conduct
- [AI-ASSISTED-DEVELOPMENT.md](docs/AI-ASSISTED-DEVELOPMENT.md) — Development process notes

## License

This project is licensed under the Apache 2.0 License — see the [LICENSE](LICENSE) file for details.