<div align="center">

![ARIS banner](assets/banner.png)

# ARIS

**自主研究迭代系统 (Autonomous Research Iteration System)**

*面向 AI 编程 Agent 的自主实验循环*

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

把任何可量化的工程目标变成受控循环：Agent 修改代码、运行验证、记录指标、保留改进、回滚失败。

灵感来自 [Karpathy 的 autoresearch](https://github.com/karpathy/autoresearch)。

[快速上手](#快速上手) · [为什么用 ARIS](#为什么用-aris) · [工作原理](#工作原理) · [Skill 命令](#skill-命令) · [CLI](#cli)

[English](README_en.md)

</div>

---

## 这是什么？

ARIS 是一套**可移植的 skill 协议**，并配有可选的 Rust CLI。它让 AI 编程 Agent 用纪律化流程优化可量化目标，而不是靠“感觉还不错”来反复试错。

只使用 skill 时，它就是一套提示词级工作流；加上 CLI 后，你会得到一键安装、启动前检查、结构化实验日志、最佳结果查询、报告生成和实时 TUI。

**支持的 Agent：** Claude Code、OpenAI Codex、Cursor、Windsurf、OpenCode、Gemini CLI、GitHub Copilot，以及通用 `.agents/skills/` 工作流。

**效果演示：**

```
你：/aris
    Goal: Increase test coverage to 90%
    Verify: pytest --cov | grep TOTAL | awk '{print $NF}' | tr -d '%'

Agent：（自主运行 50+ 次实验）
       基线：72% → 最佳：91%（23 次保留，27 次丢弃，0 次崩溃）
```

**用自然语言布置任务 — 无需记忆结构化字段：**

```
你：/aris
    我现在要测试我的数据预处理管线是否完全 ok，每一项输出你都用 vllm
    进行验证，133 项数据全部测试通过就是我们的 goal

Agent：（自动理解目标，拆解验证步骤，逐项迭代）
       基线：98/133 通过 → 最终：133/133 通过（12 次保留，5 次丢弃）
```

你可以像和同事说话一样描述目标。Agent 会自动推断指标、验证方式和完成条件。

你设定目标。Agent 执行工作。你审查结果。

---

## 快速上手

### 1. 安装 ARIS

```bash
cargo install aris-cli
aris install claude-code
```

其他平台可以使用 `aris install codex`、`cursor`、`windsurf`、`opencode`、`gemini`、`copilot`、`agents` 或 `all`。

### 2. 配置可量化目标

```bash
aris init \
  --target-file src/api/routes.ts \
  --eval-command "npm run bench:api | grep p99 | awk '{print $NF}'" \
  --metric-name p99_latency \
  --metric-direction lower

aris doctor
```

`aris doctor` 会在 Agent 开始迭代前检查项目、git 状态、eval 命令和日志配置。

### 3. 启动循环

在你的 Agent 中输入：

```
/aris
Goal: Reduce API response time below 100ms
Scope: src/api/**/*.ts
Metric: p99 latency (ms)
Direction: lower
Verify: npm run bench:api | grep "p99" | awk '{print $NF}'
Guard: npm test
```

就这样。Agent 进入自主循环 — 修改、验证、保留或回滚 — 直到达成目标或你中断。

想手动安装也可以：把 `skills/aris/`、`commands/aris/` 和 `commands/aris.md` 复制到对应 Agent 的 skill/command 目录。协议本身不依赖 CLI；CLI 只是让安装、验证和报告更省心。

---

## 为什么用 ARIS？

AI 编程 Agent 很会改代码，但如果没有明确流程，长时间优化很容易变成松散试错。ARIS 提供的就是这个流程。

| 问题 | ARIS 的处理方式 |
|------|-----------------|
| Agent 一次改太多，难以定位原因 | 每轮只做一个原子变更 |
| “看起来更好”替代真实指标 | 只接受机械化指标验证 |
| 失败尝试污染工作区 | 通过 git 自动回滚 |
| 实验历史从上下文中丢失 | 结构化日志 + commit 历史 |
| 迭代久了开始钻指标空子 | 奖励作弊检测标记异常跳变 |
| 不同 Agent 需要不同提示词 | 一套可移植协议适配多个平台 |

---

## 工作原理

### 8 阶段循环

```
阶段 0：前置检查  — 验证环境（git 仓库、配置、eval 命令）
阶段 1：回顾      — 读取实验历史 + git log（每次迭代都执行）
阶段 2：构思      — 基于过往结果选择一个原子变更
阶段 3：修改      — 实现变更
阶段 4：提交      — 验证前先 git commit（便于干净回滚）
阶段 5：验证      — 运行 eval，提取指标
阶段 6：决策      — 机械决策：保留 / 丢弃 / 返工 / 崩溃
阶段 7：记录      — 记录结果
阶段 8：循环      — 回到阶段 1
```

### 决策规则

| 条件 | 动作 |
|------|------|
| 指标改善，guard 通过 | **保留** — 前进 |
| 指标改善，guard 失败 | **返工**（最多 2 次） |
| 指标持平或变差 | **丢弃** — git revert |
| eval 崩溃 | **自动修复**（最多 3 次） |
| 未产生代码变更 | **跳过** |
| 连续 15 次无改善 | **平台期** — 暂停并询问 |

### 核心原则

1. **每次迭代一个变更** — 原子性。出错时你知道确切原因。
2. **仅机械验证** — 不接受主观判断，只看数字。
3. **自动回滚** — 失败的变更通过 `git revert` 撤销。
4. **Git 即记忆** — 每次实验都有 commit，失败历史可见。
5. **超参数优先** — 最低风险，最高信号。架构改动放最后。
6. **奖励作弊检测** — 标记不合理的改善。

---

## Skill 命令

| 命令 | Agent 做什么 |
|------|-------------|
| `/aris` | 运行自主实验循环 |
| `/aris:plan` | 交互式向导：目标 → 范围、指标、方向、验证命令 |
| `/aris:debug` | 自主 bug 狩猎循环（科学方法） |
| `/aris:fix` | 迭代修复错误直到零错误 |

### 使用示例

```
# 无限循环 — 直到中断或触发平台期
/aris
Goal: Increase test coverage to 90%
Scope: src/**/*.ts
Verify: npx jest --coverage | grep 'All files' | awk '{print $4}'

# 有界 — 精确 25 次迭代
/aris
Goal: Reduce bundle size below 200KB
Iterations: 25

# 带 guard（回归防护）
/aris
Goal: Improve API response time
Verify: node bench.js | tail -1
Guard: npm test
Direction: lower
```

### 自然语言也行

不需要记忆结构化字段，直接用自然语言描述你想做的事：

```
/aris
我要测试数据预处理管线是否完全 ok，每一项输出都用 vllm 验证，
133 项数据全部测试通过是我们的 goal

/aris
帮我把这个模型的推理延迟压到 50ms 以下，用 wrk 跑 benchmark，
不能破坏现有的单元测试

/aris
我的 ETL 脚本现在有 17 个 edge case 会挂，逐个修掉，
cargo test 全绿就算完成
```

Agent 会自动从你的描述中推断出目标指标、验证命令、scope 和完成条件。

### 不知道用什么指标？

```
/aris:plan
Goal: Make the API faster
```

plan 向导会分析你的代码库，建议指标，并在启动前试运行验证命令。

---

## CLI

ARIS 不需要任何二进制文件也能工作，skill 协议可以通过普通 shell 和 git 命令完成核心循环。CLI 增加的是长时间运行时更需要的能力：安装、验证、结构化日志、最佳结果查询、报告、导出、并行探索和实时仪表盘。

```bash
cargo install aris-cli
```

### CLI 带来什么

| 无 CLI（bash 后备） | 有 CLI |
|---------------------|--------|
| TSV 文件记录实验 | JSONL 结构化存储 |
| 手动追踪指标 | 奖励作弊检测 |
| `tail` / `sort` 查历史 | `aris log`、`aris best`、`aris diff` |
| 无验证 | `aris doctor`（14+ 项预检） |
| 无可视化 | `aris watch`（实时 TUI 仪表盘） |

### 主要命令

| 命令 | 用途 |
|------|------|
| `aris init` | 初始化项目配置 |
| `aris doctor` | 启动前验证 |
| `aris record --metric X --status Y` | 记录实验 |
| `aris log` | 查看历史 |
| `aris best` | 最佳结果 + diff |
| `aris watch` | 实时 TUI 仪表盘 |
| `aris fork` / `aris merge-best` | 并行探索 |
| `aris report` | 生成摘要 |
| `aris export --format csv` | 导出供分析 |

所有命令支持 `--json` 标志和 `AUTORESEARCH_FORMAT=json` 环境变量。

---

## 适配不同领域

| 领域 | 指标 | 验证命令 | Guard |
|------|------|----------|-------|
| ML 训练 | val_loss（越低越好） | `python train.py \| grep val_loss` | — |
| 测试覆盖率 | coverage %（越高越好） | `pytest --cov \| grep TOTAL` | `pytest -x` |
| Web 性能 | p99 延迟（越低越好） | `npm run bench \| grep p99` | `npm test` |
| 构建速度 | 构建时间（越低越好） | `time cargo build` | `cargo test` |
| 代码质量 | lint 分数（越高越好） | `pylint src/ \| grep rated` | `pytest -x` |
| 包体积 | 字节数（越低越好） | `esbuild --bundle --minify \| wc -c` | `npm test` |

---

## 项目结构

```
aris-cli/
├── skills/aris/                    ← Skill 协议（核心）
│   ├── SKILL.md                    ← 主 skill 索引 + 路由
│   └── references/                 ← 各阶段协议
│       ├── autonomous-loop-protocol.md
│       ├── core-principles.md
│       ├── plan-workflow.md
│       ├── debug-workflow.md
│       ├── fix-workflow.md
│       └── results-logging.md
├── commands/aris/                   ← 斜杠命令分发器
│   ├── plan.md
│   ├── debug.md
│   └── fix.md
├── src/                            ← 可选的 Rust CLI
└── tests/
```

---

## 许可证

MIT — 见 [LICENSE](LICENSE)。

---

## 致谢

- [Andrej Karpathy](https://github.com/karpathy) — [autoresearch](https://github.com/karpathy/autoresearch) 概念的提出者
- [uditgoenka/autoresearch](https://github.com/uditgoenka/autoresearch) — 启发了协议层设计的 Claude Code skill 框架
