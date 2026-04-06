# Slurm Client 命令参考

Rust 版 `slurm-assistant` 通过 `slurm-server + slurm-client` 工作。除 `server status` 和 `connection list/add/get/remove` 外，大多数命令都需要显式传入 `--connection <connection_id>`。

---

## 启动前提

先在本机启动 server：

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-server -- serve
```

确认 server 可用：

```bash
cargo run --quiet --bin slurm-client -- server status --json
```

建议先准备一个环境变量，避免重复输入：

```bash
CONN_ID="conn_gzu_cluster"
```

---

## 连接管理

### connection add

```bash
cargo run --quiet --bin slurm-client -- connection add --label <label> --kind <local|cluster|instance|server> [--host <host>] [--port <port>] [--user <user>] [--jump-host <jump_host>] [--json]
```

示例：

```bash
cargo run --quiet --bin slurm-client -- connection add \
  --label gzu-cluster \
  --host 210.40.56.85 \
  --port 21563 \
  --user qiandingh \
  --kind cluster \
  --json
```

### connection list

```bash
cargo run --quiet --bin slurm-client -- connection list --json
```

### connection get

```bash
cargo run --quiet --bin slurm-client -- connection get --id <connection_id> --json
```

### connection remove

```bash
cargo run --quiet --bin slurm-client -- connection remove --id <connection_id> --json
```

---

## 集群状态

### status

```bash
cargo run --quiet --bin slurm-client -- status --connection <connection_id> [--gpu] [--partition <partition>] [--json]
```

示例：

```bash
cargo run --quiet --bin slurm-client -- status --connection "$CONN_ID" --gpu --json
cargo run --quiet --bin slurm-client -- status --connection "$CONN_ID" --gpu --partition gpu-a10 --json
cargo run --quiet --bin slurm-client -- status --connection "$CONN_ID" --partition cpu48c --json
```

### find-gpu

```bash
cargo run --quiet --bin slurm-client -- find-gpu [<gpu_type>] --connection <connection_id> [--json]
```

示例：

```bash
cargo run --quiet --bin slurm-client -- find-gpu --connection "$CONN_ID" --json
cargo run --quiet --bin slurm-client -- find-gpu a10 --connection "$CONN_ID" --json
```

### partition-info

```bash
cargo run --quiet --bin slurm-client -- partition-info --connection <connection_id> [-p <partition>] [--json]
```

### node-info

```bash
cargo run --quiet --bin slurm-client -- node-info <node> --connection <connection_id> [--json]
```

### node-jobs

```bash
cargo run --quiet --bin slurm-client -- node-jobs <node> --connection <connection_id> [--json]
```

---

## 作业管理

### alloc

```bash
cargo run --quiet --bin slurm-client -- alloc --connection <connection_id> -p <partition> [-g <gres>] [-c <cpus>] [--time <time>] [--mem <mem>] [--nodelist <node>] [--max-wait <minutes>] [--execute] [--json]
```

说明：

- 默认输出的是建议执行的 `salloc` 命令
- 加 `--execute` 才会真正发起申请

示例：

```bash
cargo run --quiet --bin slurm-client -- alloc --connection "$CONN_ID" -p gpu-a10 -g gpu:1 --json
cargo run --quiet --bin slurm-client -- alloc --connection "$CONN_ID" -p gpu-a10 -g gpu:1 -c 8 --execute --json
```

### release

```bash
cargo run --quiet --bin slurm-client -- release <job_id> --connection <connection_id> [--json]
```

### run

```bash
cargo run --quiet --bin slurm-client -- run --connection <connection_id> [-p <partition>] [-g <gres>] [-c <cpus>] [--time <time>] [--mem <mem>] [--nodelist <node>] <command>... [--json]
```

示例：

```bash
cargo run --quiet --bin slurm-client -- run --connection "$CONN_ID" -p gpu-a10 -g gpu:1 python train.py --epochs 1 --json
```

### submit

```bash
cargo run --quiet --bin slurm-client -- submit --connection <connection_id> <script_path> [--json]
```

### jobs

```bash
cargo run --quiet --bin slurm-client -- jobs --connection <connection_id> [--job-id <job_id>] [--json]
```

### log

```bash
cargo run --quiet --bin slurm-client -- log <job_id> --connection <connection_id> [--json]
```

### cancel

```bash
cargo run --quiet --bin slurm-client -- cancel <job_id>... --connection <connection_id> [--json]
```

说明：

- `cancel` 支持一次传多个作业 ID
- 当前未提供 `history` 子命令；历史查询后续再补

---

## 文件传输

### upload

```bash
cargo run --quiet --bin slurm-client -- upload <local_path> <remote_path> --connection <connection_id> [-r] [--json]
```

### download

```bash
cargo run --quiet --bin slurm-client -- download <remote_path> <local_path> --connection <connection_id> [-r] [--json]
```

示例：

```bash
cargo run --quiet --bin slurm-client -- upload train.py ~/train.py --connection "$CONN_ID" --json
cargo run --quiet --bin slurm-client -- download ~/slurm-12345.out ./slurm-12345.out --connection "$CONN_ID" --json
```

---

## 兜底命令

### exec

```bash
cargo run --quiet --bin slurm-client -- exec --connection <connection_id> --cmd '<command>' [--timeout-secs <seconds>] [--json]
```

只在现有高层子命令不够用时使用。

安全分级：

- A 类：只读或轻量命令，可直接执行
- B 类：会改用户目录的常规命令，先说明再执行
- C 类：编译、大下载、长时任务，不要在登录节点直接做
- D 类：破坏性命令，必须先征得用户确认

示例：

```bash
cargo run --quiet --bin slurm-client -- exec --connection "$CONN_ID" --cmd 'hostname' --json
cargo run --quiet --bin slurm-client -- exec --connection "$CONN_ID" --cmd 'ls -lh ~/workspace' --json
cargo run --quiet --bin slurm-client -- exec --connection "$CONN_ID" --cmd 'grep ERROR slurm-*.out' --json
```
