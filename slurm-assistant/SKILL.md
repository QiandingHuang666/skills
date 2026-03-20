---
name: slurm-assistant
description: |
  Slurm HPC 集群助手，为高校学生/教师定制。支持本地（集群上）和远程（集群外）两种使用模式。

  TRIGGER 当用户：
  - 提到 slurm、hpc、集群、超算、计算节点、作业调度
  - 想要查看分区/节点状态、队列情况
  - 需要提交/取消/查看作业（sbatch/scancel/squeue）
  - 需要申请交互式资源（salloc）或运行命令（srun）
  - 需要生成或修改 slurm 作业脚本
  - 需要上传/下载文件到集群
  - 询问如何使用集群、如何提交作业等新手问题
compatibility:
  - python3
  - uv (可选，优先使用)
  - ssh (远程模式需要)
  - scp (文件传输需要)
---

# Slurm 集群助手

跨平台 Slurm HPC 集群管理工具，支持 Windows/macOS/Linux。

---

## 必经流程：配置检查（每次会话开始时执行）

**重要：在执行任何命令前，必须先进行配置检查！**

### 检查命令

```bash
uv run python "$SCRIPT" init --check --output-json
```

### 检查结果说明

```json
{
  "configured": true/false,          // 是否已配置
  "local_slurm_available": true/false,  // 本地 Slurm 是否可用
  "ssh_key_configured": true/false,  // SSH 密钥是否已配置
  "ssh_connection_ok": true/false,   // SSH 连接是否成功
  "config_valid": true/false         // 配置是否有效
}
```

### 处理流程

#### 场景 A：未配置 (`configured: false`)

触发"首次连接"流程，使用 `AskUserQuestion` 询问用户：

```json
{
  "questions": [
    {
      "question": "检测到您是首次使用，请选择使用模式：",
      "options": [
        "远程模式（从本机连接集群）",
        "本地模式（已在集群节点上）"
      ]
    }
  ]
}
```

根据选择跳转到：
- 远程模式 → `workflow_init.md`
- 本地模式 → `workflow_local_execution.md`

#### 场景 B：已配置但无效 (`configured: true, config_valid: false`)

向用户说明问题并询问是否重新配置：

```json
{
  "questions": [
    {
      "question": "配置存在问题：{config_error}。是否重新配置？",
      "options": ["重新配置", "稍后处理"]
    }
  ]
}
```

#### 场景 C：已配置且有效 (`configured: true, config_valid: true`)

配置正确，可以直接使用。向用户简要确认：

```
配置已加载，可以开始使用。
```

---

## 脚本路径（固定）

```bash
SCRIPT=~/.claude/skills/slurm-assistant/scripts/slurm-cli.py
```

## Python 运行命令

**优先使用 uv（推荐）：**
```bash
uv run python "$SCRIPT" <command>
```

**无 uv 时使用：**
```bash
python3 "$SCRIPT" <command>
# 或 Windows:
python "$SCRIPT" <command>
```

### 集群本地执行模式

当用户已在集群节点上（本地模式）时，参见 `workflow_local_execution.md` 了解执行规范和流程要求。

---
## 命令速查

| 类别 | 命令 | 说明 | 详细 |
|------|------|------|------|
| 状态 | status | 查看资源状态（`--gpu` 显示 GPU） | `commands.md` |
| 状态 | partition-info | 分区详情 | `commands.md` |
| 状态 | find-gpu | 查找 GPU 资源 | `commands.md` |
| 作业 | alloc | 申请交互式资源 | `commands.md` |
| 作业 | submit | 提交作业 | `commands.md` |
| 作业 | jobs | 查看作业状态 | `commands.md` |
| 作业 | cancel | 取消作业 | `commands.md` |
| 文件 | upload | 上传文件/目录 | `commands.md` |
| 文件 | download | 下载文件/目录 | `commands.md` |
| **核心** | **exec** | **执行远程命令（统一入口）** | `commands.md` |

---

## exec 命令说明（核心）

`exec` 是核心命令，用于减少授权询问次数。所有需要直接在集群上执行的命令都应通过 `exec` 进行。

```bash
uv run python "$SCRIPT" exec -c <命令>
```

**安全要求：**
- AI 必须在调用 `exec` 命令前进行安全评估
- 危险命令（`rm -rf`、`dd`、`shutdown` 等）必须使用 `AskUserQuestion` 请求用户确认
- 安全命令（`ls`、`cat`、`grep`、`squeue` 等）可直接执行

**更多命令详情：** `references/commands.md`

---

## 常用流程

| 流程 | 说明 | 详细 |
|------|------|------|
| 首次使用 | 配置检查、场景选择 | `workflow_init.md` |
| 环境配置 | conda + uv 配置开发环境 | `workflow_env_config.md` |
| 提交作业/生成作业脚本 | 收集信息、选择环境、生成脚本 | `workflow_job.md` |
| GPU 查询 | 查找可用 GPU 资源 | `commands.md` |
| 文件上传/下载 | 上传/下载文件到集群 | `workflow_file_transfer.md` |
| 用户作业与资源状况 | 查看作业状态、队列情况、节点状态 | `workflow_status.md` |

### 贵州大学 HPC 特有功能

当用户使用贵州大学 HPC 集群时，具有以下"特权"功能：

| 功能 | 说明 | 详细 |
|------|------|------|
| 公共资源检查 | 下载前检查 `/home/share/Official/` 公共目录 | `gzu_public_resources.md` |

**重要：** 贵州大学用户在下载数据集或安装软件前，AI 必须先引导用户检查公共资源，避免重复下载。

---

## 输出要求

- 不使用 emoji
- 状态用文字（如 `[RUNNING]`、`[PENDING]`）
- 表格简单对齐

**GPU 节点信息输出格式：**
```
节点                 分区          GPU 空闲/总数    CPU 空闲/总数    GPU型号
------------------------------------------------------------------------------------------
gpu-node01          gpu          2/4              8/32             A100
gpu-node02          gpu          0/4              32/32            A100
gpu-node03          gpu          1/2              16/24            V100
```

输出 GPU 节点状态时，必须明确说明：
- 该节点有几张 GPU 显卡是空闲的
- 该节点空闲几个 CPU

---

## 权限配置

使用 `exec` 命令可以显著减少授权询问次数。

### 首次使用询问

首次使用时，应询问用户是否配置权限：

```json
{
  "questions": [
    {
      "question": "为减少授权询问，是否允许 slurm-cli.py 自动执行？",
      "options": ["是，添加到允许列表", "否，每次确认"]
    }
  ]
}
```

### 推荐配置

选择"是"时，将以下规则添加到 `~/.claude/settings.json`：

```json
{
  "permissions": {
    "allow": [
      "Bash(uv run python ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py*)",
      "Bash(python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py*)",
      "Bash(python ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py*)"
    ]
  }
}
```

---

## 安装

```bash
cp -r slurm-assistant ~/.claude/skills/
```

---

## 参考资源

| 文件 | 说明 |
|------|------|
| `commands.md` | 完整命令参考、用法、示例 |
| `workflow_init.md` | 首次使用流程（配置检查、场景选择） |
| `workflow_env_config.md` | 环境配置流程（conda + uv） |
| `workflow_job.md` | 生成作业脚本流程 |
| `workflow_file_transfer.md` | 文件上传/下载流程 |
| `workflow_status.md` | 用户作业与资源状况查询 |
| `workflow_local_execution.md` | 集群本地执行模式规范 |
| `job_templates.md` | 作业脚本模板 |
| `common_errors.md` | 常见错误 |
| `set_free_password.md` | 免密登录配置 |
| `use_gzu.md` | 贵州大学 HPC 配置 |
| `gzu_public_resources.md` | 贵州大学公共资源检查 |
| `use_other.md` | 其他集群配置 |
| `use_local.md` | 本地模式使用 |
