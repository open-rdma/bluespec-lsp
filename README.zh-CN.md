# Bluespec LSP 工作区

注：由大型语言模型（LLM）主导实现。

该工作区包含两个相关子项目，共同为 Bluespec SystemVerilog (BSV) 提供语言支持：

- `tree-sitter-bsv` — BSV 的 Tree-sitter 语法与解析器。
- `bsv-language-server` — 基于 Rust 的语言服务器实现以及对应的 VS Code 客户端扩展。

快速上手（开发）

1. 在 VS Code 中打开此工作区目录 `bluespec_lsp`。
2. 按照子项目的说明构建：
   - `tree-sitter-bsv`：运行 `tree-sitter generate` 以及 `tree-sitter test` 来验证语法。
   - `bsv-language-server`：编译 Rust 服务端（`cargo build --release`）并编译客户端（`npm install && npm run compile`）。
3. 迭代扩展时：在 VS Code 中打开 `bsv-language-server`，进入左侧 Run and Debug，选择 `Launch Extension` 启动 Extension Development Host。在新窗口中打开 `.bsv` 文件以测试功能（悬停、补全、文档符号、转到定义）。

推荐阅读位置
- `tree-sitter-bsv/src/` — 语法与解析器源文件。
- `bsv-language-server/src/` — 服务器实现（Rust）。
- `bsv-language-server/client/` — VS Code 客户端（TypeScript）。

贡献
- 欢迎通过 issue 或 PR 提交语法修复、LSP 功能或客户端改进。
