# 首次使用流程

slurm-assistant 首次使用的配置和初始化流程。

---

## 1. 检查配置状态

```bash
uv run python "$SCRIPT" init --check --output-json
```

**输出示例：**
```json
{"configured": false, "local_slurm_available": false}
```

---

## 2. 如果未配置，收集配置信息

### 第一步：询问使用场景

使用 `AskUserQuestion` 工具：

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

### 第二步：根据选择继续收集

#### A. 贵州大学 HPC 集群

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

#### B. 其他 Slurm 集群（远程）

```json
{
  "questions": [
    {"question": "请输入集群名称（如：xx大学超算）", "options": ["输入集群名称"]},
    {"question": "请输入集群登录节点地址", "options": ["输入地址（如 login.hpc.edu）"]},
    {"question": "请输入 SSH 端口", "options": ["22（默认）", "其他端口"]},
    {"question": "请输入您的用户名", "options": ["输入用户名"]},
    {"question": "是否需要通过跳板机连接？", "options": ["不需要", "需要"]},
    {"question": "是否已配置免密登录？", "options": ["已配置", "未配置，需要帮助"]}
  ]
}
```

如未配置免密登录，参考 `references/set_free_password.md`

#### C. 当前已在集群上（本地模式）

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

---

## 3. 保存配置

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

## 相关文档

- `references/set_free_password.md` - 免密登录配置
- `references/use_gzu.md` - 贵州大学 HPC 配置详情
- `references/use_other.md` - 其他集群配置详情
- `references/use_local.md` - 本地模式使用
