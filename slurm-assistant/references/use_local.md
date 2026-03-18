# 本地模式配置

## 使用场景

用户当前已在 Slurm 集群的登录节点或计算节点上，可以直接执行 Slurm 命令。

## 配置流程

**不进行追问，直接提示用户该 skill 支持的功能。**

```json
{
  "questions": [
    {
      "question": "检测到您已在 Slurm 集群上，请为这个集群命名（用于标识）",
      "options": ["使用默认名称（local）", "输入自定义名称"]
    }
  ]
}
```

## 保存配置

```bash
uv run python "$SCRIPT" init --mode local --cluster-name "名称"
```

## 提示可用功能

配置完成后，提示用户：

```
已配置为本地模式。您可以使用以下功能：

集群状态：
- status     查看资源状态
- node-info  查看节点详情
- find-gpu   查找 GPU 资源

作业管理：
- alloc      申请交互式资源
- release    释放资源
- run        srun 运行命令
- submit     提交作业
- jobs       查看作业状态
- log        查看作业日志
- cancel     取消作业
- history    作业历史

文件传输：
- upload     上传文件（本地复制）
- download   下载文件（本地复制）
```
