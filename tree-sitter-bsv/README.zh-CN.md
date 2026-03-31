# tree-sitter-bsv

注：由大型语言模型（LLM）主导实现。

这是为 Bluespec SystemVerilog (BSV) 编写的 Tree-sitter 语法。

目录说明
- 语法定义：`src/grammar.json`
- 节点类型：`src/node-types.json`
- 生成的解析器：`src/parser.c`（由 tree-sitter 生成）
- 测试用例：`test/`（用于语法验证的示例 .bsv 文件）

快速开始

前置依赖：
- `tree-sitter` CLI（可通过 `npm install -g tree-sitter-cli` 安装）

生成并测试解析器：

```bash
cd tree-sitter-bsv
tree-sitter generate    # 根据 grammar.json 生成 C 解析器文件
tree-sitter test        # 运行语法测试（使用 test/ 中的文件）
```

使用说明
- 可以将该语法嵌入到各种语言绑定（例如 node、rust 等）中，参见 Tree-sitter 的官方嵌入说明。
- 如果修改了 `src/grammar.json`，请运行 `tree-sitter generate` 重新生成解析器。

贡献
- 欢迎提交 issue 或 PR 修正语法、节点类型或测试用例。请保持改动小且附带能复现问题的测试文件。

许可
- 请参阅仓库中的 LICENSE 文件获取许可信息。
