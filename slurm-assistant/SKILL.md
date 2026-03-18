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

---

## 首次使用流程

### 1. 检查配置状态

```bash
uv run python "$SCRIPT" init --check --output-json
```

输出：
```json
{"configured": false, "local_slurm_available": false}
```

### 2. 如果未配置，收集配置信息

**第一步：询问使用场景**
```json
{
  "questions": [
    {
      "question": "请选择您的使用场景",
      "options": [
        "贵州大学 HPC 集群",
        "其他 Slurm 集群（远程）",
        "当前已在集群上（本地模式）"
      ]
    }
  ]
}
```

**第二步：根据选择继续收集**

**A. 贵州大学 HPC 集群**
```json
{
  "questions": [
    {
      "question": "请输入您的贵州大学 HPC 集群用户名",
      "options": ["输入用户名"]
    },
    {
      "question": "是否已配置免密登录？",
      "options": ["已配置", "未配置，需要帮助"]
    }
  ]
}
```
如未配置免密登录，参考 `references/set_free_password.md`

**B. 其他 Slurm 集群（远程）**
```json
{
  "questions": [
    {
      "question": "请输入集群名称（如：xx大学超算）",
      "options": ["输入集群名称"]
    },
    {
      "question": "请输入集群登录节点地址",
      "options": ["输入地址（如 login.hpc.edu）"]
    },
    {
      "question": "请输入 SSH 端口",
      "options": ["22（默认）", "其他端口"]
    },
    {
      "question": "请输入您的用户名",
      "options": ["输入用户名"]
    },
    {
      "question": "是否需要通过跳板机连接？",
      "options": ["不需要", "需要"]
    },
    {
      "question": "是否已配置免密登录？",
      "options": ["已配置", "未配置，需要帮助"]
    }
  ]
}
```
如未配置免密登录，参考 `references/set_free_password.md`

**C. 当前已在集群上（本地模式）**
```json
{
  "questions": [
    {
      "question": "请为这个集群命名（用于标识）",
      "options": ["使用默认名称（local）", "输入自定义名称"]
    }
  ]
}
```

### 3. 保存配置

```bash
# 贵州大学 HPC
uv run python "$SCRIPT" init --mode remote \
  --cluster-name "贵州大学 HPC" \
  --host 210.40.56.85 \
  --port 21563 \
  --username "用户输入的用户名"

# 其他集群
uv run python "$SCRIPT" init --mode remote \
  --cluster-name "用户输入的名称" \
  --host "用户输入的地址" \
  --port 用户输入的端口 \
  --username "用户输入的用户名" \
  --jump-host "跳板机地址（如有）"

# 本地模式
uv run python "$SCRIPT" init --mode local --cluster-name "用户输入的名称"
```

---

## 命令速查

### 集群状态（重点关注 GPU）

| 命令 | 用法 | 说明 |
|------|------|------|
| status | `uv run python "$SCRIPT" status [--gpu] [-p 分区]` | 查看资源状态，`--gpu` 显示 GPU 详情 |
| partition-info | `uv run python "$SCRIPT" partition-info [-p 分区]` | 分区详情（一次调用获取所有节点） |
| node-info | `uv run python "$SCRIPT" node-info <节点>` | 查看节点详情 |
| node-jobs | `uv run python "$SCRIPT" node-jobs <节点>` | 查看节点上的作业（运行中/排队中） |
| find-gpu | `uv run python "$SCRIPT" find-gpu [型号]` | 查找 GPU 资源（不指定型号显示所有） |

### 作业管理

| 命令 | 用法 | 说明 |
|------|------|------|
| alloc | `uv run python "$SCRIPT" alloc -p <分区> [-g gres] [-c cpus] [--max-wait 时间]` | 申请交互式资源，CPU 自动计算 |
| release | `uv run python "$SCRIPT" release <id>` | 释放资源 |
| run | `uv run python "$SCRIPT" run <命令>` | srun 运行命令 |
| submit | `uv run python "$SCRIPT" submit <脚本>` | 提交作业 |
| jobs | `uv run python "$SCRIPT" jobs [--id <id>]` | 查看作业状态 |
| log | `uv run python "$SCRIPT" log <job_id> [-f]` | 查看作业日志 |
| cancel | `uv run python "$SCRIPT" cancel <ids...>` | 取消作业 |
| history | `uv run python "$SCRIPT" history` | 作业历史 |

### 文件传输

| 命令 | 用法 | 说明 |
|------|------|------|
| upload | `uv run python "$SCRIPT" upload <本地> <远程> [-r]` | 上传文件/目录（自动检查本地文件） |
| download | `uv run python "$SCRIPT" download <远程> <本地> [-r]` | 下载文件/目录（自动检查远程文件） |

**注意：**
- `upload` 会检查本地文件是否存在，显示文件大小/类型
- `download` 会先检查远程文件是否存在，不存在则报错

### 远程命令（核心功能）

| 命令 | 用法 | 说明 |
|------|------|------|
| exec | `uv run python "$SCRIPT" exec -c <命令>` | 在集群上执行命令（统一入口，**必须掌握**） |

**重要提示：**
- `exec` 是核心命令，用于减少授权询问次数
- 所有需要直接在集群上执行的命令都应通过 `exec` 进行
- **AI 模型必须在调用 `exec` 命令前进行安全评估**
- 危险命令包括但不限于：
  - 删除操作：`rm -rf`、`rmdir` 等
  - 破坏性操作：`dd`、`mkfs`、格式化等
  - 系统影响：`kill -9 -1`、`shutdown`、`reboot` 等
  - 权限修改：`chmod 000`、`chown` 等
- **对于危险命令，AI 必须使用 `AskUserQuestion` 工具请求用户确认**，告知可能的后果
- 安全命令示例：`ls`、`cat`、`grep`、`squeue`、`sinfo`、`head`、`tail` 等

---

## GPU 查询示例

```bash
# 查看所有 GPU 节点状态（推荐）
uv run python "$SCRIPT" status --gpu

# 查看特定分区
uv run python "$SCRIPT" status --gpu -p gpu

# 查找特定型号
uv run python "$SCRIPT" find-gpu a100

# 查看分区所有节点（一次调用）
uv run python "$SCRIPT" partition-info -p gpu

# 查看节点上的作业
uv run python "$SCRIPT" node-jobs gpu-node01
```

---

## 资源申请示例

```bash
# 申请 GPU 节点（CPU 自动计算）
uv run python "$SCRIPT" alloc -p gpu -g gpu:1

# 申请 GPU 节点（指定 CPU 数量）
uv run python "$SCRIPT" alloc -p gpu -g gpu:1 -c 8

# 申请 GPU 节点（设置最大等待时间 5 分钟）
uv run python "$SCRIPT" alloc -p gpu -g gpu:1 --max-wait 5
```

---

## 远程命令示例（exec - 核心命令）

```bash
# 查看文件列表（安全命令，直接执行）
uv run python "$SCRIPT" exec -c ls -lh

# 查看文件内容
uv run python "$SCRIPT" exec -c cat slurm-12345.out

# 搜索作业日志（使用单引号包围复杂命令）
uv run python "$SCRIPT" exec -c 'grep ERROR slurm-*.out'

# 查看磁盘使用
uv run python "$SCRIPT" exec -c 'du -sh ~/workspace'
```

**危险命令处理流程（AI 必须遵循）：**

当用户请求执行危险命令时，AI 必须使用 `AskUserQuestion` 工具请求确认：

```json
{
  "questions": [
    {
      "question": "即将执行危险命令：rm -rf /tmp/test，这将永久删除该目录及其内容。是否继续？",
      "options": [
        "继续执行",
        "取消操作"
      ]
    }
  ]
}
```

---

### 贵州大学 HPC 特有功能：公共资源检查

**重要：当用户请求下载数据集或安装软件时，AI 必须先检查 `/home/share/Official/` 公共目录！**

许多常用的数据集、模型、软件工具可能已经在公共目录中存在。重复下载会：
- 浪费存储空间和带宽
- 增加等待时间
- 可能违反集群使用规定

**AI 处理流程：**

1. **识别场景**：当用户提到以下关键词时触发检查：
   - "下载 dataset"、"下载数据集"、"download dataset"
   - "安装软件"、"install"、"下载模型"
   - 具体的数据集名称（如 ImageNet、COCO、LLaMA 等）

2. **执行检查**：
```bash
uv run python "$SCRIPT" exec -c 'ls -lh /home/share/Official/'
```

3. **搜索相关资源**（如果有具体名称）：
```bash
uv run python "$SCRIPT" exec -c 'find /home/share/Official/ -iname "*关键字*" 2>/dev/null | head -20'
```

4. **处理结果**：
   - **找到资源**：告知用户可以直接使用，提供软链接命令
   - **未找到**：继续执行用户的下载/安装请求

**示例对话：**

用户："帮我下载 ImageNet 数据集"

AI 应该：
1. 先检查：`uv run python "$SCRIPT" exec -c 'find /home/share/Official/ -iname "*imagenet*" 2>/dev/null'`
2. 如果找到：告知用户 "公共目录已有 ImageNet，无需下载，可以使用软链接直接使用"
3. 如果未找到：才执行下载操作

---

---

## 减少授权询问（重要）

使用 `exec` 命令可以显著减少授权询问次数。建议将以下权限规则添加到允许列表：

**推荐配置（包含 exec 命令）：**
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

首次使用时询问：
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

**选择"是"时，添加到 `~/.claude/settings.json`:**
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

## 输出要求
- 不使用 emoji
- 状态用文字（如 `[RUNNING]`）
- 表格简单对齐

**GPU 节点信息（重要）：**
输出 GPU 节点状态时，必须明确说明：
- 该节点有几张 GPU 显卡是空闲的
- 该节点空闲几个 CPU

示例输出：
```
节点                 分区          GPU 空闲/总数    CPU 空闲/总数    GPU型号
------------------------------------------------------------------------------------------
gpu-node01          gpu          2/4              8/32             A100
gpu-node02          gpu          0/4              32/32            A100
gpu-node03          gpu          1/2              16/24            V100
```

---

## 生成作业脚本（重要流程）

当用户请求生成作业脚本时，AI 必须按照以下流程进行：

### 第一步：收集基本信息

使用 `AskUserQuestion` 工具收集以下信息：

```json
{
  "questions": [
    {
      "question": "请选择作业分区",
      "options": ["cpu", "gpu-a100", "gpu-v100", "其他（请说明）"]
    },
    {
      "question": "需要 GPU 资源吗？",
      "options": ["不需要", "需要 (1卡)", "需要 (多卡，请说明数量)"]
    },
    {
      "question": "预计运行时间？",
      "options": ["1小时", "4小时", "24小时", "其他（请说明）"]
    }
  ]
}
```

### 第二步：询问虚拟环境配置（必须）

**这是关键步骤！** 必须询问用户需要使用哪种虚拟环境：

```json
{
  "questions": [
    {
      "question": "请选择 Python 环境管理方式",
      "options": [
        "uv（推荐，快速现代）",
        "conda（传统方式）",
        "conda + uv（conda 管理 CUDA，uv 管理 Python 包）",
        "不需要（使用系统 Python）"
      ]
    }
  ]
}
```

### 第三步：根据回答生成激活语句

根据用户选择，在作业脚本中添加相应的激活语句：

| 用户选择 | 激活语句 |
|---------|---------|
| **uv（推荐）** | `# 使用 uv run，无需激活<br>uv run python your_script.py` |
| **conda** | `source ~/.bashrc<br>conda activate your_env_name` |
| **conda + uv** | `# 先激活 conda（获取 CUDA）<br>source ~/.bashrc<br>conda activate base<br># 使用 uv 运行（获取 Python 包）<br>uv run python your_script.py` |
| **不需要** | `# 使用系统 Python<br>module load python/3.9<br>python your_script.py` |

### 第四步：生成完整脚本

结合收集的信息，生成完整的作业脚本。

**示例：用户选择 uv + GPU**

```bash
#!/bin/bash
#SBATCH --job-name=training
#SBATCH --partition=gpu-a100
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=8
#SBATCH --time=4:00:00
#SBATCH --output=logs/%j.out
#SBATCH --error=logs/%j.err

cd $SLURM_SUBMIT_DIR

# 创建日志目录
mkdir -p logs

# 显示作业信息
echo "Job ID: $SLURM_JOB_ID"
echo "Node: $(hostname)"
echo "GPUs: $CUDA_VISIBLE_DEVICES"

# 显示 GPU 信息
nvidia-smi

# 运行训练（uv 方式，无需激活虚拟环境）
uv run python train.py --config config.yaml --epochs 100

echo "Job completed at: $(date)"
```

**示例：用户选择 conda + uv**

```bash
#!/bin/bash
#SBATCH --job-name=training
#SBATCH --partition=gpu-a100
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=8
#SBATCH --time=4:00:00
#SBATCH --output=logs/%j.out
#SBATCH --error=logs/%j.err

cd $SLURM_SUBMIT_DIR

# 创建日志目录
mkdir -p logs

# 显示作业信息
echo "Job ID: $SLURM_JOB_ID"

# 激活 conda 环境（获取 CUDA 支持）
source ~/.bashrc
conda activate cuda_env

# 使用 uv 运行（管理 Python 包）
uv run python train.py --config config.yaml

echo "Job completed at: $(date)"
```

### 第五步：提交作业

生成脚本后，询问用户是否立即提交：

```json
{
  "questions": [
    {
      "question": "脚本已生成，是否立即提交作业？",
      "options": ["立即提交", "先查看脚本内容", "稍后手动提交"]
    }
  ]
}
```

如果用户选择立即提交，执行：
```bash
uv run python "$SCRIPT" submit script.sh
```

---

## Python 环境策略（作业脚本）
```bash
# 优先级：uv > conda > module
uv run python train.py
```

---

## 参考资源
- `references/job_templates.md` - 作业脚本模板
- `references/common_errors.md` - 常见错误
- `references/set_free_password.md` - 免密登录配置
