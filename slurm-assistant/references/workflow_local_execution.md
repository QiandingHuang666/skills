# 集群本地执行模式规范

当用户已在集群节点上时，仍然遵循 `client -> 同机 server` 的统一架构；区别只是连接类型改为 `local`，server 不再走 SSH。

---

## 触发条件

当满足以下条件时，使用本地执行模式：

1. 用户明确表示“我已经在集群上”
2. 当前机器能直接运行 Slurm 命令
3. 已存在 `kind=local` 的连接，或可以立即创建一个

---

## 初始化

启动本机 server：

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-server -- serve
```

创建本地连接：

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-client -- connection add --label local-cluster --kind local --json
```

---

## 命令执行规范

### 1. 优先仍用 slurm-client

```bash
cargo run --quiet --bin slurm-client -- status --connection <connection_id> --gpu --json
cargo run --quiet --bin slurm-client -- jobs --connection <connection_id> --json
cargo run --quiet --bin slurm-client -- submit --connection <connection_id> job.sh --json
```

只有在 `slurm-client` 尚未覆盖的细节上，才考虑直接使用原生命令。

### 2. 文件操作

本地模式下，所有路径都在当前集群机器上：

- 小规模复制优先 `cp`
- 批量查询优先 `ls` / `find`
- 如要保持 agent 统一接口，也可以继续用 `exec`

### 3. 作业运行

本地模式下生成的脚本仍然通过 `submit` 或 `sbatch` 提交，本质流程不变。

---

## 输出要求

- 不使用 emoji
- 状态用文字，如 `[RUNNING]`
- 先总结，再贴必要命令输出

---

## 常见场景

### 用户在集群上查看 GPU

```bash
cargo run --quiet --bin slurm-client -- status --connection <connection_id> --gpu --json
```

### 用户在集群上提交作业

```bash
cargo run --quiet --bin slurm-client -- submit --connection <connection_id> job.sh --json
```

### 用户在集群上复制文件

```bash
cp ~/source.txt ~/dest.txt
ls -lh ~/dest.txt
```

---

## 安全要求

本地模式同样遵守四级安全分流：

- A 类：只读/轻量，可直接执行
- B 类：常规写操作，说明后执行
- C 类：重操作，避免在登录节点直接做
- D 类：破坏性操作，必须先确认

---

## 相关文档

- `references/commands.md`
- `references/workflow_job.md`
