# BSV Language Server — Feature Status

## Legend

| Mark | Meaning |
|------|---------|
| ✅ | Implemented |
| ◐ | Partially implemented / basic version |
| ❌ | Not yet implemented |

## Implemented Features

| Category | Feature | Status | Notes |
|----------|---------|--------|-------|
| 🧭 Navigation | Go-to-Definition | ✅ | Same-file and cross-file symbol lookup |
| 🧭 Navigation | Find References | ✅ | Combined definition + usage locations |
| 🧭 Navigation | Document Symbols | ✅ | Flat list |
| 🧭 Navigation | Workspace Symbols | ✅ | Across all open files |
| ℹ️ Information | Hover | ✅ | Constant expansion + symbol information |
| ℹ️ Information | Diagnostics | ✅ | Real-time syntax error underlines |
| ✏️ Editing | Completion | ◐ | Basic keywords + local symbols |
| ✏️ Editing | Folding Range | ✅ | Modules, rules, functions, interfaces, etc. |
| ⚙️ Language | `#define` constant expansion | ✅ | Including type functions (TAdd, TSub, etc.) |
| ⚙️ Language | Error recovery | ✅ | Symbol extraction from malformed code |
| 🎨 Presentation | TextMate syntax highlighting | ✅ | Basic level |
| ✏️ Editing | Formatting | ✅ | AST-guided line reformatting, 3-space indent |
| 🧭 Navigation | Document Highlight | ✅ | Per-document symbol highlighting via SymbolTable |

## Planned Features

| Category | Feature | Notes |
|----------|---------|-------|
| 🧭 Navigation | Go-to-Type-Definition | Requires type system analysis |
| 🧭 Navigation | Go-to-Implementation | |
| 🧭 Navigation | Call Hierarchy | Requires function call graph analysis |
| 🧭 Navigation | Type Hierarchy | |
| ℹ️ Information | Signature Help | ✅ | Function / method parameter extraction |
| ✏️ Editing | Completion (cross-file) | Leverages existing SymbolTable |
| ✏️ Editing | Code Actions | |
| ✏️ Editing | Rename | Leverages existing references index |
| ✏️ Editing | Selection Range | Leverages existing AST traversal |
| 📖 Semantic | Semantic Tokens | Advanced syntax highlighting |
| 📖 Semantic | Inlay Hints | |
| 📖 Semantic | Code Lens | |
| 📖 Semantic | Document Link | |
| ⚙️ Language | Hierarchical Document Symbols | Symbol tree structure |
| ⚙️ Language | Incremental Document Sync | Incremental edit tracking |
| 🔧 Workspace | Configuration change handling | `didChangeConfiguration` |
| 🔧 Workspace | Watched file change handling | `didChangeWatchedFiles` |
| 🔧 Workspace | Workspace folder change handling | `didChangeWorkspaceFolders` |
| 🔧 Diagnostics | Workspace Diagnostics | `workspace/diagnostic` |