# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

这是一个 Claude Code Skills 集合，为高校研究生日常科研计算和 HPC 集群使用定制。主要包含 Slurm Assistant skill，提供跨平台（Windows/macOS/Linux）的 Slurm 集群管理功能。

## 目录结构

```
.
├── slurm-assistant/
│   ├── SKILL.md                 # Skill 主文档（使用入口）
│   ├── scripts/
│   │   └── slurm-cli.py         # 核心 CLI 工具
│   └── references/
│       ├── commands.md          # 命令参考
│       ├── workflow_*.md        # 各类工作流程文档
│       └── job_templates.md     # 作业脚本模板
└── README.md
```

## 核心架构

`slurm-cli.py` 采用面向对象设计：

- **ConfigManager**: 配置管理器，负责加载/保存 `~/.claude/skills/slurm-assistant/config.json`
- **SlurmExecutor**: Slurm 执行器，根据模式（本地/远程）执行 Slurm 命令
  - 本地模式：直接执行 `squeue`、`sbatch` 等命令
  - 远程模式：通过 SSH 在集群上执行命令
- **Color/输出工具**: 终端颜色和消息格式化

## Python 运行方式

**优先使用 uv（推荐）**:
```bash
uv run python ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py <command>
```

**无 uv 时**:
```bash
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py <command>
```

## 核心概念

### 两种使用模式

1. **本地模式**：用户已在集群节点上，直接执行 Slurm 命令
2. **远程模式**：用户从本地机器通过 SSH 连接集群

### 配置检查流程

每次会话开始时必须执行配置检查：
```bash
uv run python "$SCRIPT" init --check --output-json
```

根据返回状态决定后续流程（未配置→配置向导，已配置→直接使用）

### exec 命令（核心）

`exec` 是减少授权询问的统一入口：
```bash
uv run python "$SCRIPT" exec -c <命令>
```

AI 必须在调用前进行安全评估，危险命令需用户确认。

## 主要命令

| 类别 | 命令 | 用途 |
|------|------|------|
| 状态 | status | 查看集群资源（`--gpu` 显示 GPU） |
| 状态 | find-gpu | 查找 GPU 资源 |
| 作业 | submit | 提交作业 |
| 作业 | jobs | 查看作业状态 |
| 作业 | cancel | 取消作业 |
| 作业 | alloc | 申请交互式资源 |
| 文件 | upload/download | 文件传输 |
| 核心 | exec | 执行远程命令（统一入口） |

## 开发注意事项

- Skill 使用 `SKILL.md` 作为主入口，包含 trigger 条件和完整工作流程
- 参考文档按功能组织在 `references/` 目录
- 输出格式要求：不使用 emoji，状态用文字标签（如 `[RUNNING]`）
- 贵州大学 HPC 有特有功能（公共资源检查），见 `gzu_public_resources.md`

## 安装测试

安装 skill 到全局目录：
```bash
cp -r slurm-assistant ~/.claude/skills/
```

验证安装：
```bash
uv run python ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py --help
```
