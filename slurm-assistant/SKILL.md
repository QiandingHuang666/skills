---
name: slurm-assistant
description: |
  Slurm HPC 集群助手，为高校学生/教师定制。支持本地（集群上）和远程（集群外）两种使用模式。

  TRIGGER 当用户：
  - 提到 slurm、sbatch、squeue、scancel、salloc、srun、sinfo 等 Slurm 命令
  - 提到 hpc 集群、slurm 集群、超算、计算节点、作业调度系统
  - 想要查看分区/节点状态、队列情况、GPU 可用性
  - 需要提交/取消/查看作业（使用 squeue/sbatch/scancel 等术语）
  - 需要申请交互式资源（salloc）或运行命令（srun）
  - 需要生成或修改 slurm 作业脚本
  - 需要上传/下载文件到 HPC 集群
  - 询问如何使用 HPC 集群、如何提交作业等新手问题
  - 提到"连接实例"、"切换实例"、"实例端口"、"帮我连接到实例"
---

# Slurm 集群助手

跨平台 Slurm HPC 集群管理工具，支持 Windows/macOS/Linux。

---

## 最小决策树（主流程）

先按下面的最小流程执行；只有在当前步骤缺信息时，才读取对应的 `references/*.md`。

### Step 0：确定脚本路径

优先使用：

```bash
SCRIPT="/absolute/path/to/slurm-assistant/scripts/slurm-cli.py"
```

如果路径未知，再使用：

```bash
uv run python "$SCRIPT" path --json
```

### Step 1：先做配置检查

每次会话开始先执行：

```bash
uv run python "$SCRIPT" init --check --output-json --fast
```

只根据这几个字段决策：

- `configured`
- `config_valid`
- `local_slurm_available`
- `connection_count`
- `connections`
- `current_agent_authorized`

### Step 2：按检查结果分流

#### A. `configured = false`

不要直接猜配置。先判断用户是在“本地模式”还是“远程模式”：

- 用户明确说“我已经在集群/登录节点上”  
  → 走本地模式，读取 `references/workflow_local_execution.md`
- 否则  
  → 走首次远程配置，读取 `references/workflow_init.md`

#### B. `configured = true` 且 `config_valid = false`

停止后续操作，先修配置问题：

- 远程连接问题 → `references/workflow_init.md`
- 本地 Slurm 不可用 → `references/workflow_local_execution.md`

#### C. `configured = true` 且 `config_valid = true`

进入正常执行流程：

- `connection_count <= 1`：直接用当前活动连接
- `connection_count > 1`：先执行

  ```bash
  uv run python "$SCRIPT" connection --list
  ```

  再按用户意图选连接：
  - 说“集群” → 选 `type=cluster`
  - 说“实例” → 选 `type=instance`
  - 提到端口 → 匹配端口对应连接
  - 不明确 → 先澄清，不要猜

  后续命令统一优先用 `-C <连接名>` 临时切换，不要默认永久改活动连接。

### Step 3：把用户请求归到 6 类动作

只需先分类，再执行，不要一上来读所有参考文档。

1. **资源查看**
   - 关键词：状态、GPU、分区、节点、排队
   - 优先命令：
     ```bash
     uv run python "$SCRIPT" status --gpu
     uv run python "$SCRIPT" find-gpu
     uv run python "$SCRIPT" partition-info
     uv run python "$SCRIPT" node-info <节点名>
     ```
   - 需要细节时再读：`references/workflow_status.md`

2. **作业管理**
   - 关键词：submit、jobs、log、cancel、alloc、srun
   - 优先命令：
     ```bash
     uv run python "$SCRIPT" submit <脚本>
     uv run python "$SCRIPT" jobs
     uv run python "$SCRIPT" log <job_id>
     uv run python "$SCRIPT" cancel <job_id>
     uv run python "$SCRIPT" alloc -p <分区> [-g gpu:1]
     ```
   - 生成/提交流程再读：`references/workflow_job.md`

3. **文件传输**
   - 关键词：上传、下载、拷文件、日志拉回本地
   - 优先命令：
     ```bash
     uv run python "$SCRIPT" upload <本地> <远程>
     uv run python "$SCRIPT" download <远程> <本地>
     ```
   - 需要细节时再读：`references/workflow_file_transfer.md`

4. **环境配置**
   - 关键词：conda、uv、CUDA、PyTorch、环境安装
   - 先判断是否涉及重操作；如涉及安装/编译/大下载，不能在登录节点直接做
   - 再读：`references/workflow_env_config.md`

5. **实例连接 / 多连接**
   - 关键词：连接实例、切换实例、实例端口
   - 先执行：
     ```bash
     uv run python "$SCRIPT" connection --list
     ```
   - 再读：本文件“实例连接流程”或 `references/workflow_init.md`

6. **任意远程命令**
   - 只有当现有子命令覆盖不了用户需求时，才使用：
     ```bash
     uv run python "$SCRIPT" exec -c '<命令>'
     ```
   - 执行前必须做安全分类，见下文“安全分流”。

### Step 4：安全分流

先判断命令属于哪一类：

- **A 类：只读/轻量**  
  如 `squeue`、`sinfo`、`ls`、`cat`、`grep`、`head`、`tail`  
  → 可直接执行

- **B 类：会改用户目录，但通常可逆**  
  如新建目录、写脚本、上传下载、提交普通作业  
  → 可以执行，但要先简要说明会改什么

- **C 类：高成本或应避开登录节点**  
  如大规模下载、编译、大包安装、长时间数据处理  
  → 不要在登录节点直接做；先引导 `alloc`/`sbatch`

- **D 类：危险/破坏性**  
  如 `rm -rf`、`dd`、`chmod 000`、`shutdown`、批量 kill  
  → 必须先明确确认；不确认不执行

### Step 5：输出要求

- 优先给“结论 + 下一步”，不要先贴长日志
- 命令输出只保留关键行
- 如果用了某个 reference，在回答中只吸收其必要步骤，不要整段复述

### 何时读取哪份参考文档

- 首次配置 / 修配置：`references/workflow_init.md`
- 本地模式：`references/workflow_local_execution.md`
- 资源状态：`references/workflow_status.md`
- 作业脚本 / 提交：`references/workflow_job.md`
- 文件传输：`references/workflow_file_transfer.md`
- 环境配置：`references/workflow_env_config.md`
- 贵州大学特例：`references/gzu_public_resources.md`

---

## ⛔ 不可违背原则

**禁止在登录节点进行任何费资源操作！**

登录节点是共享资源，仅供提交作业、编辑文件等轻量操作。以下操作**严禁**在登录节点执行：

| 禁止操作 | 正确做法 |
|---------|---------|
| `find /` 等大规模文件搜索 | 先 `salloc` 申请计算节点，在计算节点执行 |
| 下载大数据集（如 `git clone` 大仓库、`wget` 大文件） | 先申请计算节点，在计算节点下载 |
| 编译安装软件（`make`、`pip install` 大包） | 先申请计算节点，在计算节点编译 |
| 运行计算任务（训练模型、数据处理） | 使用 `sbatch` 提交作业或 `salloc` 申请资源 |
| 启动占用大量内存的程序 | 先申请计算节点资源 |

**正确流程：**
1. 先申请资源：`salloc -p <分区> -g gpu:1`
2. 获得计算节点后，再执行费资源操作
3. 操作完成后释放资源：`exit` 或 `scancel <job_id>`

**如果用户请求在登录节点执行上述操作，必须拒绝并引导用户先申请资源！**

---

## 必经流程：配置检查（每次会话开始时执行）

**重要：在执行任何命令前，必须先进行配置检查！**

### 检查命令

```bash
uv run python "$SCRIPT" init --check --output-json [--fast]
```

**参数说明：**
- `--fast`: 快速模式，跳过 SSH 连接测试（加速初始化）

### 检查结果说明

```json
{
  "configured": true/false,          // 是否已配置
  "local_slurm_available": true/false,  // 本地 Slurm 是否可用
  "ssh_key_configured": true/false,  // SSH 密钥是否已配置
  "ssh_connection_ok": true/false,   // SSH 连接是否成功
  "config_valid": true/false,        // 配置是否有效
  "auto_exec_authorized": true/false // 是否已授权自动执行（新增）
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

#### 场景 B：已配置（单个连接）

直接使用该连接执行后续命令。

#### 场景 C：已配置（多个连接）

当存在多个连接时，执行：

```bash
uv run python "$SCRIPT" connection --list
```

根据输出结果，AI 应该：
1. 列出所有连接及其状态
2. 标记当前活动连接（[ACTIVE]）
3. 在后续命令中使用 `-C` 参数指定连接

**示例：**
```
连接列表：
  别名                  类型        地址                    状态
  gzu-cluster          cluster    qiandingh@210.40.56.85:21563  [ACTIVE]
  gzu-instance-21810   instance   hqd@210.40.56.85:21810

当用户说"集群"时使用 `gzu-cluster`，说"实例"时使用 `gzu-instance-21810`。
```

---

## 多连接管理

Slurm Assistant 支持多个"连接"（集群和实例），每个连接包含独立的配置信息。

### 连接结构

每个连接包含以下信息：
- `name`: 显示名称
- `host`: IP 地址
- `port`: SSH 端口
- `username`: 用户名
- `jump_host`: 跳板机（可选）
- `type`: 类型（cluster 或 instance）
- `parent`: 父连接名（仅实例需要）
- `passwordless`: 是否已配置免密登录

### 连接别名

系统自动为连接生成简短的别名，格式：
- 集群：`{关键词}-cluster`（如 `gzu-cluster`）
- 实例：`{关键词}-instance-{端口}`（如 `gzu-instance-21810`）

### 连接管理命令

```bash
# 列出所有连接
uv run python "$SCRIPT" connection --list

# 切换活动连接（永久保存）
uv run python "$SCRIPT" connection --switch <别名>

# 临时切换连接（单次命令有效，不修改配置）
uv run python "$SCRIPT" -C <别名> <其他命令>

# 添加新连接
uv run python "$SCRIPT" connection --add <别名> --host <IP> --port <端口> --username <用户名> --type <cluster|instance>

# 删除连接
uv run python "$SCRIPT" connection --remove <别名>

# 查看连接详情
uv run python "$SCRIPT" connection --info <别名>
```

### 多连接场景处理

**当存在多个连接时，AI 应该：**

1. 在配置检查后，列出所有可用连接：
   ```bash
   uv run python "$SCRIPT" connection --list
   ```

2. 根据用户意图判断目标连接：
   - 用户说"集群" → 查找 type=cluster 的连接
   - 用户说"实例" → 查找 type=instance 的连接
   - 用户提到端口号（如"21810"）→ 查找对应端口的连接

3. 使用 `-C` 参数临时切换：
   ```bash
   uv run python "$SCRIPT" -C gzu-cluster find-gpu
   uv run python "$SCRIPT" -C gzu-instance-21810 exec -c "hostname"
   ```

---

## 实例连接流程

当用户请求"连接实例"、"切换到实例"时执行此流程。

**背景**：部分集群提供"实例"功能，IP 与集群一致，端口不同。

### 0. 检查已有连接

首先列出所有连接：

```bash
uv run python "$SCRIPT" connection --list
```

**根据结果分支处理：**

#### 0a. 已有实例连接

如果存在 `type=instance` 的连接：
- **只有一个实例** → 直接切换：
  ```bash
  uv run python "$SCRIPT" connection --switch <实例别名>
  ```
- **多个实例** → 询问用户要连接哪个：
  ```json
  {
    "questions": [
      {
        "question": "检测到多个实例连接，请选择要连接的实例：",
        "options": ["<实例1别名>", "<实例2别名>", "添加新实例"]
      }
    ]
  }
  ```

#### 0b. 没有实例连接

进入"添加实例连接"流程（以下步骤 1-5）。

### 1. 询问端口和用户名

使用 AskUserQuestion 收集实例端口和用户名：

```json
{
  "questions": [
    {
      "question": "请问实例的 SSH 端口是？",
      "header": "实例端口"
    },
    {
      "question": "请问您的集群用户名是？",
      "header": "用户名"
    }
  ]
}
```

### 2. 验证免密登录

在保存配置前，必须先验证免密登录是否已配置：

```bash
uv run python "$SCRIPT" ssh-test --host "<集群 host>" --port <用户输入的端口> --username <用户输入的用户名>
```

**根据验证结果处理：**

#### 2a. 免密登录已配置（验证成功）

继续执行步骤 3 保存配置。

#### 2b. 免密登录未配置（验证失败）

告知用户需要先配置免密登录，并引导配置：

```
检测到 SSH 免密登录未配置。请按以下步骤配置：

1. 在本地生成 SSH 密钥（如果已有可跳过）：
   ssh-keygen -t ed25519

2. 将公钥复制到集群：
   ssh-copy-id -p <端口> <用户名>@<集群 host>

3. 配置完成后，请告诉我"已配置"，我将继续连接流程。
```

**等待用户确认配置完成后再继续。**

### 3. 保存配置

获取端口和验证免密登录后，使用 init 命令保存配置：

```bash
uv run python "$SCRIPT" init --mode remote \
  --cluster-name "<原集群名> 实例" \
  --host "<原集群 host>" \
  --port <用户输入的端口> \
  --username "<用户输入的用户名>"
```

**说明**：
- `--host`：与原集群相同的 IP 地址
- `--port`：用户提供的实例端口
- `--username`：用户提供的用户名

### 4. 验证连接

```bash
uv run python "$SCRIPT" ssh-test
```

### 5. 确认成功

告知用户：
```
已连接到实例（端口 <端口>）
```

---

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

当用户已在集群节点上（本地模式）时，参见 `references/workflow_local_execution.md` 了解执行规范和流程要求。

---
## 命令速查

| 类别 | 命令 | 说明 | 详细 |
|------|------|------|------|
| 状态 | status | 查看资源状态（`--gpu` 显示 GPU） | `references/commands.md` |
| 状态 | partition-info | 分区详情 | `references/commands.md` |
| 状态 | find-gpu | 查找 GPU 资源 | `references/commands.md` |
| 作业 | alloc | 申请交互式资源 | `references/commands.md` |
| 作业 | submit | 提交作业 | `references/commands.md` |
| 作业 | jobs | 查看作业状态 | `references/commands.md` |
| 作业 | cancel | 取消作业 | `references/commands.md` |
| 文件 | upload | 上传文件/目录 | `references/commands.md` |
| 文件 | download | 下载文件/目录 | `references/commands.md` |
| **核心** | **exec** | **执行远程命令（统一入口）** | `references/commands.md` |

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
| 首次使用 | 配置检查、场景选择 | `references/workflow_init.md` |
| 环境配置 | conda + uv 配置开发环境 | `references/workflow_env_config.md` |
| 提交作业/生成作业脚本 | 收集信息、选择环境、生成脚本 | `references/workflow_job.md` |
| GPU 查询 | 查找可用 GPU 资源 | `references/commands.md` |
| 文件上传/下载 | 上传/下载文件到集群 | `references/workflow_file_transfer.md` |
| 用户作业与资源状况 | 查看作业状态、队列情况、节点状态 | `references/workflow_status.md` |

### 贵州大学 HPC 特有功能

**配置信息（AI 必须使用以下值，不得猜测）：**
- Host: `210.40.56.85`
- Port: `21563`

当用户使用贵州大学 HPC 集群时，具有以下"特权"功能：

| 功能 | 说明 | 详细 |
|------|------|------|
| 公共资源检查 | 下载前检查 `/home/share/Official/` 公共目录 | `references/gzu_public_resources.md` |
| LaTeX 快速安装 | 使用集群提供的 TeX Live 安装脚本 | `references/gzu_public_resources.md` |

**重要：** 贵州大学用户在下载数据集或安装软件前，AI 必须先引导用户检查公共资源，避免重复下载。

**LaTeX 安装：**
- 用户需要 LaTeX 但未安装时，引导执行：`sh /home/share/Official/tools/texlive/install.sh`

**A10 分区使用提示：**
- A10 分区有严格的空闲自动释放机制
- 申请资源后如果没有立即运行命令，资源会被自动释放

**推荐做法：使用 `tmux + sleep` 保持连接**

```bash
# 方法1：在 tmux 中申请（推荐）
tmux new -s gzu-a10
salloc -p gpu-a10 --gres=gpu:1
# 在 salloc 交互式 shell 中执行命令

# 方法2：申请时直接运行命令
salloc -p gpu-a10 --gres=gpu:1 srun python train.py
```

**原理**：`tmux` 保持会话活跃，`sleep` 娡拟用户活动，避免被判定为空闲而释放。
- 避免用户手动下载安装 TexLive（非常耗时）

**路径映射（重要）：**

贵州大学 HPC 提供三种访问方式，路径映射不同：

| 环境 | 个人目录 | 项目目录 |
|------|----------|----------|
| 容器实例 | `/home/<username>` | `/groups/<project>/home/<username>` |
| 虚拟机实例 | `/webdav/MyData` | `/webdav/ProjectGroup(<project>)` |
| 公共集群 | `/users/<username>` | `/groups/<project>/home/<username>` |

**AI 注意事项：**
- 当用户提到"实例"时，需确认是容器实例（SSH）还是虚拟机实例（WebDAV）
- 容器实例路径以 `/home/` 开头，虚拟机实例路径以 `/webdav/` 开头
- 详细映射见 `references/use_gzu.md`

---

## 输出要求

- 不使用 emoji
- 状态用文字（如 `[RUNNING]`、`[PENDING]`）
- 表格简单对齐

**GPU 节点信息输出格式：**
```
节点                 分区          GPU 空闲/总数    CPU 空闲/总数    GPU型号
------------------------------------------------------------------------------------------
gpu-node01          gpu          2/4             8/32            A100
gpu-node02          gpu          0/4             32/32           A100
gpu-node03          gpu          1/2             16/24           V100
```

输出 GPU 节点状态时，必须明确说明：
- 该节点有几张 GPU 显卡是空闲的
- 该节点空闲几个 CPU

---

## 权限配置

**重要：此 skill 为全局安装，授权配置也是全局的（写入 `~/.claude/settings.json`）**

使用 `exec` 命令可以显著减少授权询问次数。

### 授权状态管理

技能会直接管理全局 settings.json 中的权限规则：

```bash
# 授权自动执行（写入全局 settings.json）
uv run python "$SCRIPT" init --authorize

# 取消授权
uv run python "$SCRIPT" init --unauthorize

# 查看授权状态（在 --check 输出中）
uv run python "$SCRIPT" init --check --output-json
```

### 授权机制

**授权时自动添加的规则（写入 `~/.claude/settings.json`）：**
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

**检测授权状态的方式：**
- 检查 `~/.claude/settings.json` 是否存在
- 检查 `permissions.allow` 数组中是否包含 slurm-cli.py 规则

### 每次会话必查

**AI 必须在每次使用技能时检查授权状态！**

配置检查输出中的 `auto_exec_authorized` 字段表示授权状态：
- `true`: 全局 settings.json 中已配置授权规则
- `false`: 未配置授权，需要询问用户

### 授权询问流程

当 `auto_exec_authorized: false` 时，必须询问用户：

```json
{
  "questions": [
    {
      "question": "为减少授权询问次数，是否允许 slurm-cli.py 自动执行命令？",
      "description": "授权将写入 ~/.claude/settings.json 的全局权限规则",
      "options": [
        "是，授权自动执行（推荐）",
        "否，每次执行前确认"
      ]
    }
  ]
}
```

**用户选择"是"后，执行授权命令：**
```bash
uv run python "$SCRIPT" init --authorize
```

### Claude Code 权限配置（可选）

如果用户想要在 Claude Code 层面完全跳过授权询问，可以手动添加到 `~/.claude/settings.json`：

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
| `references/commands.md` | 完整命令参考、用法、示例 |
| `references/workflow_init.md` | 首次使用流程（配置检查、场景选择） |
| `references/workflow_env_config.md` | 环境配置流程（conda + uv） |
| `references/workflow_job.md` | 生成作业脚本流程 |
| `references/workflow_file_transfer.md` | 文件上传/下载流程 |
| `references/workflow_status.md` | 用户作业与资源状况查询 |
| `references/workflow_local_execution.md` | 集群本地执行模式规范 |
| `references/job_templates.md` | 作业脚本模板 |
| `references/common_errors.md` | 常见错误 |
| `references/set_free_password.md` | 免密登录配置 |
| `references/use_gzu.md` | 贵州大学 HPC 配置 |
| `references/gzu_public_resources.md` | 贵州大学公共资源检查 |
| `references/use_other.md` | 其他集群配置 |
| `references/use_local.md` | 本地模式使用 |
