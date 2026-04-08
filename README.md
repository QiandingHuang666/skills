# Agent Skills for Graduate Students

为高校研究生日常学习与科研计算定制的 Agent Skills 集合，面向 Codex CLI、Claude Code、OpenCLAW 等终端型 agent。

## 简介

当前仓库重点维护 `slurm-assistant`。它已经从单体 Python CLI 演进为 `server + client + skill` 架构：

- `slurm-server`：常驻本机，负责保存运行时信息、持久化连接配置、转发 SSH/Slurm 操作
- `slurm-client`：给 agent 和终端直接调用的统一命令行接口
- `SKILL.md`：给模型的最小决策树、工作流约束和输出规范

该方案面向 Windows/macOS/Linux，默认采用“client 只访问同机 server”的简单模型；远程集群访问通过 server 调用系统 `ssh` / `scp` 完成。

需要区分两层含义：

- `slurm-assistant` 自身运行时已经不依赖 Python
- 文档里出现的 `python`、`uv run python`、`conda`，通常只是用户在集群上运行自己的科研脚本

## Slurm Assistant

Slurm HPC 集群助手，适合高校公共集群、个人服务器、实例和本地集群节点混合使用。

当前 Rust 版本已覆盖：

- server 状态检查
- 连接管理：`add`、`list`、`get`、`remove`
- 资源查询：`status --gpu`、`find-gpu`、`partition-info`、`node-info`、`node-jobs`
- 作业流程：`jobs`、`submit`、`log`、`cancel`、`alloc`、`release`、`run`
- 文件传输：`upload`、`download`
- 兜底执行：`exec`

## 安装二进制

安装器现在会按平台拉取对应 release 压缩包，压缩包内包含：

- `slurm-client`
- `slurm-server`
- 安装脚本
- `slurm-assistant` skill 运行时文件

安装时会同时完成两类动作：

- 将二进制安装到本机命令目录
- 将 `slurm-assistant` 安装到 `~/.codex/skills/`、`~/.claude/skills/`、`~/.openclaw/skills/`

Linux/macOS：

```bash
curl -fsSL https://raw.githubusercontent.com/QiandingHuang666/skills/master/install-slurm-assistant.sh | bash
```

Windows PowerShell：

```powershell
powershell -ExecutionPolicy Bypass -NoProfile -Command "irm https://raw.githubusercontent.com/QiandingHuang666/skills/master/install-slurm-assistant.ps1 | iex"
```

默认二进制安装位置：

- `~/.local/bin/slurm-client`
- `~/.local/bin/slurm-server`
- Windows：`%USERPROFILE%\AppData\Local\Programs\slurm-assistant\bin\slurm-client.exe`
- Windows：`%USERPROFILE%\AppData\Local\Programs\slurm-assistant\bin\slurm-server.exe`

Linux/macOS 如需自定义二进制目录：

```bash
curl -fsSL https://raw.githubusercontent.com/QiandingHuang666/skills/master/install-slurm-assistant.sh | SLURM_ASSISTANT_INSTALL_DIR=/your/bin bash
```

Windows 如需自定义二进制目录：

```powershell
$env:SLURM_ASSISTANT_INSTALL_DIR="D:\tools\slurm-assistant\bin"
powershell -ExecutionPolicy Bypass -NoProfile -Command "irm https://raw.githubusercontent.com/QiandingHuang666/skills/master/install-slurm-assistant.ps1 | iex"
```

Linux/macOS 卸载：

```bash
curl -fsSL https://raw.githubusercontent.com/QiandingHuang666/skills/master/install-slurm-assistant.sh | bash -s -- uninstall
```

Windows 卸载：

```powershell
powershell -ExecutionPolicy Bypass -NoProfile -Command "& ([scriptblock]::Create((irm https://raw.githubusercontent.com/QiandingHuang666/skills/master/install-slurm-assistant.ps1))) -Action uninstall"
```

## 安装 Skill

将 skill 复制到对应 Agent 的全局 skills 目录：

```bash
# Codex CLI
cp -r slurm-assistant ~/.codex/skills/

# Claude Code
cp -r slurm-assistant ~/.claude/skills/

# OpenCLAW
cp -r slurm-assistant ~/.openclaw/skills/
```

## 本地开发

### 构建 Rust 二进制

```bash
cd slurm-assistant/rust
cargo build
```

### 启动本机 server

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-server -- serve
```

### 调用 client

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-client -- server status --json
```

### 添加贵州大学集群连接

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-client -- connection add \
  --label gzu-cluster \
  --host 210.40.56.85 \
  --port 21563 \
  --user qiandingh \
  --kind cluster \
  --json
```

## 目录结构

```text
.
├── README.md
└── slurm-assistant/
    ├── SKILL.md
    ├── references/
    ├── rust/
    │   ├── Cargo.toml
    │   ├── crates/
    │   │   ├── slurm-client/
    │   │   ├── slurm-proto/
    │   │   └── slurm-server/
    │   └── scripts/
    │       └── live_smoke_gzu.sh
    ├── evals/
    └── scripts/
```

## 测试

Rust 单元测试：

```bash
cd slurm-assistant/rust
cargo test
```

贵州大学实机 smoke：

```bash
cd slurm-assistant/rust
bash scripts/live_smoke_gzu.sh
```

## 技术栈

- Rust
- SQLite
- Slurm
- SSH / SCP

## 兼容性

- Agent CLI：Codex CLI、Claude Code、OpenCLAW
- 操作系统：Windows、macOS、Linux
- 远程执行：依赖系统 `ssh` / `scp`

## 许可证

MIT License
