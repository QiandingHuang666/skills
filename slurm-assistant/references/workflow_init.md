# 首次使用流程

Rust 版 `slurm-assistant` 的初始化目标只有两件事：

1. 确保与 client 同机的 `slurm-server` 已启动
2. 创建至少一个可用的连接记录

---

## 1. 启动并检查 server

```bash
slurm-server serve
```

另一个终端检查：

```bash
slurm-client server status --json
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

若用户消息包含“贵州大学 / 贵大 / GZU”，必须先进入贵州大学专属分支：

1. 先读取 `references/use_gzu.md`
2. 若涉及“实例”，先判定实例类型：容器实例(SSH) / 虚拟机实例(WebDAV)
3. 先确认路径映射（个人目录 / 项目目录 / 公共集群目录）
4. 完成映射确认前，禁止执行安装、软链接、数据目录写操作

---

## 3. 添加连接

### 贵州大学 HPC

```bash
slurm-client connection add \
  --label gzu-cluster \
  --host 210.40.56.85 \
  --port 21563 \
  --user "<用户名>" \
  --kind cluster \
  --json
```

### 其他远程集群

```bash
slurm-client connection add \
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
slurm-client connection add \
  --label "<实例名>" \
  --host "<host>" \
  --port <port> \
  --user "<用户名>" \
  --kind instance \
  --json
```

若是贵州大学实例，在 `connection add` 之后仍需先做一次路径映射确认，再执行后续命令：

- 容器实例：按 `use_gzu.md` 使用 `/home/<username>` 与 `/groups/public_cluster/...`
- 虚拟机实例：按 `use_gzu.md` 使用 `/webdav/...`

### 集群本地模式

```bash
slurm-client connection add \
  --label local-cluster \
  --kind local \
  --json
```

---

## 4. 验证连接

先列出连接：

```bash
slurm-client connection list --json
```

再做轻量探测：

```bash
slurm-client exec --connection <connection_id> --cmd 'hostname' --json
```

如果失败：

- 检查 SSH 免密登录是否已配置
- 检查 host / port / user 是否填错
- 检查是否需要跳板机

免密登录配置可参考 `references/set_free_password.md`。

贵州大学实例补充：

- 连接验证通过后，先执行“路径映射验证”命令，再进行环境配置或数据操作
- 输出里必须明确写出本次使用的映射路径，不能只给通用命令

---

## 5. 首个常用动作

连接成功后，建议立即跑一个高层命令确认数据链路正常：

```bash
slurm-client jobs --connection <connection_id> --json
```

或：

```bash
slurm-client status --connection <connection_id> --gpu --json
```

---

## 相关文档

- `references/commands.md`
- `references/use_gzu.md`
- `references/use_other.md`
- `references/use_local.md`
