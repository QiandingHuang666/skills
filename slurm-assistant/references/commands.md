# Slurm Client 命令参考

Rust 版 `slurm-assistant` 通过 `slurm-server + slurm-client` 工作。除 `server status` 和 `connection list/add/get/remove` 外，大多数命令都需要显式传入 `--connection <connection_id>`。

---

## 启动前提

优先让 client 自动确保 server 已运行：

```bash
slurm-client server ensure --json
```

`server ensure/status` 返回里包含 `api_version` 和 `capabilities`。  
client 会在执行命令前按能力自动校验；若检测到本机 server 版本漂移，会自动重启本机 server 并重试。

建议先准备一个环境变量，避免重复输入：

```bash
CONN_ID="conn_gzu_cluster"
```

---

## 连接管理

### connection add

```bash
slurm-client connection add --label <label> --kind <local|cluster|instance|server|resource-node> [--host <host>] [--port <port>] [--user <user>] [--jump-host <jump_host>] [--default-keepalive-secs <seconds>] [--json]
```

示例：

```bash
slurm-client connection add \
  --label gzu-cluster \
  --host 210.40.56.85 \
  --port 21563 \
  --user qiandingh \
  --kind cluster \
  --default-keepalive-secs 1800 \
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

返回包含 `default_keepalive_secs`（连接默认保活秒数）。
对 `resource-node`，还会返回 `health_state / health_message / last_health_checked_at`。

### connection remove

```bash
slurm-client connection remove --id <connection_id> --json
```

---

## 会话管理

### session upsert

```bash
slurm-client session upsert --id <session_id> --connection <connection_id> --type <ssh|alloc|shell> [--description <text>] [--state <active|idle|closed>] [--node-role <login|compute|unknown>] [--remote-host <host>] [--compute-node <node>] [--keepalive-secs <seconds>] [--json]
```

### session list

```bash
slurm-client session list --json
```

### session summary

```bash
slurm-client session summary --json
```

用于快速查看当前活跃会话及每个连接的“当前会话”摘要。

### session get

```bash
slurm-client session get --id <session_id> --json
```

### session remove

```bash
slurm-client session remove --id <session_id> --json
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
slurm-client alloc --connection <connection_id> -p <partition> [-g <gres>] [-c <cpus>] [--time <time>] [--mem <mem>] [--nodelist <node>] [--max-wait <minutes>] [--preempt] [--preempt-session <name>] [--timeout-secs <seconds>] [--execute] [--json]
```

说明：

- 若未传 `--cpus`，client 会自动计算 `cpus-per-task`：
  `min(节点空闲CPU, 节点CPU总量/节点GPU总量 × 申请GPU数)`
- 默认输出的是建议执行的 `salloc` 命令
- 加 `--execute` 才会真正发起申请
- 当用户明确“现在申请资源”时，应默认加 `--execute`，不要把 `salloc` 手工步骤转交给用户
- 默认不传 `--mem` 和 `--time`；仅当用户明确指定时再添加
- `--execute` 模式下默认超时是 600 秒；排队较久时建议显式增大 `--timeout-secs`
- 开启 `--preempt` 后，会自动在远端用 `tmux` 启动 `salloc ... bash -lc 'sleep infinity'` 保活，避免会话断开后资源立即释放
- `--preempt-session` 可指定 tmux 会话名；不传时自动生成

示例：

```bash
slurm-client alloc --connection "$CONN_ID" -p gpu-a10 -g gpu:1 --json
slurm-client alloc --connection "$CONN_ID" -p gpu-a10 -g gpu:1 -c 8 --execute --json
slurm-client alloc --connection "$CONN_ID" -p gpu-a100-8card -g gpu:1 --execute --json
slurm-client alloc --connection "$CONN_ID" -p gpu-a100 -g gpu:1 --execute --timeout-secs 1800 --json
slurm-client alloc --connection "$CONN_ID" -p gpu-a100 -g gpu:1 --preempt --execute --json
slurm-client alloc --connection "$CONN_ID" -p gpu-a100 -g gpu:1 --preempt --preempt-session preempt_a100 --execute --json
```

### release

```bash
slurm-client release <job_id> --connection <connection_id> [--json]
```

### run

```bash
slurm-client run --connection <connection_id> [-p <partition>] [-g <gres>] [-c <cpus>] [--time <time>] [--mem <mem>] [--nodelist <node>] <command>... [--json]
```

说明：

- 与 `alloc` 一致：默认不传 `--mem` 和 `--time`；仅用户明确指定时添加

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
