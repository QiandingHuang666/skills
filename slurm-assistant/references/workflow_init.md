# 首次使用流程

Rust 版 `slurm-assistant` 的初始化目标只有两件事：

1. 确保与 client 同机的 `slurm-server` 已启动
2. 创建至少一个可用的连接记录

---

## 1. 启动并检查 server

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-server -- serve
```

另一个终端检查：

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-client -- server status --json
```

---

## 2. 收集用户场景

根据用户场景选择一种连接类型：

- 公共 Slurm 集群：`kind=cluster`
- 独立实例：`kind=instance`
- 当前已在集群节点上：`kind=local`

如果是贵州大学 HPC，固定参数为：

- Host：`210.40.56.85`
- Port：`21563`

---

## 3. 添加连接

### 贵州大学 HPC

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-client -- connection add \
  --label gzu-cluster \
  --host 210.40.56.85 \
  --port 21563 \
  --user "<用户名>" \
  --kind cluster \
  --json
```

### 其他远程集群

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-client -- connection add \
  --label "<连接名>" \
  --host "<host>" \
  --port <port> \
  --user "<用户名>" \
  --kind cluster \
  --jump-host "<jump_host，可选>" \
  --json
```

### 远程实例

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-client -- connection add \
  --label "<实例名>" \
  --host "<host>" \
  --port <port> \
  --user "<用户名>" \
  --kind instance \
  --json
```

### 集群本地模式

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-client -- connection add \
  --label local-cluster \
  --kind local \
  --json
```

---

## 4. 验证连接

先列出连接：

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-client -- connection list --json
```

再做轻量探测：

```bash
cd slurm-assistant/rust
cargo run --quiet --bin slurm-client -- exec --connection <connection_id> --cmd 'hostname' --json
```

如果失败：

- 检查 SSH 免密登录是否已配置
- 检查 host / port / user 是否填错
- 检查是否需要跳板机

免密登录配置可参考 `references/set_free_password.md`。

---

## 5. 首个常用动作

连接成功后，建议立即跑一个高层命令确认数据链路正常：

```bash
cargo run --quiet --bin slurm-client -- jobs --connection <connection_id> --json
```

或：

```bash
cargo run --quiet --bin slurm-client -- status --connection <connection_id> --gpu --json
```

---

## 相关文档

- `references/commands.md`
- `references/use_gzu.md`
- `references/use_other.md`
- `references/use_local.md`
