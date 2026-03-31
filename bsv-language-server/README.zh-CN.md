# BSV 语言服务器（bsv-language-server）

注：由大型语言模型（LLM）主导实现。

基于 Rust 的 Bluespec SystemVerilog (BSV) 语言服务器，配套一个 VS Code 客户端扩展。

重要目录
- `src/` — Rust 实现，使用 `tower-lsp`。
- `client/` — VS Code 扩展客户端（TypeScript），入口为 `client/extension.ts`。
- `target/` — Rust 的构建输出（可在 `target/release/` 找到可执行文件）。
- `syntaxes/` — 扩展使用的 TextMate 语法文件。
- `package.json` — 扩展清单与构建脚本。

前置环境
- 安装 Rust + Cargo
- 安装 Node.js + npm
- 安装 VS Code（用于扩展开发与调试）

开发构建步骤

1. 构建 Rust 语言服务器（建议 release 模式）：

```bash
cd bsv-language-server
cargo build --release
# 可执行文件位于 target/release/bsv-language-server
```

2. 安装并编译扩展客户端：

```bash
# 在 bsv-language-server 目录下
npm install
npm run compile   # 编译 client/ 的 TypeScript
```

3. 在 VS Code 中运行扩展（开发模式）：
- 在 VS Code 中打开 `bsv-language-server` 文件夹。
- 进入左侧的 Run and Debug 面板。
- 选择 `Launch Extension`，按 F5 启动。VS Code 将打开一个新的 Extension Development Host 窗口。
- 在新窗口中打开或创建一个 `.bsv` 文件，试用功能：符号大纲、悬停信息、补全，以及“转到定义”（F12 或 右键 → 转到定义）。

关于扩展如何找到语言服务器
- 扩展会读取配置项 `bsv.languageServer.path`（参见 `package.json` 中的 configuration）。如果为空，扩展会尝试使用 `client` 相对路径 `../target/release/bsv-language-server`，或者系统 PATH 中名为 `bsv-language-server` 的可执行文件。

开发提示
- 修改 Rust 代码后重新运行 `cargo build`，并在扩展宿主（Extension Development Host）中重启扩展；也可使用命令面板中的 `BSV: Restart Language Server`。
- 在 VS Code 的 Output 面板选择 “BSV Language Server” 查看日志和调试信息。

配置项（摘录）
- `bsv.languageServer.path` — 指定语言服务器可执行文件路径
- `bsv.languageServer.trace.server` — 通信追踪级别：`off`、`messages`、`verbose`
- `bsv.languageServer.enable` — 启用或禁用语言服务器

许可与贡献
- 详见仓库 LICENSE。欢迎通过 issues 或 PR 提交改进建议和补丁。
