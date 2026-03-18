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
  - 询问如何使用集群、如何提交作业等新手问题

  即使是简单问题也请使用此 skill，因为它包含集群特定的配置和环境检测。
compatibility:
  - bash
  - uv/uvx (优先) 或 python3
---

# Slurm 集群助手

为高校学生/教师定制的 Slurm HPC 集群管理工具。支持本地（集群上）和远程（集群外）两种使用模式。

---

## 核心工作流程

**所有操作通过 `slurm-cli.py` 统一执行，避免多次授权询问**：

```
用户请求 → slurm-cli.py init --check → [已配置?] → 执行 slurm-cli.py <command>
                                          ↓ 否
                                    AskUserQuestion 收集信息 → slurm-cli.py init --mode ... → 执行操作
```

### ⚠️ 减少授权询问

由于每次执行的命令参数不同，可能会频繁触发授权询问。

**首次使用时，询问用户是否允许脚本执行**：

```json
{
  "questions": [
    {
      "question": "为减少授权询问，是否允许 slurm-cli.py 脚本自动执行？",
      "options": [
        "是，添加到允许列表（推荐）",
        "否，每次手动确认"
      ]
    }
  ]
}
```

**如果用户选择"是"**：
1. 在项目根目录创建或编辑 `.claude/settings.json`
2. 添加以下内容：
   ```json
   {
     "permissions": {
       "allow": [
         "Bash(python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py*)"
       ]
     }
   }
   ```
3. 告知用户：已添加权限配置，后续操作将自动执行

### ⚠️ 关键原则：统一使用 slurm-cli.py

**禁止执行单独的 shell 命令**来检测环境或检查配置，所有操作必须通过 `slurm-cli.py` 执行：

```bash
# 正确：检查配置状态（一次授权）
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py init --check --output-json

# 错误：不要执行这些单独的命令
# cat ~/.claude/skills/slurm-assistant/config.json
# which sinfo
# sinfo --version
```

### ⚠️ 输出格式限制

**输出内容时避免使用 emoji 和复杂表格**，防止显示错位：

- 使用纯文本格式输出状态信息
- 表格使用简单的文本对齐，不使用 `│` `├` 等特殊字符
- 状态用文字描述而非 emoji（如用 `[RUNNING]` 而非 `🔵`）
- 示例：
  ```
  作业ID     名称         状态        提交时间
  12345     train_job   RUNNING     2024-01-15 10:30
  12346     test_job    PENDING     2024-01-15 11:00
  ```

### 配置检查流程

每次执行任何 slurm 相关操作前：

1. **检查配置状态（一次命令）**
   ```bash
   python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py init --check --output-json
   ```

2. **解析 JSON 输出**：
   - `configured`: true → 已配置，继续执行
   - `configured`: false → 需要初始化配置
   - `local_slurm_available`: true → 可使用本地模式
   - `local_slurm_available`: false → 需要远程模式

3. **如果未配置**：
   - 使用 `AskUserQuestion` 收集信息（见下方）
   - 调用 `slurm-cli.py init --mode ... --host ... --port ... --username ...` 保存配置
   - 脚本会自动测试连接

4. **配置完成后**：执行用户请求的操作

### ⚠️ 重要：询问方式

**必须使用 `AskUserQuestion` 工具**来收集用户信息，不要直接用文本询问。

原因：
- AskUserQuestion 提供结构化的选项和输入
- 减少来回对话，提高效率
- 确保收集的信息格式规范

---

## 环境检测与初始化

### 步骤 1: 检查配置状态（统一命令）

```bash
# 一次命令获取所有状态信息
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py init --check --output-json
```

输出示例：
```json
{
  "configured": false,
  "mode": null,
  "cluster": {},
  "local_slurm_available": false
}
```

### 步骤 2: 收集配置信息（使用 AskUserQuestion）

**必须使用 `AskUserQuestion` 工具来收集信息**，不要直接用文本询问。

#### 简化的提问流程

**第一步：询问使用场景（一次 AskUserQuestion）**

```json
{
  "questions": [
    {
      "question": "请选择您的使用场景",
      "options": [
        "贵州大学 HPC 集群",
        "其他 Slurm 集群",
        "当前已在集群上（本地模式）"
      ]
    }
  ]
}
```

**根据用户选择处理**：

1. **选择"贵州大学 HPC 集群"**：
   - 自动使用预设配置：host=210.40.56.85, port=21563
   - 只需再询问用户名（可选）

   ```json
   {
     "questions": [
       {
         "question": "集群用户名（当前系统用户名: $USER）",
         "options": [
           "使用默认用户名"
         ]
       }
     ]
   }
   ```

   **说明**：
   - 选择"使用默认用户名"→ 使用当前系统用户名
   - 直接在 "Other" 输入框中输入 → 使用输入的用户名

2. **选择"其他 Slurm 集群"**：
   - 需要收集：登录地址、端口、用户名

   ```json
   {
     "questions": [
       {
         "question": "请输入集群登录地址（如 login.hpc.edu.cn）",
         "options": ["在下方输入"]
       },
       {
         "question": "SSH 端口",
         "options": ["22（默认）", "自定义端口"]
       },
       {
         "question": "集群用户名",
         "options": ["使用本地用户名", "自定义用户名"]
       }
     ]
   }
   ```

3. **选择"当前已在集群上"**：
   - 直接配置为本地模式，无需额外询问

### 步骤 3: 保存配置（通过 slurm-cli.py）

收集完信息后，**通过 slurm-cli.py 一次性保存配置**：

#### 本地模式（在集群上）

```bash
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py init --mode local --cluster-name "<集群名称>"
```

#### 远程模式（集群外）

```bash
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py init \
  --mode remote \
  --cluster-name "<集群名称>" \
  --host "<登录地址>" \
  --port <端口> \
  --username "<用户名>" \
  [--jump-host "<跳板机地址>"]
```

**贵州大学预设配置**：
```bash
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py init \
  --mode remote \
  --cluster-name "贵州大学" \
  --host "210.40.56.85" \
  --port 21563 \
  --username "<用户名>"
```

脚本会自动：
- 创建配置目录
- 保存配置到 `config.json`
- 测试 SSH 连接（远程模式）
- 报告连接状态

如果连接失败，引导用户配置 SSH 密钥（见下方 SSH 配置引导）。

---

## SSH 密钥配置引导

当用户需要配置免密登录时，可以使用 `setup-ssh` 命令进行交互式配置：

```bash
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py setup-ssh
```

该命令会自动：
1. 检查现有 SSH 密钥
2. 引导生成新密钥（如需要）
3. 配置 SSH config
4. 复制公钥到集群
5. 测试免密登录

### 手动配置（备选）

如果需要手动配置，可以参考以下步骤：

#### 1. 检查现有密钥

```bash
ls -la ~/.ssh/id_*
```

#### 2. 生成密钥（如不存在）

```bash
ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519
```

#### 3. 复制公钥到集群

```bash
ssh-copy-id -p <port> [-J <jump_host>] <username>@<host>
```

#### 4. 配置 SSH config（可选但推荐）

在 `~/.ssh/config` 中添加：
```
Host <集群名>
    HostName <host>
    User <username>
    Port <port>
    IdentityFile ~/.ssh/id_ed25519
    ProxyJump <跳板机>  # 如需要
```

#### 5. 测试免密登录

```bash
ssh <集群名> "echo 免密登录成功"
```

---

## 命令执行接口

**所有 Slurm 操作通过 `slurm-cli.py` 统一接口执行**，避免多次授权询问：

```bash
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py <command> [options]
```

### ⚠️ 减少授权询问原则

1. **配置初始化**：收集完所有信息后，一次性写入配置文件（一次 bash 授权）
2. **日常操作**：所有操作通过 `slurm-cli.py` 脚本执行，脚本内部处理 SSH 连接
3. **避免**：不要手动执行多个 `ssh`、`sinfo`、`squeue` 等命令，而是通过脚本统一执行

### 可用命令

| 命令 | 功能 | 触发条件 |
|------|------|----------|
| `init` | 初始化配置 | 首次使用或重新配置 |
| `setup-ssh` | SSH 密钥配置向导 | 需要配置免密登录 |
| `status [-p 分区] [-n]` | 查看资源状态 | 用户想看集群状态 |
| `node-info <节点>` | 查看节点详情 | 用户想看特定节点 |
| `alloc [-p 分区] [-g gres]` | 申请交互式资源 | 用户需要交互式环境 |
| `release <id>` | 释放资源 **(需确认)** | 用户想释放资源 |
| `run <cmd>` | srun 运行命令 | 用户想直接运行命令 |
| `script-gen` | 生成作业脚本 | 用户想创建脚本 |
| `submit <脚本>` | 提交作业 | 用户想提交作业 |
| `jobs [--id <id>]` | 查看作业状态 | 用户想查看作业 |
| `log <job_id> [-f]` | 查看作业日志 | 用户想看日志 |
| `cancel <ids...>` | 取消作业 **(需确认)** | 用户想取消作业 |
| `history` | 作业历史 | 查看通过此 skill 提交的作业 |
| `refresh-cache` | 刷新分区缓存 | 更新集群分区和节点信息缓存 |
| `show-cache` | 显示缓存信息 | 查看缓存的分区和节点详情 |
| `find-gpu <型号> [--数量]` | 查找 GPU 资源 | 搜索指定型号的可用 GPU 节点 |

---

## 分区缓存功能

针对常用集群（如贵州大学），系统会自动缓存集群硬件配置信息，避免反复查询。

### 缓存内容

**仅存储硬件配置（静态信息）**：
- 所有可用分区列表
- 每个分区的节点列表
- 节点配置信息：CPU 核心数、GPU 型号和数量、内存大小

**不存储动态状态**：
- 节点状态（空闲/混合/已分配）在查询时动态获取

### 缓存命令

#### 刷新缓存

```bash
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py refresh-cache
```

首次使用或需要更新集群硬件配置时执行。缓存长期有效，仅在硬件变更时需要更新。

#### 显示缓存

```bash
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py show-cache
```

查看当前缓存的集群硬件配置信息：
- 分区名称
- 各节点的硬件配置（CPU、GPU、内存）

#### 查找 GPU

```bash
# 查找所有 A100 GPU
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py find-gpu a100

# 查找 4 张 V100 GPU 的节点
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py find-gpu v100 --数量 4
```

快速查找满足特定 GPU 需求的节点，返回可用节点列表及其配置。

### 缓存文件

缓存存储在 `~/.claude/skills/slurm-assistant/partition_cache.json`

---

## 功能执行流程

### 1. 查看资源状态

**用户说**：「查看集群状态」「有哪些分区」「GPU 分区空闲吗」

**执行流程**：
```bash
# 检查配置 → [执行命令]
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py status [-p 分区]
```

**输出解释**：向用户解释分区的 A/I/O/T 含义（Allocated/Idle/Other/Total）

### 2. 申请交互式资源

**用户说**：「申请一个 GPU」「我要交互式资源」

**执行流程**：
1. 询问资源需求：
   - 分区（如 gpu）
   - GRES（如 gpu:1, gpu:a100:2）
   - CPU 核心数
   - 是否需要保活（默认 24h）

2. 生成并执行 salloc 命令：
   ```bash
   salloc -p <分区> --gres=<gres> --cpus-per-task=<cpus> tmux new-session -d 'sleep 24h'
   ```

3. 告知用户：
   - 分配的作业 ID
   - 如何连接到分配的节点
   - 如何释放资源

### 3. 生成作业脚本

**用户说**：「帮我写个作业脚本」「生成训练脚本」

**执行流程**：
1. 询问/确认信息：
   - 作业名称
   - 脚本保存路径
   - 分区、GRES、CPU 等
   - **不主动询问 mem 和 time**（除非用户提及）
   - 要运行的命令

2. 生成脚本，遵循原则：
   - 日志输出到 `logs/` 目录
   - Python 优先使用 uv
   - 包含 `mkdir -p logs`
   - 不指定 mem/time（除非用户要求）

3. 询问保存位置并保存

4. 询问是否立即提交

### 4. 提交作业

**用户说**：「提交作业」「sbatch 这个脚本」

**执行流程**：
```bash
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py submit <脚本路径>
```

提交后：
- 显示作业 ID
- 记录到作业历史（`jobs.json`）
- 告知日志路径

### 5. 查看作业状态

**用户说**：「我的作业」「作业状态」

**执行流程**：
```bash
python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py jobs [--id <job_id>]
```

同时显示：
- squeue 输出（当前队列）
- 此 skill 记录的作业历史

### 6. 查看作业日志

**用户说**：「看日志」「作业输出」

**执行流程**：
1. 从作业历史中查找日志路径
2. 如果找不到，使用默认路径 `logs/<job_id>.out`
3. 显示日志内容：
   ```bash
   python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py log <job_id> [-f]
   ```

### 7. 取消作业（危险操作）

**用户说**：「取消作业」「kill 那个任务」

**执行流程**：
1. **确认对话框**（必须）：
   ```
   ⚠️  危险操作：即将取消作业 12345
       作业名称：training_job
       状态：RUNNING
       提交时间：2024-01-15T10:30:00

   确认执行？[y/N]
   ```

2. 用户确认后执行：
   ```bash
   scancel <job_id>
   ```

3. 更新作业历史状态

---

## 危险操作确认原则

以下操作**必须**向用户确认：

- `cancel` - 取消作业
- `release` - 释放资源
- 任何包含 `rm` 的命令
- 任何包含 `kill` 的命令
- `scancel` 相关操作
- 批量操作（`--all`）

**确认格式**：
```
⚠️  危险操作：<操作描述>
    <详细信息>

确认执行？[y/N]
```

---

## Python 环境策略

**优先级**：`uv/uvx` > `conda` > `module load`

### 在作业脚本中使用

```bash
# 方式 1: uv run（项目环境，推荐）
uv run python train.py

# 方式 2: uvx（单文件脚本）
uvx --with numpy --with pandas python script.py

# 方式 3: conda（如果用户已有环境）
source ~/.bashrc && conda activate my_env

# 方式 4: module（集群提供）
module load python/3.9
```

---

## 作业记录管理

作业记录存储在 `~/.claude/skills/slurm-assistant/jobs.json`：

```json
{
  "jobs": [
    {
      "job_id": "12345",
      "name": "training_job",
      "script": "/home/user/project/train.sh",
      "submitted_at": "2024-01-15T10:30:00",
      "status": "RUNNING",
      "output_file": "logs/train_12345.out",
      "error_file": "logs/train_12345.err"
    }
  ]
}
```

每次提交作业时自动记录，用于后续日志查看和状态追踪。

---

## 项目级别安装

### 安装方式

Skill 支持两种安装方式：

#### 1. 全局安装（默认）

将 skill 文件复制到 `~/.claude/skills/slurm-assistant/`，所有项目共享同一配置。

```bash
cp -r slurm-assistant ~/.claude/skills/
```

#### 2. 项目级别安装

将 skill 文件复制到项目的 `.claude/skills/` 目录下，配置随项目分发。

```bash
cp -r slurm-assistant .claude/skills/
```

### 配置优先级

当同时存在项目配置和全局配置时，合并策略如下：
- 项目配置覆盖全局配置的同名设置
- 全局配置作为项目配置的默认值补充

配置文件位置：
- 项目配置：`<project>/.claude/skills/slurm-assistant/config.json`
- 全局配置：`~/.claude/skills/slurm-assistant/config.json`

### 使用场景

| 场景 | 推荐方式 |
|------|----------|
| 个人使用单一集群 | 全局安装 |
| 团队协作，共享项目配置 | 项目级别安装 |
| 多集群环境，不同项目用不同集群 | 项目级别安装 |
| 项目需要特定集群配置（如默认分区） | 项目级别安装 |

### 保存配置到项目级别

在初始化时使用 `--save-to-project` 参数：

```bash
python3 .claude/skills/slurm-assistant/scripts/slurm-cli.py init --mode remote --host ... --save-to-project
```

---

## 参考资源

按需读取以下参考文件：
- `references/job_templates.md` - 作业脚本模板（GPU、多节点、数组作业等）
- `references/common_errors.md` - 常见错误及解决方案

当用户需要特定模板或遇到错误时，读取对应参考文件。
