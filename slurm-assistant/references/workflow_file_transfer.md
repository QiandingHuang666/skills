# 文件传输流程

文件上传/下载的规范流程和默认行为。

---

## 前置检查

### 1. 确认文件路径

执行任何传输操作前，必须先确认文件路径是否正确。

**上传前检查：**
```bash
# 本地文件检查
ls -lh <本地路径>
```

**下载前检查：**
```bash
# 远程文件检查
uv run python "$SCRIPT" exec -c "ls -lh <远程路径>"
```

如果文件/目录不存在，必须使用 `AskUserQuestion` 询问用户：

```json
{
  "questions": [
    {
      "question": "文件 <路径> 不存在，是否继续？",
      "options": ["继续操作", "取消操作"]
    }
  ]
}
```

---

## 默认路径规则

### 上传 (upload)

| 用户指定 | 默认行为 |
|---------|---------|
| 只指定源文件 | 目标路径 = `~/` (家目录) |
| 指定源和目标 | 使用用户指定的路径 |
| 相对路径 | 相对于 `~/` 目录 |
| 当前目录 | 使用 `.` 表示 |

**示例：**
```bash
# 用户说"上传 train.py"
# AI 应该执行：
uv run python "$SCRIPT" upload train.py ~/

# 用户说"上传 data/ 到 /workspace/datasets"
# AI 应该执行：
uv run python "$SCRIPT" upload data/ /workspace/datasets -r
```

### 下载 (download)

| 用户指定 | 默认行为 |
|---------|---------|
| 只指定远程文件 | 本地路径 = `~/` (家目录) |
| 指定远程和本地 | 使用用户指定的路径 |
| 相对路径 | 相对于 `~/` 目录 |
| 当前目录 | 使用 `.` 表示 |

**示例：**
```bash
# 用户说"下载 slurm-12345.out"
# AI 应该执行：
uv run python "$SCRIPT" download slurm-12345.out ~/

# 用户说"下载 /workspace/models/checkpoint.pth 到 ./checkpoints/"
# AI 应该执行：
uv run python "$SCRIPT" download /workspace/models/checkpoint.pth ./checkpoints/
```

---

## 传输前确认

### 大文件警告

文件大于 1GB 时，必须询问用户确认：

```json
{
  "questions": [
    {
      "question": "文件较大（约 2.5GB），传输可能需要较长时间。是否继续？",
      "options": ["继续传输", "取消"]
    }
  ]
}
```

### 目录传输

传输目录时必须使用 `-r` 参数。如果用户未指定，AI 应自动添加。

---

## 传输后验证

### 上传验证

上传完成后，验证文件是否存在：

```bash
uv run python "$SCRIPT" exec -c "ls -lh <目标路径>"
```

### 下载验证

下载完成后，验证本地文件：

```bash
ls -lh <本地路径>
```

---

## 常见场景

### 场景 1：上传训练脚本

用户："上传我的训练脚本"

AI 流程：
1. 检查本地文件：`ls -lh train.py`
2. 确认存在后上传：`uv run python "$SCRIPT" upload train.py ~/`
3. 验证：`uv run python "$SCRIPT" exec -c "ls -lh ~/train.py"`

### 场景 2：下载作业输出

用户："下载今天的作业输出"

AI 流程：
1. 询问具体文件名或搜索：`uv run python "$SCRIPT" exec -c "ls -lt ~/slurm-*.out | head"`
2. 确认文件后下载：`uv run python "$SCRIPT" download slurm-12345.out ~/`
3. 验证：`ls -lh ~/slurm-12345.out`

### 场景 3：上传数据集目录

用户："上传 imagenet 数据集"

AI 流程：
1. 检查本地目录：`ls -lh imagenet/`
2. 检查大小，如果很大（>1GB）警告用户
3. 上传：`uv run python "$SCRIPT" upload imagenet/ ~/imagenet -r`
4. 验证：`uv run python "$SCRIPT" exec -c "ls -lh ~/imagenet/"`

---

## 相关文档

- `references/commands.md` - upload/download 命令详细用法
