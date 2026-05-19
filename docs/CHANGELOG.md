# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Project foundation: tree-sitter grammar and Rust LSP server
- BSV language support for VS Code (syntax highlighting, language configuration)
- LSP features: document symbols, go-to-definition, hover, completion, workspace symbols
- `#define` constant expansion with type function evaluation (TAdd, TSub, TMul, TDiv, TLog, TExp, TMax, TMin)
- Error-tolerant symbol extraction from malformed BSV code
- BVI (Bluespec Verilog Interface) import syntax support
- BDPI (Bluespec Direct Programming Interface) import syntax support
- Multi-language tree-sitter bindings (Rust, Node.js, Python, Go, Swift)
- GitHub Actions release workflow (Windows x86_64 VSIX packaging)
- Apache 2.0 License
- Chinese and English documentation (README, CONTRIBUTING, TESTING)
- AI-Assisted Development workflow documentation

### Changed
- Release workflow now builds both Windows x86_64 and macOS ARM64 VSIX via build matrix
- Added `.vscodeignore` to reduce VSIX size
- Added `repository` field to `package.json`

### Fixed