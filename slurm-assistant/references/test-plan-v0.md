# 测试计划 v0

本计划用于指导 `slurm-server` + `slurm-client` + `skill` 的测试驱动开发。

核心原则：

- 先写契约测试，再写实现
- 不只测函数，还要测协议与行为
- 高层命令优先以 JSON 契约为准

---

## 1. 测试分层

测试分 5 层：

1. `proto` 单元测试
2. `server` 单元/集成测试
3. `client` CLI 测试
4. `e2e` 测试
5. `skill eval` / `compliance eval`

---

## 2. proto 测试

目标：

- 确认共享模型稳定
- 确认 JSON 序列化和反序列化不漂移

### 覆盖范围

- request structs
- response structs
- error enums
- connection/session/job model

### 首批测试

- `ping_response_roundtrip`
- `connection_add_request_roundtrip`
- `exec_run_response_roundtrip`
- `slurm_jobs_response_roundtrip`
- `error_response_roundtrip`

### 通过标准

- 所有结构可稳定 serde roundtrip
- JSON 字段名与 API contract 一致

---

## 3. server 测试

目标：

- 验证 server 运行时行为
- 验证存储、鉴权、接口和错误语义

### 3.1 启动与运行时

首批测试：

- `server_starts_and_writes_runtime`
- `server_ping_ok`
- `server_status_ok`
- `server_rejects_invalid_token`
- `server_returns_unauthorized_without_token`

### 3.2 Store / SQLite

首批测试：

- `sqlite_wal_enabled`
- `connection_insert_and_list`
- `connection_delete`
- `job_insert_and_query`
- `cache_insert_and_expire`
- `concurrent_writes_do_not_corrupt_state`

### 3.3 Connection API

首批测试：

- `add_connection_roundtrip`
- `list_connections_returns_added_item`
- `remove_connection_roundtrip`
- `duplicate_connection_is_rejected_or_upserted_as_specified`

### 3.4 Exec API

首批测试：

- `local_exec_returns_stdout_and_exit_code`
- `remote_exec_returns_stdout_and_exit_code`
- `exec_timeout_returns_timeout_error`
- `exec_failure_returns_exec_failed`

### 3.5 Slurm API

首批测试：

- `jobs_empty_result_is_well_formed`
- `log_missing_file_is_well_formed`
- `status_gpu_empty_summary_is_well_formed`
- `find_gpu_empty_summary_is_well_formed`

---

## 4. client 测试

目标：

- 验证 CLI 参数、server 发现、输出稳定性和退出码

### 4.1 runtime 发现

首批测试：

- `client_reads_runtime_file`
- `client_missing_runtime_returns_clear_error`
- `client_invalid_runtime_returns_clear_error`

### 4.2 基础命令

首批测试：

- `client_server_status_json_ok`
- `client_server_status_text_ok`
- `client_connection_list_json_ok`

### 4.3 错误处理

首批测试：

- `client_surfaces_unauthorized_error`
- `client_surfaces_server_unavailable_error`
- `client_returns_nonzero_on_failure`

### 4.4 文本输出稳定性

首批测试：

- `client_exec_text_output_stable`
- `client_jobs_text_output_stable`
- `client_status_gpu_text_output_stable`

---

## 5. E2E 测试

目标：

- 跑通真 server + 真 client
- 确认本机 IPC 和 server 生命周期闭环

### 场景 1：本地最小闭环

- 启动 server
- client 调 `server status`
- 新增 local connection
- 运行 `exec -c "hostname"`

### 场景 2：远程最小闭环

- 新增 cluster connection
- 运行远程 `exec`
- 获取 `jobs`

### 首批测试

- `server_and_client_ping_roundtrip`
- `local_connection_exec_roundtrip`
- `remote_connection_exec_roundtrip`
- `jobs_json_roundtrip`
- `status_gpu_json_roundtrip`

---

## 6. Skill Eval

目标：

- 验证 agent 是否遵循 skill
- 验证 agent 是否优先调用高层 client 命令

### 必测行为

- 会话开始先检查本机 server
- connection 未配置时先引导 connection 管理
- 查询 GPU 优先 `status --gpu` / `find-gpu`
- 查询作业优先 `jobs`
- 仅在高层命令不足时才 `exec`
- 危险命令先确认

### 首批 case

- `server_check_precedes_business_command`
- `gpu_query_prefers_status_gpu`
- `jobs_query_prefers_jobs`
- `file_query_prefers_upload_download`
- `dangerous_command_requires_confirmation`

---

## 7. 真实环境回归测试

当前仓库已有真实贵州大学集群 eval 基础，可逐步迁移到新 client。

目标环境：

- Host: `210.40.56.85`
- Port: `21563`
- User: `qiandingh`

### 第一批 live tests

- `status --gpu`
- `find-gpu`
- `jobs`
- `log <不存在的 job>`
- `exec -c hostname`

当前已落地的脚本：

- `slurm-assistant/rust/scripts/live_smoke_gzu.sh`

该脚本会串行验证：

- `jobs`
- `status --gpu`
- `find-gpu`
- 缺失日志契约
- `upload/download`
- `submit`
- `release`

### 验收标准

- 所有关键命令都有 `--json`
- 文本输出与 JSON 能对应
- 失败时错误语义清晰，不静默吞错

---

## 8. TDD 开发顺序

推荐顺序：

1. 写 `proto` roundtrip tests
2. 写 `server` 的 `ping/status` tests
3. 写 `client server status` tests
4. 写 connection API tests
5. 写 exec API tests
6. 写 Slurm read API tests
7. 写 write API tests
8. 最后迁移 skill eval

原则：

- 每一批功能都先补测试
- 未通过测试的 API 不进入 skill

---

## 9. CI 建议

建议 CI 至少分 4 个 job：

### `rust-check`

- `cargo fmt --check`
- `cargo clippy -- -D warnings`

### `rust-test`

- `cargo test`

### `client-e2e`

- 启动测试 server
- 跑 client e2e

### `skill-eval`

- 跑 trace-based compliance eval
- 条件允许时跑 live eval

---

## 10. 完成定义

可认为测试计划执行到位的标准：

- API contract 的每个 endpoint 至少有一个 server 测试
- 每个 client 高层命令至少有一个 CLI 测试
- 至少有一条本地 e2e 和一条远程 e2e
- skill 有独立 compliance eval
- 真实贵州大学集群有最小回归用例
