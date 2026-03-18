# 其他 Slurm 集群（远程）配置

## 配置流程

**重要：必须使用 AskUserQuestion 工具收集信息，不要使用 shell 命令！**

### 1. 收集集群基本信息

```json
{
  "questions": [
    {
      "question": "请输入集群名称（如：xx大学超算)",
      "options": [
        "输入集群名称"
      ]
    },
    {
      "question": "请输入集群登录节点地址",
      "options": [
        "输入地址（如 login.hpc.edu）"
      ]
    },
    {
      "question": "请输入 SSH 端口",
      "options": [
        "22（默认）",
        "其他端口"
      ]
    },
    {
      "question": "请输入您的用户名",
      "options": [
        "输入用户名"
      ]
    }
  ]
}
```

### 2. 询问跳板机（可选)
```json
{
  "questions": [
    {
      "question": "是否需要通过跳板机连接？",
      "options": [
        "不需要",
        "需要（请提供跳板机地址）"
      ]
    }
  ]
}
```

### 3. 询问免密登录状态
```json
{
  "questions": [
    {
      "question": "是否已配置免密登录？",
      "options": [
        "已配置",
        "未配置，需要帮助"
      ]
    }
  ]
}
```

**如果选择"未配置，需要帮助"**，参考 `references/set_free_password.md`。

### 4. 保存配置

```bash
uv run python "$SCRIPT" init --mode remote \
  --cluster-name "用户输入的名称" \
  --host "用户输入的地址" \
  --port 用户输入的端口 \
  --username "用户输入的用户名" \
  --jump-host "跳板机地址（如有）"
```

---

## 错误示例（禁止使用）

```bash
# 错误！不要这样做
read -p "请输入用户名: " username
```

**正确做法：使用 askUserQuestion 工具**
