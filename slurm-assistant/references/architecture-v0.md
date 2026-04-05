# server + client + skill 架构设计 v0

本设计用于把当前单体 `slurm-cli.py` 演进为：

- `slurm-server`：Rust 常驻服务
- `slurm-client`：Rust CLI 二进制
- `SKILL.md`：agent 调用 `slurm-client` 的指引

配套图见：`references/architecture-v0.excalidraw`

---

## 1. 目标

### 目标

- 把 SSH、会话、缓存、持久化从 skill 和 CLI 中剥离出来
- 让 agent 只调用稳定的 client 接口
- 支持本地模式和远程模式
- 支持同一用户在不同节点上分别运行本机 server
- 以测试驱动为主线，避免再次演化成单体脚本

### 非目标

- 不做跨节点 server 路由
- 不做 live session 迁移
- 不做分布式协调
- 第一版不做持久 shell 上下文
- 第一版不做 Rust 原生 SSH 协议栈

---

## 2. 核心原则

### Principle 1：client 只连接本机 server

无论本地模式还是远程模式，规则统一为：

> client 发起的请求只由“与 client 同主机”的 server 处理。

这意味着：

- 登录节点上的 client 只连登录节点上的 server
- 计算节点上的 client 只连计算节点上的 server
- Windows/macOS/Linux 本机上的 client 只连本机上的 server

### Principle 2：live state 只存在于本机 server

live state 包括：

- 本地执行上下文
- SSH 控制连接
- 运行中的子进程
- 实时流式输出

这些状态不跨节点共享。

### Principle 3：共享的是持久化数据，不是活连接

在集群本地模式下，多个节点都可能读写用户 home 目录，因此共享层只包含：

- connection 配置
- session 摘要
- job history
- resource cache

持久化采用 SQLite，解决并发读写。

### Principle 4：高层语义命令优先

agent 优先通过 client 调用高层命令：

- `status --gpu`
- `find-gpu`
- `jobs`
- `log`
- `submit`
- `cancel`

只有在现有高层命令无法覆盖时，才调用 `exec`。

---

## 3. 分层结构

## Skill

职责：

- 定义最小决策树
- 定义安全边界
- 指导 agent 选择 `slurm-client` 命令

不负责：

- 直接 SSH
- 直接访问数据库
- 直接管理会话
- 处理 IPC 细节

## Client

建议二进制名：

```text
slurm-client
```

职责：

- 参数解析
- 发现本机 server
- 发起 RPC 请求
- 输出文本或 JSON
- 提供稳定退出码

不负责：

- 自己拼 SSH
- 自己写主状态数据库
- 自己持有 live session

## Server

建议二进制名：

```text
slurm-server
```

职责：

- 本机常驻服务
- 管理本机会话
- 本地执行和远程 SSH 执行
- 持久化状态
- 暴露 RPC 接口
- 聚合 Slurm 高层服务

---

## 4. IPC 设计

### 第一版方案

为了兼容 Windows/macOS/Linux，第一版统一采用：

- `localhost TCP`
- token 鉴权

具体为：

- server 监听 `127.0.0.1:<port>`
- 启动时生成随机 token
- 写入 runtime 文件供 client 发现

### 运行时文件

Linux/macOS:

```text
~/.local/share/slurm-assistant/runtime.json
```

Windows:

```text
%APPDATA%/slurm-assistant/runtime.json
```

示例：

```json
{
  "version": 1,
  "transport": "tcp",
  "host": "127.0.0.1",
  "port": 47831,
  "token": "random-secret",
  "pid": 12345,
  "started_at": "2026-04-05T12:34:56Z"
}
```

### 后续优化

后续可按平台替换 transport：

- Linux/macOS：Unix socket
- Windows：Named Pipe

但不影响 client/server 协议。

---

## 5. 持久化设计

数据库建议：

```text
~/.local/share/slurm-assistant/state.db
```

第一版使用 SQLite，并启用：

```sql
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA busy_timeout=5000;
```

### 为什么不用 JSON

- 多节点并发写入容易损坏
- 难以做事务
- 难以做索引与查询

SQLite 更适合：

- 连接配置
- 历史记录
- cache
- session 摘要

---

## 6. Session 模型

第一版只做两种 session：

### `local_exec`

用于本地模式，在当前节点直接执行命令。

### `ssh_control`

用于远程模式，通过系统 `ssh` 建立和复用远程会话。

### 第一版不做

- 持久 shell 上下文
- 跨请求共享 `cd`
- `conda activate` 后上下文保持
- 复杂交互 shell channel

这些可在第二版引入 `shell` session。

---

## 7. SSH 实现策略

第一版 server 不自己实现 SSH 协议，而是调用系统：

- `ssh`
- `scp`

原因：

- 对 HPC 环境兼容性更好
- 对跳板机、用户现有 `~/.ssh/config` 兼容更好
- 跨平台风险更低

server 需要封装自己的 transport 层，而不是让 client 直接调 shell。

---

## 8. 推荐 Rust 工程结构

```text
slurm-assistant/
  rust/
    Cargo.toml
    crates/
      slurm-proto/
      slurm-server/
      slurm-client/
```

### `slurm-proto`

共享：

- request / response structs
- error codes
- connection / session / job models

### `slurm-server`

负责：

- runtime file
- token auth
- IPC server
- sqlite store
- session manager
- openssh transport
- slurm services

### `slurm-client`

负责：

- CLI 参数解析
- runtime 发现
- rpc 调用
- 文本和 JSON 输出

---

## 9. 新旧体系迁移关系

当前：

- `scripts/slurm-cli.py` 是主入口

目标：

- `slurm-client` 成为主入口
- `slurm-cli.py` 迁移到 `compat/`，作为兼容壳存在

迁移策略：

1. 新 server/client 先落最小闭环
2. 读操作优先迁移
3. 写操作随后迁移
4. skill 改为调用 client
5. compat 层再逐步接管旧命令

---

## 10. Skill 层改造方向

skill 不再围绕 `slurm-cli.py` 展开，而应围绕：

- `slurm-client server status`
- `slurm-client connection list`
- `slurm-client status --gpu`
- `slurm-client jobs`
- `slurm-client exec -c ...`

最小决策树应改写为：

1. 检查本机 server 是否在线
2. 检查 connection 是否已配置
3. 优先高层命令
4. 必要时使用 `exec`

---

## 11. 第一版交付边界

### 必做

- Rust workspace
- `slurm-server`
- `slurm-client`
- localhost TCP + token
- SQLite store
- connection add/list
- `exec`
- `jobs`
- `status --gpu`
- 测试驱动

### 暂缓

- Unix socket / Named Pipe
- shell 持久上下文
- server 间协调
- session 漂移
- Web UI

---

## 12. 成功标准

满足以下条件时，可认为 v0 架构落地成功：

- client 能稳定发现并调用本机 server
- server 能稳定持有本机会话和数据库
- 新 client 能覆盖当前高频读路径
- skill 能主要围绕 client 指引
- live eval 与 skill eval 能接入新体系

