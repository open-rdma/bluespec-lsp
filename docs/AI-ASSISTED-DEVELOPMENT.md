# AI-Assisted Development Workflow

How bluespec-lsp was built by one person + Claude Code in a few days — without writing a single line of Rust by hand.

## The Pipeline

```
Plan Mode  →  Agent Teams  →  Code Review  →  Ralph Loop
(design)     (implement)      (verify)        (iterate)
```

<img width="1536" height="1024" alt="8f674911317ffee43720eafe92e1e45c" src="https://github.com/user-attachments/assets/3439ea3c-423a-477f-9e5c-1c132d65ed9c" />


## 1. Plan Mode — Design Before You Build

**What**: Before writing any code, Claude Code explores the codebase and produces a structured implementation plan — phases, files to change, risks, dependencies.

**Why**: The most expensive mistake in AI-assisted development is letting the AI dive in and write hundreds of lines before you realize it's going the wrong direction. Plan Mode front-loads architectural decisions. You review and approve the plan *before* a single line is written.

**How**: `/plan <task description>`

**Used for**: Module decomposition, tree-sitter C parser integration strategy, symbol table concurrency design, data flow architecture.

## 2. Agent Teams — Parallelize Independent Work

**What**: Dispatch multiple specialized AI agents simultaneously, each with its own isolated context window, to work on independent subtasks in parallel.

**Why**: An LSP involves many loosely-coupled modules (parser, symbol table, hover, completion, VS Code client). Sequential development wastes time. Parallel agents with isolated contexts produce more reliable results than one agent context-switching across concerns.

**How**: Launch 2–3 agents in a single message, each with a focused prompt and clear scope.

**Used for**: Grammar coverage gaps, symbol navigation edge cases, VSIX packaging scripts — three independent tasks, three agents, done concurrently.

## 3. Code Review — Verify Before You Merge

**What**: After AI produces code, an independent review agent inspects it across seven categories: correctness, security, performance, type safety, pattern compliance, completeness, and maintainability.

**Why**: AI-generated code is fast but not inherently trustworthy. A separate agent with fresh context catches issues the generating agent missed — unchecked unwraps, lock ordering bugs, silently swallowed errors. Unlike a human reviewer, it never gets fatigued on the 40th file.

**How**: `/review` or use the `code-reviewer` skill.

**Used for**: Every feature before commit. CRITICAL findings block, HIGH findings must be addressed.

## 4. Ralph Loop — Iterate Autonomously

**What**: Feed the same prompt to Claude repeatedly. Each iteration, it sees its own previous output (file diffs, test results, git status) and improves. Stops when a completion criterion is met or max iterations are reached.

**Why**: For tasks with objective success criteria (all tests pass, all syntax nodes correctly classified), autonomous iteration is faster and more thorough than manual fix-and-check cycles. Set it, walk away, come back to green.

**How**: `/ralph-loop "<prompt>" --completion-promise "<success marker>" --max-iterations <N>`

**Used for**: Grammar coverage completion, test failure fixes, error recovery edge cases.

## The Human's Role

The human is not a passenger. Five responsibilities remain:

1. **Architecture**: Technology choices (tree-sitter + tower-lsp), module boundaries, data flow — these are human decisions informed by AI suggestions.
2. **Prompting**: Writing clear, specific prompts with goals, constraints, edge cases, and expected output format. Better prompts → better code.
3. **Code review**: AI review is a first pass; the human makes final judgment calls on trade-offs and design quality.
4. **Manual testing**: AI sees `cargo test` pass and calls it done. The human opens a real `.bsv` file in VS Code Extension Host and exercises every feature — hover, jump-to-def, completion, symbol search — to find real-world bugs.
5. **Prioritization**: AI has no sense of "what matters most." The human decides feature order, scope boundaries, and when something is good enough to ship.

> **TL;DR**: Human as architect and QA. AI as programmer.
