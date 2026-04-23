# 生成作业脚本流程

用户请求生成作业脚本时的完整处理流程。用户要求提交作业时，若未指定具体提交方式时，默认先写作业脚本在提交作业。

---

## 第一步：收集基本信息

使用 `AskUserQuestion` 工具收集以下信息：

```json
{
  "questions": [
    {
      "question": "请选择作业分区",
      "options": ["cpu", "gpu-a100", "gpu-v100", "其他（请说明）"]
    },
    {
      "question": "需要 GPU 资源吗？",
      "options": ["不需要", "需要 (1卡)", "需要 (多卡，请说明数量)"]
    },
    {
      "question": "预计运行时间？",
      "options": ["1小时", "4小时", "24小时", "其他（请说明）"]
    }
  ]
}
```

约束：

- `mem` 与 `time` 不是默认必填项
- 只有用户明确提出时，才在脚本中添加 `#SBATCH --mem=...` 或 `#SBATCH --time=...`

---

## 第二步：询问虚拟环境配置（必须）

这是关键步骤。这里讨论的是“用户作业脚本如何运行自己的 Python 环境”，不是 `slurm-assistant` 自身的运行时依赖。

必须询问用户需要使用哪种虚拟环境：

```json
{
  "questions": [
    {
      "question": "请选择用户作业的 Python 环境管理方式",
      "options": [
        "uv（推荐，快速现代）",
        "conda（传统方式）",
        "conda + uv（conda 管理 CUDA，uv 管理 Python 包）",
        "不需要（使用系统 Python）"
      ]
    }
  ]
}
```

---

## 第三步：根据回答生成激活语句

根据用户选择，在用户的作业脚本中添加相应的激活语句：

| 用户选择 | 激活语句 |
|---------|---------|
| **uv（推荐）** | `# 用户脚本通过 uv 运行，无需激活<br>uv run python your_script.py` |
| **conda** | `source ~/.bashrc<br>conda activate your_env_name` |
| **conda + uv** | `# 先激活 conda（获取 CUDA）<br>source ~/.bashrc<br>conda activate base<br># 用户脚本通过 uv 运行<br>uv run python your_script.py` |
| **不需要** | `# 用户脚本直接使用系统 Python<br>module load python/3.9<br>python your_script.py` |

---

## 第四步：生成完整脚本

结合收集的信息，生成完整的作业脚本。

注意：

- `submit`、`jobs`、`log` 这些集群操作由 Rust `slurm-client` 负责
- 脚本里的 `python` / `uv run python` 只是用户真正的训练、推理或数据处理命令

### 示例：用户选择 uv + GPU

```bash
#!/bin/bash
#SBATCH --job-name=training
#SBATCH --partition=gpu-a100
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=8
#SBATCH --output=logs/%j.out
#SBATCH --error=logs/%j.err

cd $SLURM_SUBMIT_DIR

# 创建日志目录
mkdir -p logs

# 显示作业信息
echo "Job ID: $SLURM_JOB_ID"
echo "Node: $(hostname)"
echo "GPUs: $CUDA_VISIBLE_DEVICES"

# 显示 GPU 信息
nvidia-smi

# 运行训练（uv 方式，无需激活虚拟环境）
uv run python train.py --config config.yaml --epochs 100

echo "Job completed at: $(date)"
```

### 示例：用户选择 conda + uv

```bash
#!/bin/bash
#SBATCH --job-name=training
#SBATCH --partition=gpu-a100
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=8
#SBATCH --output=logs/%j.out
#SBATCH --error=logs/%j.err

cd $SLURM_SUBMIT_DIR

# 创建日志目录
mkdir -p logs

# 显示作业信息
echo "Job ID: $SLURM_JOB_ID"

# 激活 conda 环境（获取 CUDA 支持）
source ~/.bashrc
conda activate cuda_env

# 使用 uv 运行（管理 Python 包）
uv run python train.py --config config.yaml

echo "Job completed at: $(date)"
```

---

## 第五步：提交作业

生成脚本后，询问用户是否立即提交：

```json
{
  "questions": [
    {
      "question": "脚本已生成，是否立即提交作业？",
      "options": ["立即提交", "先查看脚本内容", "稍后手动提交"]
    }
  ]
}
```

如果用户选择立即提交，执行：

```bash
slurm-client submit --connection <connection_id> script.sh
```

---

## 相关文档

- `references/job_templates.md` - 更多作业脚本模板
- `references/common_errors.md` - 常见错误处理
