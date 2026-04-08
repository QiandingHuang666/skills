# Slurm Client 命令参考

Rust 版 `slurm-assistant` 通过 `slurm-server + slurm-client` 工作。除 `server status` 和 `connection list/add/get/remove` 外，大多数命令都需要显式传入 `--connection <connection_id>`。

---

## 启动前提

优先让 client 自动确保 server 已运行：

```bash
slurm-client server ensure --json
```

建议先准备一个环境变量，避免重复输入：

```bash
CONN_ID="conn_gzu_cluster"
```

---

## 连接管理

### connection add

```bash
slurm-client connection add --label <label> --kind <local|cluster|instance|server> [--host <host>] [--port <port>] [--user <user>] [--jump-host <jump_host>] [--json]
```

示例：

```bash
slurm-client connection add \
  --label gzu-cluster \
  --host 210.40.56.85 \
  --port 21563 \
  --user qiandingh \
  --kind cluster \
  --json
```

### connection list

```bash
slurm-client connection list --json
```

### connection get

```bash
slurm-client connection get --id <connection_id> --json
```

### connection remove

```bash
slurm-client connection remove --id <connection_id> --json
```

---

## 集群状态

### status

```bash
slurm-client status --connection <connection_id> [--gpu] [--partition <partition>] [--json]
```

示例：

```bash
slurm-client status --connection "$CONN_ID" --gpu --json
slurm-client status --connection "$CONN_ID" --gpu --partition gpu-a10 --json
slurm-client status --connection "$CONN_ID" --partition cpu48c --json
```

### find-gpu

```bash
slurm-client find-gpu [<gpu_type>] --connection <connection_id> [--json]
```

示例：

```bash
slurm-client find-gpu --connection "$CONN_ID" --json
slurm-client find-gpu a10 --connection "$CONN_ID" --json
```

### partition-info

```bash
slurm-client partition-info --connection <connection_id> [-p <partition>] [--json]
```

### node-info

```bash
slurm-client node-info <node> --connection <connection_id> [--json]
```

### node-jobs

```bash
slurm-client node-jobs <node> --connection <connection_id> [--json]
```

---

## 作业管理

### alloc

```bash
slurm-client alloc --connection <connection_id> -p <partition> [-g <gres>] [-c <cpus>] [--time <time>] [--mem <mem>] [--nodelist <node>] [--max-wait <minutes>] [--execute] [--json]
```

说明：

- 默认输出的是建议执行的 `salloc` 命令
- 加 `--execute` 才会真正发起申请

示例：

```bash
slurm-client alloc --connection "$CONN_ID" -p gpu-a10 -g gpu:1 --json
slurm-client alloc --connection "$CONN_ID" -p gpu-a10 -g gpu:1 -c 8 --execute --json
```

### release

```bash
slurm-client release <job_id> --connection <connection_id> [--json]
```

### run

```bash
slurm-client run --connection <connection_id> [-p <partition>] [-g <gres>] [-c <cpus>] [--time <time>] [--mem <mem>] [--nodelist <node>] <command>... [--json]
```

示例：

```bash
slurm-client run --connection "$CONN_ID" -p gpu-a10 -g gpu:1 python train.py --epochs 1 --json
```

### submit

```bash
slurm-client submit --connection <connection_id> <script_path> [--json]
```

### jobs

```bash
slurm-client jobs --connection <connection_id> [--job-id <job_id>] [--json]
```

### log

```bash
slurm-client log <job_id> --connection <connection_id> [--json]
```

### cancel

```bash
slurm-client cancel <job_id>... --connection <connection_id> [--json]
```

说明：

- `cancel` 支持一次传多个作业 ID
- 当前未提供 `history` 子命令；历史查询后续再补

---

## 文件传输

### upload

```bash
slurm-client upload <local_path> <remote_path> --connection <connection_id> [-r] [--json]
```

### download

```bash
slurm-client download <remote_path> <local_path> --connection <connection_id> [-r] [--json]
```

示例：

```bash
slurm-client upload train.py ~/train.py --connection "$CONN_ID" --json
slurm-client download ~/slurm-12345.out ./slurm-12345.out --connection "$CONN_ID" --json
```

---

## 兜底命令

### exec

```bash
slurm-client exec --connection <connection_id> --cmd '<command>' [--timeout-secs <seconds>] [--json]
```

只在现有高层子命令不够用时使用。

安全分级：

- A 类：只读或轻量命令，可直接执行
- B 类：会改用户目录的常规命令，先说明再执行
- C 类：编译、大下载、长时任务，不要在登录节点直接做
- D 类：破坏性命令，必须先征得用户确认

示例：

```bash
slurm-client exec --connection "$CONN_ID" --cmd 'hostname' --json
slurm-client exec --connection "$CONN_ID" --cmd 'ls -lh ~/workspace' --json
slurm-client exec --connection "$CONN_ID" --cmd 'grep ERROR slurm-*.out' --json
```
