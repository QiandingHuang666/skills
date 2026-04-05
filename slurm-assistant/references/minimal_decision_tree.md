# 最小决策树

这是 `slurm-assistant` 的最短执行协议。优先遵循本文件；只有在缺少具体步骤时，再去读其他 references。

---

## 1. 会话开始：先检查

```bash
uv run python "$SCRIPT" init --check --output-json --fast
```

只看：

- `configured`
- `config_valid`
- `local_slurm_available`
- `connection_count`
- `connections`
- `current_agent_authorized`

---

## 2. 三路分流

### 未配置

- 在集群上 → `workflow_local_execution.md`
- 不在集群上 / 需要 SSH → `workflow_init.md`

### 已配置但无效

- 停止执行用户原请求
- 先修配置，再继续

### 已配置且有效

- 单连接 → 直接执行
- 多连接 → 先 `connection --list`，再选连接

---

## 3. 六类任务

### 资源查看

```bash
uv run python "$SCRIPT" status --gpu
uv run python "$SCRIPT" find-gpu
```

### 作业管理

```bash
uv run python "$SCRIPT" jobs
uv run python "$SCRIPT" submit <script>
uv run python "$SCRIPT" log <job_id>
uv run python "$SCRIPT" cancel <job_id>
uv run python "$SCRIPT" alloc -p <partition> [-g gpu:1]
```

### 文件传输

```bash
uv run python "$SCRIPT" upload <local> <remote>
uv run python "$SCRIPT" download <remote> <local>
```

### 环境配置

涉及安装/编译/大下载时，先判断是否在登录节点；若是，先申请资源。

### 多连接 / 实例

```bash
uv run python "$SCRIPT" connection --list
```

### 任意远程命令

```bash
uv run python "$SCRIPT" exec -c '<cmd>'
```

只在现有子命令不够用时使用。

---

## 4. 安全分级

- A 类：只读/轻量 → 直接执行
- B 类：会改用户目录 → 说明后执行
- C 类：重操作 → 不在登录节点直接做
- D 类：危险/破坏性 → 必须先确认

---

## 5. 参考文档映射

- 首次配置：`workflow_init.md`
- 本地模式：`workflow_local_execution.md`
- 资源状态：`workflow_status.md`
- 作业：`workflow_job.md`
- 文件：`workflow_file_transfer.md`
- 环境：`workflow_env_config.md`

