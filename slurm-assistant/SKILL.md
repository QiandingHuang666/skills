---
name: slurm-assistant
description: |
  Slurm HPC 集群助手，为高校学生/教师定制。支持本地（集群上）和远程（集群外）两种使用模式。

  TRIGGER 当用户：
  - 提到 slurm、sbatch、squeue、scancel、salloc、srun、sinfo 等 Slurm 命令
  - 提到 hpc 集群、slurm 集群、超算、计算节点、作业调度系统
  - 想要查看分区/节点状态、队列情况、GPU 可用性
  - 需要提交/取消/查看作业
  - 需要申请交互式资源或运行命令
  - 需要生成或修改 slurm 作业脚本
  - 需要上传/下载文件到 HPC 集群
  - 需要连接公共集群、实例或本地集群节点
---

# Slurm 集群助手

跨平台 Slurm HPC 集群管理工具，采用 `server + client + skill` 架构。

---

## 最小执行协议

### Step 0：优先使用 Rust

使用：

```bash
slurm-client --help
```

禁止把 Python CLI 当作默认入口。当前 skill 的主链路只应使用 Rust server/client。

### Step 1：先看本机 server

每次会话开始先执行：

```bash
slurm-client server ensure --json
```

### Step 2：检查连接

```bash
slurm-client connection list --json
```

然后快速检查现有会话（优先复用活跃会话）：

```bash
slurm-client session summary --json
```

分流：

- 没有连接：读 `references/workflow_init.md`
- 一个连接：直接记录其 `connection_id`
- 多个连接：按用户意图选 `cluster`、`instance` 或 `local`
- 若存在 `resource-node` 连接，先查看其 `health_state`，优先复用 `online` 状态连接

### Step 3：按 6 类任务执行

1. 资源查看
```bash
slurm-client status --connection <connection_id> --gpu --json
slurm-client find-gpu --connection <connection_id> --json
slurm-client partition-info --connection <connection_id> --json
```

2. 作业管理
```bash
slurm-client jobs --connection <connection_id> --json
slurm-client submit --connection <connection_id> <script> --json
slurm-client log <job_id> --connection <connection_id> --json
slurm-client cancel <job_id> --connection <connection_id> --json
slurm-client alloc --connection <connection_id> -p <partition> --json
slurm-client run --connection <connection_id> <command>... --json
```

`alloc` 执行规则（必须遵循）：

- 用户明确要“现在申请/直接申请/申请这张卡”时，必须使用 `--execute`
- 禁止只返回 `salloc` 规划命令后让用户手动执行
- 只有在用户明确要求“先看命令不执行”时，才允许不加 `--execute`

3. 文件传输
```bash
slurm-client upload <local> <remote> --connection <connection_id> --json
slurm-client download <remote> <local> --connection <connection_id> --json
```

4. 环境配置
- 涉及安装、编译、大下载时，优先引导到计算节点或批处理作业

5. 多连接 / 实例
```bash
slurm-client connection list --json
```

6. 兜底远程命令
```bash
slurm-client exec --connection <connection_id> --cmd '<command>' --json
```

### Step 4：安全分流

- A 类：只读/轻量，可直接执行
- B 类：普通写操作，说明后执行
- C 类：高成本操作，不在登录节点直接做
- D 类：危险/破坏性操作，必须先确认

### Step 5：输出要求

- 优先给结论和下一步
- 不直接把大段 JSON 原样贴给用户
- 不使用 emoji
- 状态用 `[RUNNING]`、`[PENDING]` 这类文字标签

---

## 不可违背原则

禁止在登录节点直接进行重操作：

- 大规模下载
- 编译大型软件
- 长时间数据处理
- 训练任务
- 高内存占用程序

正确做法：

1. 先 `alloc` 申请资源，或 `submit` 提交作业
2. 获得计算资源后再执行重操作
3. 完成后 `release` 或 `cancel`

---

## 多连接管理

连接通过 Rust client 统一管理：

```bash
slurm-client connection list --json
slurm-client connection get --id <connection_id> --json
slurm-client connection add --label <label> --kind <kind> --json
slurm-client connection remove --id <connection_id> --json
```

当存在多个连接时，后续命令必须显式传 `--connection <connection_id>`。

---

## 实例连接流程

当用户说“连接实例”“切换到实例”时：

1. 先 `connection list --json`
2. 如果已有目标实例连接，直接使用它的 `connection_id`
3. 如果没有，收集 `host / port / user`
4. 执行：

```bash
slurm-client connection add \
  --label "<实例名>" \
  --host "<host>" \
  --port <port> \
  --user "<用户名>" \
  --kind instance \
  --json
```

5. 用轻量探测验证：

```bash
slurm-client exec --connection <connection_id> --cmd 'hostname' --json
```

---

## 贵州大学 HPC 特例

固定连接参数：

- Host：`210.40.56.85`
- Port：`21563`

在贵州大学集群上，下载数据集或安装软件前，优先检查公共资源目录，详见 `references/gzu_public_resources.md`。

---

## 命令速查

| 类别 | 命令 | 详细 |
|------|------|------|
| server | `server status` | `references/commands.md` |
| 连接 | `connection add/list/get/remove` | `references/commands.md` |
| 状态 | `status` / `find-gpu` / `partition-info` / `node-info` / `node-jobs` | `references/commands.md` |
| 作业 | `alloc` / `release` / `run` / `submit` / `jobs` / `log` / `cancel` | `references/commands.md` |
| 文件 | `upload` / `download` | `references/commands.md` |
| 兜底 | `exec` | `references/commands.md` |

---

## 参考资源

| 文件 | 说明 |
|------|------|
| `references/minimal_decision_tree.md` | 最小决策树 |
| `references/commands.md` | 完整命令参考 |
| `references/workflow_init.md` | 首次使用流程 |
| `references/workflow_status.md` | 用户作业与资源状况 |
| `references/workflow_job.md` | 作业流程 |
| `references/workflow_file_transfer.md` | 文件上传下载 |
| `references/workflow_local_execution.md` | 集群本地模式 |
| `references/workflow_env_config.md` | 环境配置 |
| `references/gzu_public_resources.md` | 贵州大学公共资源 |
