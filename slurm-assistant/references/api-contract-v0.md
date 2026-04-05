# API Contract v0

本文定义 `slurm-client` 与 `slurm-server` 之间的第一版接口契约。

目标：

- 先稳定协议
- 再实现 server/client
- 所有高层命令优先支持 `--json`

---

## 1. 通用约定

### 传输

- `HTTP`
- `127.0.0.1:<port>`
- `Authorization: Bearer <token>`

### Content-Type

```text
application/json
```

### 成功响应

```json
{
  "ok": true,
  "data": {}
}
```

### 失败响应

```json
{
  "ok": false,
  "error": {
    "code": "connection_not_found",
    "message": "Connection not found: conn_xxx"
  }
}
```

### 标准错误码

- `unauthorized`
- `server_unavailable`
- `invalid_request`
- `connection_not_found`
- `session_not_found`
- `exec_failed`
- `ssh_failed`
- `timeout`
- `not_supported`
- `internal_error`

---

## 2. Server 基础接口

## `GET /v1/ping`

用途：

- client 快速检查 server 是否存活

响应：

```json
{
  "ok": true,
  "data": {
    "message": "pong"
  }
}
```

## `GET /v1/server/status`

用途：

- 获取 server 运行状态

响应：

```json
{
  "ok": true,
  "data": {
    "pid": 12345,
    "started_at": "2026-04-05T12:34:56Z",
    "transport": "tcp",
    "host": "127.0.0.1",
    "port": 47831,
    "db_path": "/path/to/state.db",
    "runtime_path": "/path/to/runtime.json"
  }
}
```

---

## 3. Connection 接口

## `POST /v1/connections/add`

请求：

```json
{
  "label": "gzu-cluster",
  "host": "210.40.56.85",
  "port": 21563,
  "username": "qiandingh",
  "kind": "cluster",
  "jump_host": null
}
```

响应：

```json
{
  "ok": true,
  "data": {
    "connection_id": "conn_gzu_cluster",
    "created": true
  }
}
```

说明：

- `kind` 取值：`local | cluster | instance | server`

## `GET /v1/connections/list`

响应：

```json
{
  "ok": true,
  "data": {
    "connections": [
      {
        "id": "conn_gzu_cluster",
        "label": "gzu-cluster",
        "host": "210.40.56.85",
        "port": 21563,
        "username": "qiandingh",
        "kind": "cluster",
        "jump_host": null
      }
    ]
  }
}
```

## `GET /v1/connections/{id}`

响应：

```json
{
  "ok": true,
  "data": {
    "id": "conn_gzu_cluster",
    "label": "gzu-cluster",
    "host": "210.40.56.85",
    "port": 21563,
    "username": "qiandingh",
    "kind": "cluster",
    "jump_host": null
  }
}
```

## `DELETE /v1/connections/{id}`

响应：

```json
{
  "ok": true,
  "data": {
    "deleted": true
  }
}
```

---

## 4. Exec 接口

## `POST /v1/exec/run`

请求：

```json
{
  "connection_id": "conn_gzu_cluster",
  "command": "hostname",
  "timeout_secs": 30
}
```

响应：

```json
{
  "ok": true,
  "data": {
    "stdout": "workstation\n",
    "stderr": "",
    "exit_code": 0
  }
}
```

约定：

- 本地模式走 `local_exec`
- 远程模式走 `ssh_control`
- `stdout` 和 `stderr` 都保留
- 必须返回 `exit_code`

---

## 5. Slurm 接口

## `POST /v1/slurm/status_gpu`

请求：

```json
{
  "connection_id": "conn_gzu_cluster",
  "partition": null
}
```

响应：

```json
{
  "ok": true,
  "data": {
    "available_nodes": [
      {
        "node": "gpu-a10-17",
        "partition": "gpu-a10",
        "gpu_idle": 2,
        "gpu_total": 2,
        "gpu_type": "A10"
      }
    ],
    "drain_nodes": [],
    "summary": {
      "available_nodes": 62,
      "total_gpu": 194,
      "idle_gpu": 12
    }
  }
}
```

## `POST /v1/slurm/find_gpu`

请求：

```json
{
  "connection_id": "conn_gzu_cluster",
  "gpu_type": null
}
```

响应：

```json
{
  "ok": true,
  "data": {
    "available_nodes": [],
    "busy_nodes": [],
    "drain_nodes": [],
    "summary": {
      "available_nodes": 0,
      "total_gpu": 0,
      "idle_gpu": 0
    }
  }
}
```

## `POST /v1/slurm/jobs`

请求：

```json
{
  "connection_id": "conn_gzu_cluster",
  "job_id": null
}
```

响应：

```json
{
  "ok": true,
  "data": {
    "jobs": [
      {
        "job_id": "57373",
        "partition": "gpu-a10",
        "name": "interactive",
        "user": "qiandingh",
        "state": "R",
        "time": "17:08:48",
        "nodes": 1,
        "reason": "gpu-a10-13"
      }
    ]
  }
}
```

## `POST /v1/slurm/log`

请求：

```json
{
  "connection_id": "conn_gzu_cluster",
  "job_id": "57373"
}
```

响应：

```json
{
  "ok": true,
  "data": {
    "job_id": "57373",
    "found": true,
    "content": "..."
  }
}
```

若未找到：

```json
{
  "ok": true,
  "data": {
    "job_id": "0",
    "found": false,
    "content": "Log file not found"
  }
}
```

## `POST /v1/slurm/submit`

请求：

```json
{
  "connection_id": "conn_gzu_cluster",
  "script_path": "job.sh"
}
```

响应：

```json
{
  "ok": true,
  "data": {
    "job_id": "60001",
    "raw_output": "Submitted batch job 60001"
  }
}
```

## `POST /v1/slurm/cancel`

请求：

```json
{
  "connection_id": "conn_gzu_cluster",
  "job_ids": ["60001", "60002"]
}
```

响应：

```json
{
  "ok": true,
  "data": {
    "cancelled": ["60001", "60002"]
  }
}
```

---

## 6. File 接口

## `POST /v1/files/upload`

请求：

```json
{
  "connection_id": "conn_gzu_cluster",
  "local_path": "/tmp/train.py",
  "remote_path": "~/train.py",
  "recursive": false
}
```

## `POST /v1/files/download`

请求：

```json
{
  "connection_id": "conn_gzu_cluster",
  "remote_path": "~/slurm-57373.out",
  "local_path": "/tmp/slurm-57373.out",
  "recursive": false
}
```

---

## 7. Client 命令面映射

建议命令：

```bash
slurm-client server status
slurm-client connection add --label gzu-cluster --host 210.40.56.85 --port 21563 --user qiandingh --kind cluster
slurm-client connection list
slurm-client status --gpu
slurm-client find-gpu
slurm-client jobs
slurm-client log 57373
slurm-client exec -c "hostname"
```

统一规则：

- 默认文本输出
- `--json` 输出完整结构化数据

---

## 8. 契约要求

### 必须稳定的部分

- endpoint 名称
- JSON 字段名
- 错误码
- client 命令参数
- `--json` 输出结构

### 可以调整的部分

- 文本输出具体排版
- server 内部模块拆分
- runtime 文件额外字段

