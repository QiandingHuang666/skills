# 最小决策树

这是 `slurm-assistant` 的最短执行协议。优先遵循本文件；只有在缺少具体步骤时，再去读其他 references。

---

## 1. 会话开始：先确保本机 server

```bash
slurm-client server ensure --json
```

---

## 2. 连接分流

先列连接：

```bash
slurm-client connection list --json
```

只看：

- 是否有可用连接
- 每个连接的 `id`
- 每个连接的 `kind`
- `label / host / port / user`

分流规则：

- `0` 个连接：去 `workflow_init.md`
- `1` 个连接：直接使用这个 `connection_id`
- 多个连接：按用户意图选 `cluster`、`instance` 或 `local`

---

## 3. 六类任务

### 资源查看

```bash
slurm-client status --connection <connection_id> --gpu --json
slurm-client find-gpu --connection <connection_id> --json
slurm-client partition-info --connection <connection_id> --json
```

### 作业管理

```bash
slurm-client jobs --connection <connection_id> --json
slurm-client submit --connection <connection_id> <script>
slurm-client log <job_id> --connection <connection_id> --json
slurm-client cancel <job_id> --connection <connection_id> --json
slurm-client alloc --connection <connection_id> -p <partition> [-g gpu:1] --json
```

`alloc` 规则：

- 用户明确“现在申请”时，必须加 `--execute`
- 不要把 `salloc` 手工执行步骤转交给用户

### 文件传输

```bash
slurm-client upload <local> <remote> --connection <connection_id> --json
slurm-client download <remote> <local> --connection <connection_id> --json
```

### 环境配置

涉及安装、编译、大下载时，先判断是否在登录节点；若是，先申请资源。

### 多连接 / 实例

```bash
slurm-client connection list --json
```

### 任意远程命令

```bash
slurm-client exec --connection <connection_id> --cmd '<cmd>' --json
```

只在现有子命令不够用时使用。

---

## 4. 安全分级

- A 类：只读/轻量，直接执行
- B 类：会改用户目录，说明后执行
- C 类：重操作，不在登录节点直接做
- D 类：危险/破坏性，必须先确认

---

## 5. 参考文档映射

- 首次配置：`workflow_init.md`
- 本地模式：`workflow_local_execution.md`
- 资源状态：`workflow_status.md`
- 作业：`workflow_job.md`
- 文件：`workflow_file_transfer.md`
- 环境：`workflow_env_config.md`
