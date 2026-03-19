# Slurm CLI 命令参考

完整命令说明、用法和示例。

---

## 集群状态命令

### status - 查看资源状态

```bash
uv run python "$SCRIPT" status [--gpu] [-p 分区]
```

| 参数 | 说明 |
|------|------|
| `--gpu` | 显示 GPU 详情 |
| `-p 分区` | 查看指定分区 |

**示例：**
```bash
# 查看所有资源
uv run python "$SCRIPT" status

# 查看 GPU 节点（推荐）
uv run python "$SCRIPT" status --gpu

# 查看特定分区
uv run python "$SCRIPT" status --gpu -p gpu
```

### partition-info - 分区详情

```bash
uv run python "$SCRIPT" partition-info [-p 分区]
```

一次调用获取分区内所有节点信息。

### node-info - 节点详情

```bash
uv run python "$SCRIPT" node-info <节点名>
```

### node-jobs - 节点作业

```bash
uv run python "$SCRIPT" node-jobs <节点名>
```

查看节点上运行中/排队中的作业。

### find-gpu - 查找 GPU 资源

```bash
uv run python "$SCRIPT" find-gpu [型号]
```

不指定型号显示所有 GPU，指定型号搜索特定 GPU。

---

## 作业管理命令

### alloc - 申请交互式资源

```bash
uv run python "$SCRIPT" alloc -p <分区> [-g gres] [-c cpus] [--max-wait 时间]
```

| 参数 | 说明 |
|------|------|
| `-p 分区` | 目标分区 |
| `-g gres` | GPU 资源（如 `gpu:1`） |
| `-c cpus` | CPU 数量（不指定则自动计算） |
| `--max-wait` | 最大等待时间（分钟） |

**示例：**
```bash
# 申请 GPU 节点（CPU 自动计算）
uv run python "$SCRIPT" alloc -p gpu -g gpu:1

# 申请 GPU 节点（指定 CPU）
uv run python "$SCRIPT" alloc -p gpu -g gpu:1 -c 8

# 设置最大等待时间 5 分钟
uv run python "$SCRIPT" alloc -p gpu -g gpu:1 --max-wait 5
```

### release - 释放资源

```bash
uv run python "$SCRIPT" release <分配ID>
```

### run - 运行命令

```bash
uv run python "$SCRIPT" run <命令>
```

使用 srun 运行命令。

### submit - 提交作业

```bash
uv run python "$SCRIPT" submit <脚本文件>
```

### jobs - 查看作业状态

```bash
uv run python "$SCRIPT" jobs [--id <作业ID>]
```

### log - 查看作业日志

```bash
uv run python "$SCRIPT" log <job_id> [-f]
```

`-f` 参数持续跟踪日志。

### cancel - 取消作业

```bash
uv run python "$SCRIPT" cancel <作业ID...>
```

支持同时取消多个作业。

### history - 作业历史

```bash
uv run python "$SCRIPT" history
```

---

## 文件传输命令

### upload - 上传文件

```bash
uv run python "$SCRIPT" upload <本地路径> <远程路径> [-r]
```

| 参数 | 说明 |
|------|------|
| `-r` | 递归上传目录 |

**注意：**
- 自动检查本地文件是否存在
- 显示文件大小/类型
- 只给出文件名时默认位于 `~` 目录

### download - 下载文件

```bash
uv run python "$SCRIPT" download <远程路径> <本地路径> [-r]
```

先检查远程文件是否存在，不存在则报错。

---

## 远程命令（exec - 核心命令）

### exec - 执行远程命令

```bash
uv run python "$SCRIPT" exec -c <命令>
```

**这是核心命令**，用于减少授权询问次数。所有需要直接在集群上执行的命令都应通过 `exec` 进行。

**重要提示：**
- AI 模型必须在调用 `exec` 命令前进行安全评估
- 危险命令必须使用 `AskUserQuestion` 工具请求用户确认

**危险命令包括：**
- 删除操作：`rm -rf`、`rmdir` 等
- 破坏性操作：`dd`、`mkfs`、格式化等
- 系统影响：`kill -9 -1`、`shutdown`、`reboot` 等
- 权限修改：`chmod 000`、`chown` 等

**安全命令示例：**
- `ls`、`cat`、`grep`
- `squeue`、`sinfo`
- `head`、`tail`

**使用示例：**
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

**危险命令处理流程：**

当用户请求执行危险命令时，使用 `AskUserQuestion` 请求确认：

```json
{
  "questions": [
    {
      "question": "即将执行危险命令：rm -rf /tmp/test，这将永久删除该目录及其内容。是否继续？",
      "options": ["继续执行", "取消操作"]
    }
  ]
}
```
