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

| Feature | Status |
|---------|--------|
| Syntax highlighting | ✅ Basic |
| Document symbols | ✅ Flat list |
| Go-to-definition | ✅ Same & cross-file |
| Hover information | ✅ Constant expansion + symbol info |
| Completion | ✅ Basic keywords & local symbols |
| Workspace symbols | ✅ Across all open files |
| Error recovery | ✅ Symbol extraction from malformed code |
| `#define` constant expansion | ✅ With type functions (TAdd, TSub, etc.) |
| BVI / BDPI import syntax | ✅ Supported |

**See [Feature Roadmap](#feature-roadmap) below for what's coming next.**

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
4. Try: hover, completion (`Ctrl+Space`), go-to-definition (`F12`), document symbols (`Ctrl+Shift+O`)

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
├── CLAUDE.md                       # Claude Code project guide
├── CONTRIBUTING.md                 # Contribution guidelines
├── CODE_OF_CONDUCT.md              # Contributor Covenant
├── SECURITY.md                     # Security policy
└── CHANGELOG.md                    # Version history
```

## Feature Roadmap

### Phase 1 — Foundation ✅ (Completed)

| Feature | Status |
|---------|--------|
| Tree-sitter BSV grammar | ✅ |
| Rust LSP server framework | ✅ |
| VS Code extension client | ✅ |
| Community health files | ✅ |

### Phase 2 — Code Quality ✅ (Completed)

| Feature | Status |
|---------|--------|
| Dead code removal | ✅ |
| Unused dependency cleanup | ✅ |
| Deprecation fixes | ✅ |
| Constant expander hardening + tests | ✅ |
| Hex constant support + tests | ✅ |
| Clippy-clean codebase | ✅ |

### Phase 3 — CI/CD

| Feature | Status |
|---------|--------|
| GitHub Actions CI (test/clippy/fmt) | ✅ |
| Multi-platform release workflow | ❌ |
| Dependabot dependency updates | ❌ |

### Phase 4 — Core LSP Features

| Feature | Status |
|---------|--------|
| Diagnostics (syntax error reporting) | ❌ |
| Find references | ❌ |
| Hierarchical document symbols | ❌ |
| Improved completion (keywords + cross-file) | ❌ |

### Phase 5 — Advanced LSP Features

| Feature | Status |
|---------|--------|
| Semantic tokens (syntax highlighting) | ❌ |
| Code folding | ✅ |
| Code actions (quick fixes) | ❌ |
| Signature help | ❌ |
| Improved TextMate grammar | ❌ |
| Incremental document sync | ❌ |

## Documentation

- [CONTRIBUTING.md](CONTRIBUTING.md) — How to contribute
- [CHANGELOG.md](CHANGELOG.md) — Version history
- [README.zh-CN.md](README.zh-CN.md) — 中文文档
- [SECURITY.md](SECURITY.md) — Security policy
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — Code of conduct
- [AI-ASSISTED-DEVELOPMENT.md](AI-ASSISTED-DEVELOPMENT.md) — Development process notes

## License

This project is licensed under the Apache 2.0 License — see the [LICENSE](LICENSE) file for details.