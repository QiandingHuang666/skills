# server + client + skill 架构设计 v0

这是当前 `slurm-assistant` 的 Rust 主架构说明，不再以 `slurm-cli.py` 为主入口。

配套图见：`references/architecture-v0.excalidraw`

---

## 1. 当前状态

仓库主链路已经统一为：

- `slurm-server`：本机常驻服务
- `slurm-client`：给 agent 和终端调用的稳定 CLI
- `SKILL.md`：约束模型如何选择命令、如何做安全分流、如何组织输出

Python 单体 CLI 已退出主架构，不再作为默认能力面。

---

## 2. 核心原则

### Principle 1：client 只连接本机 server

统一规则：

> client 发起的请求只由与 client 同主机的 server 处理。

这同时覆盖：

- Windows PC
- macOS / Linux 本机
- 集群登录节点
- 集群计算节点

### Principle 2：live state 只保存在本机

live state 包括：

- 本机 runtime 文件
- 本机 server 进程
- SSH / SCP 子进程
- 正在执行的命令

这些状态不跨节点共享。

### Principle 3：共享的是持久化数据，不是活连接

在多节点场景下，共享层只保留持久化状态：

- 连接配置
- 历史记录
- 资源缓存

持久化采用 SQLite，并启用 WAL / busy timeout 以适应并发读写。

### Principle 4：高层命令优先

agent 默认优先使用：

- `server status`
- `connection list`
- `status --gpu`
- `find-gpu`
- `partition-info`
- `jobs`
- `log`
- `submit`
- `cancel`

只有在高层语义不够时才使用 `exec`。

---

## 3. 分层职责

## Skill

职责：

- 给模型提供最小决策树
- 约束安全边界
- 规定输出格式

不负责：

- 直接 SSH
- 直接读写数据库
- 管理 IPC

## Client

二进制名：

```text
slurm-client
```

职责：

- 参数解析
- 发现本机 server
- 发起 HTTP RPC
- 输出文本或 JSON
- 给 agent 一个稳定的调用面

## Server

二进制名：

```text
slurm-server
```

职责：

- 本机常驻服务
- 管理 runtime / token / sqlite
- 执行本地命令或远程 SSH / SCP
- 暴露 `/v1/...` API

---

## 4. IPC

当前默认 transport：

- `localhost TCP`
- bearer token 鉴权

原因：

- 能覆盖 Windows
- 实现简单，利于 skill 和 client 保持统一

runtime 文件：

- Linux/macOS：`~/.local/share/slurm-assistant/runtime.json`
- Windows：`%APPDATA%/slurm-assistant/runtime.json`

---

## 5. 持久化

数据库：

```text
~/.local/share/slurm-assistant/state.db
```

SQLite 配置：

```sql
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA busy_timeout=5000;
```

---

## 6. 执行模型

### 本地模式

- client 请求同机 server
- server 直接调用本机 `squeue`、`sbatch`、`scontrol`

### 远程模式

- client 请求同机 server
- server 调用系统 `ssh` / `scp`
- 不在 client 层拼接远程执行细节

---

## 7. 测试驱动

当前测试分三层：

- Rust 单元测试：覆盖 proto、client、server
- live smoke：真实连贵州大学集群做端到端验证
- trace eval：检查模型工具轨迹是否遵循 skill 规定

评测主链路也应遵循同样原则：

- 不再依赖 Python wrapper
- 优先测试 Rust client 返回结果及其解析

---

## 8. 后续方向

后续可以继续补充：

- 更正式的 trace eval 报告格式
- 更多 server API
- Windows 侧运行手册
- 对 `partition-info / node-info / node-jobs` 的更细粒度验证
