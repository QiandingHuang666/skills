# 集群本地执行模式规范

当用户已在集群节点上（本地模式）时的命令执行规范和流程要求。

---

## 触发条件

当满足以下条件时，使用本地执行模式：

1. 用户明确表示"我在集群上"、"已在登录节点"
2. 检测到 Slurm 命令可用：`which sinfo` 返回有效路径
3. `init --check` 显示 `local_slurm_available: true`

---

## 本地模式与远程模式的区别

| 特性 | 远程模式 | 本地模式 |
|------|---------|---------|
| 命令执行方式 | 通过 `exec` 或脚本 SSH 执行 | 直接使用 slurm-cli.py 本地命令 |
| 文件路径 | 需要区分本地/远程 | 所有路径都是集群路径 |
| exec 命令 | 需要使用（减少授权询问） | 不需要，直接执行原生命令 |
| 配置需求 | 需要配置 SSH 连接 | 只需本地 Slurm 可用 |

---

## 命令执行规范

### 1. 优先使用 slurm-cli.py

即使在本地模式，也应优先使用 slurm-cli.py 脚本执行命令：

```bash
# 推荐：使用脚本
uv run python "$SCRIPT" status --gpu
uv run python "$SCRIPT" jobs
uv run python "$SCRIPT" submit job.sh

# 替代：直接使用原生命令（仅在脚本不可用时）
sinfo --format="%.10P %.5a %.10l %.6D %.6t %N"
squeue -u $USER
sbatch job.sh
```

### 2. 文件操作

本地模式下的文件操作：

| 操作 | 本地模式命令 | 说明 |
|------|-------------|------|
| 查看文件 | `cat file.txt` | 直接使用 shell 命令 |
| 列出目录 | `ls -lh` | 直接使用 shell 命令 |
| 上传文件 | `cp source ~/dest` | 使用 cp 而非 upload |
| 下载文件 | `cp ~/source ./dest` | 使用 cp 而非 download |

**注意：** 本地模式不需要使用 `upload/download` 命令，直接使用 `cp`。

### 3. 作业脚本生成

生成的作业脚本适用于直接提交：

```bash
# 生成脚本后，直接提交
sbatch job.sh

# 或使用脚本
uv run python "$SCRIPT" submit job.sh
```

---

## 输出要求

本地模式输出要求与远程模式相同：

- 不使用 emoji
- 状态用文字（如 `[RUNNING]`、`[PENDING]`）
- 表格简单对齐
- GPU 节点信息必须明确说明空闲数量

---

## 流程要求

### 用户查询资源

```bash
# 1. 使用脚本查询（推荐）
uv run python "$SCRIPT" status --gpu

# 2. 或使用原生命令
sinfo -o "%P %a %l %D %t %N" -p gpu

# 3. 按格式报告
节点 gpu-node01: 2/4 张 A100 空闲，8/32 CPU 空闲
```

### 用户提交作业

```bash
# 1. 收集信息（与远程模式相同）
# 2. 生成作业脚本（与远程模式相同）
# 3. 提交作业
sbatch job.sh
# 或
uv run python "$SCRIPT" submit job.sh
```

### 用户查看作业

```bash
# 1. 查看作业状态
uv run python "$SCRIPT" jobs

# 2. 查看日志
cat slurm-12345.out
# 或
uv run python "$SCRIPT" log 12345
```

---

## 安全要求

本地模式下仍需遵守安全规范：

### 危险命令确认

执行危险命令前必须使用 `AskUserQuestion` 确认：

```json
{
  "questions": [
    {
      "question": "即将删除目录 ~/workspace/old，是否继续？",
      "options": ["继续执行", "取消操作"]
    }
  ]
}
```

危险命令包括：
- 删除操作：`rm -rf`、`rmdir`
- 破坏性操作：`dd`、格式化
- 系统影响：`kill -9`、`shutdown`、`reboot`
- 权限修改：`chmod 000`、`chown`

### 安全命令

以下命令可直接执行：
- 查询类：`ls`、`cat`、`grep`、`head`、`tail`
- Slurm 类：`squeue`、`sinfo`、`sacct`
- 状态类：`df`、`du`、`ps`

---

## 配置检测

### 检测是否在本地模式

```bash
# 方法 1：检查 Slurm 是否可用
which sinfo

# 方法 2：使用脚本检查
uv run python "$SCRIPT" init --check --output-json
# 输出: {"configured": false, "local_slurm_available": true}
```

### 本地模式初始化

如果检测到本地 Slurm 可用，引导用户进行本地配置：

```json
{
  "questions": [
    {
      "question": "检测到您在集群节点上，是否配置本地模式？",
      "options": ["配置本地模式", "继续使用远程模式"]
    }
  ]
}
```

选择本地模式后执行：

```bash
uv run python "$SCRIPT" init --mode local --cluster-name "local"
```

---

## 常见场景

### 场景 1：用户在集群上查看 GPU

```bash
# AI 检测到本地模式
# 执行查询
uv run python "$SCRIPT" status --gpu

# 报告结果
GPU 资源概览：
节点 gpu-node01: 2/4 张 A100 空闲，8/32 CPU 空闲
```

### 场景 2：用户在集群上提交作业

```bash
# 1. 收集信息（与远程模式相同流程）
# 2. 生成脚本 job.sh
# 3. 提交
sbatch job.sh

# 输出
Submitted batch job 12345
```

### 场景 3：用户在集群上复制文件

```bash
# 直接使用 shell 命令
cp ~/source.txt ~/dest.txt

# 验证
ls -lh ~/dest.txt
```

---

## 相关文档

- `references/use_local.md` - 本地模式配置说明
- `references/commands.md` - 命令详细用法
- `references/workflow_job.md` - 作业脚本生成流程
