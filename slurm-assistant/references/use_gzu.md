# 贵州大学 HPC 集群配置

## 集群信息（自动填充）

- **地址**: 210.40.56.85
- **端口**: 21563

## 配置流程

### 1. 使用 AskUserQuestion 收集用户名

**重要：必须使用 AskUserQuestion 工具，不要使用 shell 命令！**

```json
{
  "questions": [
    {
      "question": "请输入您的贵州大学 HPC 集群用户名",
      "options": [
        "在此输入用户名"
      ]
    }
  ]
}
```

### 2. 使用 AskUserQuestion 询问免密登录状态

```json
{
  "questions": [
    {
      "question": "您是否已配置免密登录？",
      "options": [
        "已配置",
        "未配置，需要帮助"
      ]
    }
  ]
}
```

**如果选择"未配置，需要帮助"**，参考 `references/set_free_password.md` 引导用户配置。

**注意：使用 AskUserQuestion 引导用户，而不是执行 shell 命令！**

### 3. 保存配置

使用收集到的用户名执行：

```bash
uv run python "$SCRIPT" init --mode remote \
  --cluster-name "贵州大学 HPC" \
  --host 210.40.56.85 \
  --port 21563 \
  --username "用户输入的用户名"
```

**注意：这是唯一的 slurm-cli.py 命令调用，不需要其他 shell 命令！**

## 相关文档

- 公共资源检查：`gzu_public_resources.md`
