# Slurm 常见错误及解决方案

本文档列出了使用 Slurm 时常见的错误及其解决方法。

---

## 连接和认证错误

### 错误: `server returned 404 Not Found`（例如 `session summary`）

**原因**:
- 本机 `runtime.json` 指向了旧版 `slurm-server` 进程
- `slurm-client` 与 `slurm-server` 二进制版本不一致

**解决方案**:
1. 先看 server 能力信息:
   ```bash
   slurm-client server status --json
   ```
2. 主动触发 ensure:
   ```bash
   slurm-client server ensure --json
   ```
3. 仍异常时，升级并确保 `slurm-client` / `slurm-server` 来自同一 release
4. 重试原命令（例如 `slurm-client session summary --json`）

说明：
- 新版 client 已内置“能力探测 + 自动重启本机 server + 重试”逻辑
- 如果重试后仍报错，通常是本机存在旧二进制残留或 PATH 指向错误版本

### 错误: Connection refused / Connection timed out

**原因**:
- 网络连接问题
- 集群维护中
- SSH 配置错误

**解决方案**:
1. 检查网络连接
2. 确认集群是否在维护
3. 验证 SSH 配置:
   ```bash
   ssh -vvv user@cluster.edu
   ```

### 错误: Permission denied (publickey)

**原因**:
- SSH 密钥未正确配置
- 公钥未添加到集群

**解决方案**:
1. 检查密钥是否存在:
   ```bash
   ls -la ~/.ssh/id_*
   ```
2. 复制公钥到集群:
   ```bash
   ssh-copy-id user@cluster.edu
   ```
3. 或手动添加:
   ```bash
   cat ~/.ssh/id_rsa.pub | ssh user@cluster.edu "cat >> ~/.ssh/authorized_keys"
   ```

### 错误: Too many authentication failures

**原因**:
- SSH 尝试了太多密钥

**解决方案**:
1. 指定使用特定密钥:
   ```bash
   ssh -i ~/.ssh/id_rsa user@cluster.edu
   ```
2. 在 `~/.ssh/config` 中添加:
   ```
   Host cluster.edu
       IdentityFile ~/.ssh/id_rsa
       IdentitiesOnly yes
   ```

---

## 作业提交错误

### 错误: Batch job submission failed: Socket timed out

**原因**:
- 控制节点响应慢
- 网络延迟

**解决方案**:
1. 稍后重试
2. 检查集群状态:
   ```bash
   sinfo
   squeue
   ```

### 错误: Invalid account or account/partition combination

**原因**:
- 未指定正确的账户
- 分区配置错误

**解决方案**:
1. 查看可用账户:
   ```bash
   sacctmgr show user $USER -s
   ```
2. 在脚本中指定账户:
   ```bash
   #SBATCH --account=your_account
   ```

### 错误: Requested node configuration is not available

**原因**:
- 请求的资源超出分区限制
- 分区没有空闲资源

**解决方案**:
1. 查看分区资源:
   ```bash
   sinfo -p partition_name -o "%P %G %N %C"
   ```
2. 减少资源请求或更换分区

### 错误: Job violates accounting/QOS policy

**原因**:
- 超出用户/组的资源限制
- QOS 策略限制

**解决方案**:
1. 查看限制:
   ```bash
   sacctmgr show qos
   ```
2. 查看当前使用:
   ```bash
   squeue -u $USER
   ```
3. 减少资源或等待其他作业完成

---

## 作业运行错误

### 错误: Job ran for less than 2 seconds

**原因**:
- 程序立即崩溃
- 环境变量/模块未加载
- 输入文件缺失

**解决方案**:
1. 检查错误日志
2. 手动运行测试:
   ```bash
   srun --pty bash
   ```
3. 确保正确加载模块

### 错误: Out of memory (OOM)

**原因**:
- 内存不足
- 内存泄漏

**解决方案**:
1. 增加内存请求:
   ```bash
   #SBATCH --mem=64G
   ```
2. 检查程序内存使用
3. 使用 `--mem-per-cpu` 替代

### 错误: CUDA out of memory

**原因**:
- GPU 显存不足
- 批量大小过大

**解决方案**:
1. 减小 batch size
2. 清理 GPU 缓存:
   ```python
   import torch
   torch.cuda.empty_cache()
   ```
3. 使用梯度累积

### 错误: Job cancelled due to time limit

**原因**:
- 作业超过时间限制

**解决方案**:
1. 增加时间限制:
   ```bash
   #SBATCH --time=48:00:00
   ```
2. 实现检查点 (checkpoint) 机制
3. 使用作业依赖链

---

## GPU 相关错误

### 错误: CUDA device not found

**原因**:
- GPU 资源未正确分配
- 驱动问题

**解决方案**:
1. 确认请求了 GPU:
   ```bash
   #SBATCH --gres=gpu:1
   ```
2. 检查 GPU 是否可见:
   ```bash
   nvidia-smi
   echo $CUDA_VISIBLE_DEVICES
   ```

### 错误: GPU already in use

**原因**:
- 多进程尝试使用同一 GPU

**解决方案**:
1. 使用 `CUDA_VISIBLE_DEVICES`:
   ```python
   import os
   os.environ["CUDA_VISIBLE_DEVICES"] = os.environ.get("CUDA_VISIBLE_DEVICES", "0")
   ```
2. 或在 PyTorch 中:
   ```python
   device = torch.device("cuda:0" if torch.cuda.is_available() else "cpu")
   ```

---

## 文件系统错误

### 错误: No space left on device

**原因**:
- 存储配额已满
- 临时文件未清理

**解决方案**:
1. 检查配额:
   ```bash
   df -h ~
   quota -s
   ```
2. 清理不必要的文件
3. 使用 scratch 目录存储大文件

### 错误: Disk quota exceeded

**原因**:
- 文件数限制
- 空间限制

**解决方案**:
1. 检查文件数:
   ```bash
   lfs quota -u $USER /path/to/fs
   ```
2. 清理小文件
3. 使用归档减少文件数

---

## 交互式会话错误

### 错误: salloc: Job allocation cancelled

**原因**:
- 长时间无活动
- 资源被抢占

**解决方案**:
1. 使用 tmux/screen:
   ```bash
   tmux new-session -d 'sleep 24h'
   ```
2. 定期发送活动信号

### 错误: srun: error: Unable to allocate resources

**原因**:
- 资源不足
- 请求无效

**解决方案**:
1. 查看可用资源:
   ```bash
   sinfo -o "%P %C"
   ```
2. 减少资源请求
3. 等待资源释放

---

## 模块和环境错误

### 错误: module: command not found

**原因**:
- Environment Modules 未初始化

**解决方案**:
在 `~/.bashrc` 中添加:
```bash
source /etc/profile.d/modules.sh
```

### 错误: Unable to locate module

**原因**:
- 模块不存在
- 模块路径未配置

**解决方案**:
1. 列出可用模块:
   ```bash
   module avail
   ```
2. 搜索模块:
   ```bash
   module spider python
   ```

### 错误: Conda environment not found

**原因**:
- 环境未激活
- 路径问题

**解决方案**:
1. 在脚本中初始化 conda:
   ```bash
   source ~/.bashrc
   conda activate my_env
   ```
2. 或使用完整路径:
   ```bash
   source /path/to/conda/etc/profile.d/conda.sh
   conda activate my_env
   ```

---

## 排查技巧

### 1. 查看作业详情
```bash
scontrol show job <job_id>
```

### 2. 查看历史作业
```bash
sacct -j <job_id> --format=JobID,State,ExitCode,Elapsed,MaxRSS
```

### 3. 查看节点状态
```bash
scontrol show node <node_name>
```

### 4. 查看日志
```bash
# 查看输出日志
cat slurm-<job_id>.out

# 查看错误日志
cat slurm-<job_id>.err

# 实时查看
tail -f slurm-<job_id>.out
```

### 5. 测试脚本
```bash
# 测试模式 (不实际提交)
sbatch --test-only script.sh

# 交互式测试
srun --pty bash
```

---

## 获取帮助

如果以上方法都无法解决问题:

1. 查看集群文档/FAQ
2. 联系集群管理员
3. 查询集群用户组/论坛
4. 使用 `scontrol show` 获取详细信息后向管理员报告
