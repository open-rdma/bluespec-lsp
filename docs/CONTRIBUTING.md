# Contributing to BSV Language Server

Thank you for your interest in contributing to the Bluespec SystemVerilog (BSV) Language Server! This document provides guidelines and information to help you get started with development, testing, and contributing to the project.

## Table of Contents

- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Testing](#testing)
- [Contribution Process](#contribution-process)
- [Good First Issues](#good-first-issues)
- [Debugging Tips](#debugging-tips)

## Development Setup

### Prerequisites

- **Rust** toolchain (1.70 or higher) - for building the language server
- **Node.js** (20 or higher) - for building the VS Code client
- **VS Code** - for extension development and debugging
- **Git** - for version control

### Getting Started

1. Clone the repository:

   ```bash
   git clone https://github.com/6eanut/bluespec_lsp.git
   cd bluespec_lsp
   ```
2. Build the Rust language server:

   ```bash
   cd bsv-language-server
   cargo build --release
   # Binary will be at target/release/bsv-language-server
   ```
3. Install and compile the VS Code client:

   ```bash
   npm install
   npm run compile  # Compiles TypeScript to client/out/
   ```
4. Start development:

   - Open the `bsv-language-server` folder in VS Code
   - Use **Run and Debug** → **Launch Extension** to start an Extension Development Host
   - Open or create `.bsv` files in the development host to test LSP features

### Development Workflow

- The extension automatically uses the bundled server binary when available
- When iterating on the Rust server, rebuild with `cargo build` and restart the extension host
- Use the `bsv.restartServer` command from the Command Palette to restart the language server without restarting VS Code
- View server logs in the Output panel by selecting "BSV Language Server"

## Project Structure

### Core Components

```
bsv-language-server/
├── src/
│   ├── server.rs          # Main LSP server implementation
│   ├── parser.rs          # Tree-sitter parser with error recovery
│   ├── symbols.rs         # Symbol table management
│   ├── constant_expansion/# Constant evaluation system
│   └── lib.rs             # Main library exports
├── client/
│   └── extension.ts       # VS Code extension entry point
├── test_fixtures/         # Test BSV code samples
└── scripts/               # Build and test scripts
```

### Key Features

- **Error Recovery**: Extracts symbols from malformed BSV code by traversing ERROR nodes
- **Constant Expansion**: Evaluates `#define` constants with nested type functions (TAdd, TSub, TMul, etc.)
- **LSP Features**: Document symbols, go-to-definition, hover, completion, workspace symbols
- **Performance**: Uses concurrent data structures and caching for efficient operations

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run with verbose output (shows detailed test results)
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run constant expansion tests specifically
cargo test constant_expansion

# Run via provided script
./scripts/run_tests.sh
./scripts/run_tests.sh --verbose
```

### Test Fixtures

Test fixture files are located in `test_fixtures/`:

- `correct.bsv` - Syntactically correct BSV code
- `broken.bsv` - BSV code with intentional syntax errors
- `constants.bsv` - `#define` constant definitions for testing

These can be used for manual testing or as reference for writing new tests.

### Adding New Tests

To add a new test:

1. Add a test function in the appropriate file under `#[cfg(test)]`
2. Follow the existing test pattern:
   ```rust
   #[test]
   fn test_your_new_scenario() {
       let source = "your BSV code here";
       let parser = BsvParser::default();
       let tree = parser.parse(source).expect("parse failed");
       let symbols = parser.extract_symbols(&tree, source);

       assert!(symbols.iter().any(|s| s.name == "expected_symbol"));
   }
   ```
3. Run your test: `cargo test test_your_new_scenario`

## Contribution Process

### Branch Naming

Use descriptive branch names following this pattern:

- `feature/description` - for new features
- `fix/description` - for bug fixes
- `docs/description` - for documentation changes
- `chore/description` - for maintenance tasks

### Commit Messages

Follow conventional commit format:

- `feat: add hover support for constants`
- `fix: handle missing endmodule in error recovery`
- `test: add test cases for TMax function`
- `docs: update contributing guide`

### Pull Requests

1. Ensure all tests pass before submitting
2. Include relevant test cases for new functionality
3. Update documentation if needed (README.md, TESTING.md)
4. Reference any related issues in the PR description
5. Keep PRs focused on a single concern when possible

### Code Review

- All contributions require code review
- Be prepared to address feedback and make changes
- Reviewers will check for code quality, test coverage, and adherence to project patterns

## Good First Issues

[Here](https://github.com/6eanut/bluespec_lsp/issues?q=is%3Aissue%20state%3Aopen%20label%3A%22good%20first%20issue%22) are some beginner-friendly issues to help you get started:

---

We welcome contributions of all kinds! If you have questions or need help getting started, please open an issue or reach out to the maintainers.
