<div align="center">

![ARIS banner](assets/banner.png)

# ARIS

**一套让 AI 编程 Agent 自主迭代的 Skill**

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

把 `skills/aris/` 文件夹复制到你的项目里，输入 `/aris`，Agent 就进入自主实验循环 —— 修改代码、验证指标、保留改进、回滚失败 —— 无需安装任何二进制文件。

灵感来自 [Karpathy 的 autoresearch](https://github.com/karpathy/autoresearch)。

[30 秒上手](#30-秒上手) · [用自然语言布置任务](#用自然语言布置任务) · [Skill 命令](#skill-命令) · [为什么用 ARIS](#为什么用-aris) · [工作原理](#工作原理) · [CLI（可选）](#cli可选)

[English](README_en.md)

</div>

---

## 这是什么？

ARIS 是一套**可移植的 Skill 协议** —— 几个 Markdown 文件，教会 AI 编程 Agent 用纪律化流程优化任何可量化目标。


| 你需要做的 | Agent 会做的 |
|-----------|-------------|
| 复制 `skills/aris/` 到项目 | 自动读取协议 |
| 输入 `/aris` + 目标描述 | 进入 8 阶段自主循环 |
| 喝咖啡 ☕ | 修改 → 验证 → 保留/回滚 → 重复 |
| 审查结果 | 输出结构化实验报告 |

**支持的 Agent：** Claude Code · OpenAI Codex · Cursor · Windsurf · OpenCode · Gemini CLI · GitHub Copilot · 通用 `.agents/skills/` 工作流

---

## 30 秒上手

**方式一：让 Agent 帮你装（最推荐）**

直接向 Claude Code（或其他带命令行权限的 Agent）布置任务：
> "帮我安装 aris-cli（你可以使用 `cargo install aris-cli` 全局安装，或者以适合当前项目的方式安装），安装完成后运行 `aris install claude-code`。"

然后，你只需要在 Agent 中输入 `/aris` 即可开始。

**方式二：手动复制（零安装）**

```bash
# 把 skill 文件复制到你的项目
cp -r skills/aris/ 你的项目/.claude/skills/aris/
cp -r commands/aris/ 你的项目/.claude/commands/aris/
cp commands/aris.md 你的项目/.claude/commands/aris.md
```

然后在 Agent 中输入 `/aris`。完了。

**方式三：手动用 CLI 安装**

```bash
cargo install aris-cli
aris install claude-code   # 或 codex / cursor / windsurf / gemini / copilot / all
```

CLI 会自动把 skill 文件放到正确的位置，还附赠 `aris doctor` 预检、`aris watch` 实时仪表盘等能力。但 **skill 本身不依赖 CLI**。

---

## 用自然语言布置任务

不需要记忆任何结构化字段。像和同事说话一样告诉 Agent 你想做什么：

```
/aris
我现在要测试我的数据预处理管线是否完全 ok，每一项输出你都用 vllm
进行验证，133 项数据全部测试通过就是我们的 goal

Agent：（自动理解目标，拆解验证步骤，逐项迭代）
       基线：98/133 通过 → 最终：133/133 通过（12 次保留，5 次丢弃）
```

```
/aris
帮我把这个模型的推理延迟压到 50ms 以下，用 wrk 跑 benchmark，
不能破坏现有的单元测试

Agent：（自动推断指标为 p99 延迟，方向 lower，guard 为 npm test）
       基线：127ms → 最佳：43ms（18 次保留，12 次丢弃）
```

```
/aris
我的 ETL 脚本现在有 17 个 edge case 会挂，逐个修掉，
cargo test 全绿就算完成

Agent：（拆解为 17 个子目标，逐一迭代修复）
       基线：0/17 通过 → 最终：17/17 通过（17 次保留，9 次丢弃）
```

当然，你也可以用结构化字段精确控制：

```
/aris
Goal: Increase test coverage to 90%
Scope: src/**/*.ts
Verify: npx jest --coverage | grep 'All files' | awk '{print $4}'
Guard: npm test
Direction: higher
Iterations: 30
```

Agent 会自动从你的描述中推断出目标指标、验证命令、范围和完成条件。说人话就行。

---

## Skill 命令

| 命令 | Agent 做什么 |
|------|-------------|
| `/aris` | 运行自主实验循环 |
| `/aris:plan` | 交互式向导：分析代码库 → 建议目标、指标、验证命令 |
| `/aris:debug` | 自主 bug 狩猎循环（科学方法：假设 → 验证 → 排除） |
| `/aris:fix` | 迭代修复错误直到零错误 |

### 不知道用什么指标？

```
/aris:plan
Goal: Make the API faster
```

plan 向导会分析你的代码库，建议指标，并在启动前试运行验证命令。让 Agent 帮你想。

---

## 为什么用 ARIS？

AI 编程 Agent 很会改代码，但如果没有明确流程，长时间优化很容易变成松散试错。ARIS 提供的就是这个流程。

| 问题 | ARIS 的处理方式 |
|------|-----------------|
| Agent 一次改太多，难以定位原因 | 每轮只做一个原子变更 |
| "看起来更好"替代真实指标 | 只接受机械化指标验证 |
| 失败尝试污染工作区 | 通过 git 自动回滚 |
| 实验历史从上下文中丢失 | 结构化日志 + commit 历史 |
| 迭代久了开始钻指标空子 | 奖励作弊检测标记异常跳变 |
| 不同 Agent 需要不同提示词 | 一套可移植 skill 适配多个平台 |

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

## CLI（可选）

Skill 协议本身可以通过普通 shell 和 git 命令完成核心循环，**不需要安装任何东西**。CLI 增加的是长时间运行时更需要的能力：

```bash
cargo install aris-cli
```

### Skill vs Skill + CLI

| 纯 Skill（零安装） | Skill + CLI |
|---------------------|-------------|
| ✅ 直接可用 | ✅ 一键安装到任意 Agent |
| ✅ 完整的 8 阶段循环 | ✅ 完整循环 + 结构化日志 |
| TSV 文件记录实验 | JSONL 结构化存储 |
| 手动追踪指标 | 奖励作弊检测 |
| `tail` / `sort` 查历史 | `aris log`、`aris best`、`aris diff` |
| 无预检 | `aris doctor`（14+ 项预检） |
| 无可视化 | `aris watch`（实时 TUI 仪表盘） |

### 主要命令

| 命令 | 用途 |
|------|------|
| `aris init` | 初始化项目配置 |
| `aris install <agent>` | 一键安装 skill 到指定 Agent |
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

## 项目结构

```
aris-cli/
├── skills/aris/                    ← Skill 协议（核心，复制这个就够了）
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
