# 用户作业与资源状况

查询和报告用户作业、集群资源状态的规范流程和格式要求。

---

## 查询场景分类

| 用户询问 | 优先命令 |
|---------|---------|
| “我的作业”“作业状态”“队列情况” | `jobs` |
| “GPU 情况”“有哪些 GPU”“GPU 空闲吗” | `status --gpu` 或 `find-gpu` |
| “CPU 节点”“CPU 资源”“普通节点” | `partition-info -p <partition>` |
| “节点状态”“分区情况” | `partition-info` |
| “某个节点上有谁在跑” | `node-jobs <node>` |
| “这个节点详情” | `node-info <node>` |

---

## 推荐执行顺序

1. 先确定 `connection_id`
2. 优先调用高层子命令，不直接 `exec squeue/sinfo`
3. 只有高层输出不够，才补充 `exec`

---

## 命令模板

### 作业状态

```bash
cargo run --quiet --bin slurm-client -- jobs --connection <connection_id> --json
```

### GPU 资源

```bash
cargo run --quiet --bin slurm-client -- status --connection <connection_id> --gpu --json
```

或：

```bash
cargo run --quiet --bin slurm-client -- find-gpu a10 --connection <connection_id> --json
```

### 分区状态

```bash
cargo run --quiet --bin slurm-client -- partition-info --connection <connection_id> -p cpu48c --json
```

### 节点详情

```bash
cargo run --quiet --bin slurm-client -- node-info gpu-a10-01 --connection <connection_id> --json
```

### 节点作业

```bash
cargo run --quiet --bin slurm-client -- node-jobs gpu-a10-01 --connection <connection_id> --json
```

---

## 报告格式规范

### 作业状态报告

```text
当前作业状态：
[RUNNING] 作业ID 12345 - training | gpu-a10-01 | 运行 2小时30分
[PENDING] 作业ID 12346 - inference | 排队中 | 等待 15分钟
```

要求：

- 状态用方括号包裹
- 优先提炼“运行中 / 排队中 / 已完成 / 未找到”
- 如果没有作业，要明确说明“当前没有可见作业”

### GPU 资源报告

```text
GPU 资源概览：
节点 gpu-a10-01: 2/4 张 A10 空闲，24/64 CPU 空闲
节点 gpu-a100-02: 0/4 张 A100 空闲，8/64 CPU 空闲

推荐：gpu-a10-01 目前最适合申请 1 张 A10
```

要求：

- 必须明确 GPU 空闲数/总数
- 必须明确 CPU 空闲数/总数
- 有推荐时，给出最合适节点或分区

### CPU / 分区报告

```text
分区 cpu48c 概览：
节点 cpu48c-01: 32/48 CPU 空闲，内存使用 41%
节点 cpu48c-02: 0/48 CPU 空闲，内存使用 97%
```

### 节点作业报告

```text
节点 gpu-a10-01 上的作业：
[RUNNING] 12345 training | qiandingh | 01:13:22
[PENDING] 12352 debug | qiandingh | Priority
```

---

## 默认行为

### 用户表述模糊时

- “看看资源” -> `status --gpu`
- “看看我的任务” -> `jobs`
- “看看节点情况” -> `partition-info`

### 用户只说“报告一下”

默认先做两项：

1. `jobs`
2. `status --gpu`

---

## 输出要求总结

- 不使用 emoji
- 状态用文字，如 `[RUNNING]`
- 优先给结论，不先贴大段原始输出
- 当命令返回 JSON 时，回答里要做二次归纳，不能把原始 JSON 直接甩给用户

---

## 相关文档

- `references/commands.md`
