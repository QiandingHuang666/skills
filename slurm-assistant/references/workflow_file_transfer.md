# 文件传输流程

文件上传/下载的规范流程和默认行为。

---

## 前置检查

### 上传前

```bash
ls -lh <本地路径>
```

### 下载前

```bash
slurm-client exec --connection <connection_id> --cmd "ls -lh <远程路径>" --json
```

如果路径不存在，不要盲传；先让用户确认路径是否写错。

---

## 默认路径规则

### 上传

| 用户指定 | 默认行为 |
|---------|---------|
| 只给源文件名 | 目标补全为 `~/<文件名>` |
| 指定源和目标 | 使用用户指定路径 |
| 源是目录 | 自动加 `-r` |

示例：

```bash
slurm-client upload train.py ~/train.py --connection <connection_id> --json
slurm-client upload data/ ~/data --connection <connection_id> -r --json
```

### 下载

| 用户指定 | 默认行为 |
|---------|---------|
| 只给远程文件名 | 本地补全为 `./<文件名>` 或用户当前工作目录 |
| 指定远程和本地 | 使用用户指定路径 |
| 远程是目录 | 自动加 `-r` |

示例：

```bash
slurm-client download ~/slurm-12345.out ./slurm-12345.out --connection <connection_id> --json
slurm-client download ~/checkpoints ./checkpoints --connection <connection_id> -r --json
```

---

## 传输前确认

- 文件超过 1GB 时，先提醒用户耗时和网络成本
- 目录传输时自动补 `-r`
- 远程路径涉及覆盖已有结果时，先说明

---

## 传输后验证

### 上传验证

```bash
slurm-client exec --connection <connection_id> --cmd "ls -lh <远程目标路径>" --json
```

### 下载验证

```bash
ls -lh <本地目标路径>
```

---

## 常见场景

### 场景 1：上传训练脚本

1. 检查本地文件：`ls -lh train.py`
2. 上传：`slurm-client upload train.py ~/train.py --connection <connection_id> --json`
3. 验证：`slurm-client exec --connection <connection_id> --cmd "ls -lh ~/train.py" --json`

### 场景 2：下载作业输出

1. 搜索日志：`slurm-client exec --connection <connection_id> --cmd "ls -lt ~/slurm-*.out | head" --json`
2. 下载：`slurm-client download ~/slurm-12345.out ./slurm-12345.out --connection <connection_id> --json`
3. 验证：`ls -lh ./slurm-12345.out`

### 场景 3：上传目录

1. 检查本地目录：`ls -lh imagenet/`
2. 如体积很大先提醒
3. 上传：`slurm-client upload imagenet/ ~/imagenet --connection <connection_id> -r --json`
4. 验证：`slurm-client exec --connection <connection_id> --cmd "ls -lh ~/imagenet | head" --json`

---

## 相关文档

- `references/commands.md`
