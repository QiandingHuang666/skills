# Slurm 作业脚本模板

本文档包含常用的 Slurm 作业脚本模板，可根据需要修改使用。

---

## 目录

1. [基础 Python 脚本 (uv 优先)](#1-基础-python-脚本-uv-优先)
2. [GPU 训练脚本](#2-gpu-训练脚本)
3. [多节点并行任务](#3-多节点并行任务)
4. [数组作业](#4-数组作业)
5. [交互式开发环境](#5-交互式开发环境)
6. [Conda 环境任务](#6-conda-环境任务)
7. [uv 环境管理](#7-uv-环境管理)

---

## 1. 基础 Python 脚本 (uv 优先)

**推荐使用 uv/uvx** 作为 Python 环境管理工具，比传统 conda/pip 更快更可靠。

### 使用 uvx 运行（单文件脚本，推荐）

```bash
#!/bin/bash
#SBATCH --job-name=python_job
#SBATCH --output=logs/%j.out
#SBATCH --error=logs/%j.err

cd $SLURM_SUBMIT_DIR

# 创建日志目录
mkdir -p logs

# 打印作业信息
echo "Job ID: $SLURM_JOB_ID"
echo "Running on: $(hostname)"
echo "Start time: $(date)"

# 使用 uvx 运行（自动管理依赖）
uvx --with numpy --with pandas python your_script.py

echo "End time: $(date)"
```

### 使用 uv run（项目环境，推荐）

```bash
#!/bin/bash
#SBATCH --job-name=uv_project
#SBATCH --output=logs/%j.out
#SBATCH --error=logs/%j.err

cd $SLURM_SUBMIT_DIR

# 创建日志目录
mkdir -p logs

echo "Job ID: $SLURM_JOB_ID"
echo "Running on: $(hostname)"
echo "Start time: $(date)"

# 确保项目有 pyproject.toml 或 requirements.txt
# uv 会自动创建虚拟环境并安装依赖
uv run python train.py --config config.yaml

echo "End time: $(date)"
```

### 使用 uv 同步环境后运行

```bash
#!/bin/bash
#SBATCH --job-name=uv_sync
#SBATCH --output=logs/%j.out
#SBATCH --error=logs/%j.err

cd $SLURM_SUBMIT_DIR

mkdir -p logs

# 同步依赖（首次运行或依赖变更时）
uv sync

# 运行脚本
uv run python your_script.py
```

### 传统方式（备选）

```bash
#!/bin/bash
#SBATCH --job-name=python_job
#SBATCH --output=python_%j.out
#SBATCH --error=python_%j.err

cd $SLURM_SUBMIT_DIR

# 加载模块（如果集群有）
module load python/3.9

# 打印作业信息
echo "Job ID: $SLURM_JOB_ID"
echo "Running on: $(hostname)"
echo "Start time: $(date)"

# 运行 Python 脚本
python your_script.py

echo "End time: $(date)"
```

---

## 2. GPU 训练脚本

```bash
#!/bin/bash
#SBATCH --job-name=gpu_train
#SBATCH --partition=gpu
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=4
#SBATCH --output=gpu_%j.out
#SBATCH --error=gpu_%j.err

cd $SLURM_SUBMIT_DIR

# 加载 CUDA 和 Python
module load cuda/11.8
module load python/3.9

# 显示 GPU 信息
echo "Available GPUs:"
nvidia-smi

echo "Job ID: $SLURM_JOB_ID"
echo "Start time: $(date)"

# 运行训练脚本
python train.py --config config.yaml

echo "End time: $(date)"
```

### 多 GPU 训练 (DataParallel)

```bash
#!/bin/bash
#SBATCH --job-name=multi_gpu
#SBATCH --partition=gpu
#SBATCH --gres=gpu:4
#SBATCH --cpus-per-task=8
#SBATCH --nodes=1
#SBATCH --output=multi_gpu_%j.out

cd $SLURM_SUBMIT_DIR

module load cuda/11.8 python/3.9

# 设置可见 GPU
export CUDA_VISIBLE_DEVICES=0,1,2,3

python train.py --gpus 4
```

### 分布式训练 (DistributedDataParallel)

```bash
#!/bin/bash
#SBATCH --job-name=ddp_train
#SBATCH --partition=gpu
#SBATCH --gres=gpu:4
#SBATCH --cpus-per-task=8
#SBATCH --nodes=2
#SBATCH --ntasks-per-node=4
#SBATCH --output=ddp_%j.out

cd $SLURM_SUBMIT_DIR

module load cuda/11.8 python/3.9

# 获取节点列表
NODES=$(scontrol show hostnames $SLURM_JOB_NODELIST)
MASTER_NODE=$(scontrol show hostnames $SLURM_JOB_NODELIST | head -n 1)

# 运行分布式训练
srun python -m torch.distributed.launch \
    --nnodes=$SLURM_JOB_NUM_NODES \
    --nproc_per_node=4 \
    --node_rank=$SLURM_NODEID \
    --master_addr=$MASTER_NODE \
    --master_port=29500 \
    train_ddp.py
```

---

## 3. 多节点并行任务

```bash
#!/bin/bash
#SBATCH --job-name=parallel
#SBATCH --nodes=4
#SBATCH --ntasks-per-node=16
#SBATCH --output=parallel_%j.out

cd $SLURM_SUBMIT_DIR

module load openmpi/4.1

# 运行 MPI 程序
srun -n 64 mpirun ./parallel_program

# 或者直接使用 srun
srun ./mpi_program
```

---

## 4. 数组作业

### 基础数组作业

```bash
#!/bin/bash
#SBATCH --job-name=array_job
#SBATCH --array=1-100
#SBATCH --output=array_%A_%a.out
#SBATCH --error=array_%A_%a.err

cd $SLURM_SUBMIT_DIR

# SLURM_ARRAY_TASK_ID 即为当前任务的数组索引
TASK_ID=$SLURM_ARRAY_TASK_ID

echo "Running task $TASK_ID"
python process_data.py --input data_${TASK_ID}.txt --output result_${TASK_ID}.txt
```

### 带参数列表的数组作业

```bash
#!/bin/bash
#SBATCH --job-name=param_array
#SBATCH --array=0-9
#SBATCH --output=param_%A_%a.out

cd $SLURM_SUBMIT_DIR

# 参数列表
PARAMS=(0.001 0.01 0.1 1 10 100 1000 0.005 0.05 0.5)

# 获取当前参数
LR=${PARAMS[$SLURM_ARRAY_TASK_ID]}

echo "Training with learning rate: $LR"
python train.py --lr $LR --output model_lr_${LR}
```

### 并发限制的数组作业

```bash
#!/bin/bash
#SBATCH --job-name=limited_array
#SBATCH --array=1-100%10  # 最多同时运行 10 个
#SBATCH --output=limited_%A_%a.out

python process.py --id $SLURM_ARRAY_TASK_ID
```

---

## 5. 交互式开发环境

### Jupyter Lab

```bash
#!/bin/bash
#SBATCH --job-name=jupyter
#SBATCH --partition=gpu
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=4
#SBATCH --output=jupyter_%j.out

cd $SLURM_SUBMIT_DIR

module load python/3.9 cuda/11.8

# 获取节点名称
NODE=$(hostname -s)

# 生成随机端口
PORT=$(shuf -i 8000-9999 -n 1)

echo "Jupyter Lab running on: http://${NODE}:${PORT}"
echo "SSH tunnel command:"
echo "  ssh -N -L ${PORT}:${NODE}:${PORT} ${USER}@login.cluster.edu"

# 启动 Jupyter Lab
jupyter lab --no-browser --port=${PORT} --ip=0.0.0.0
```

### TensorBoard

```bash
#!/bin/bash
#SBATCH --job-name=tensorboard
#SBATCH --output=tensorboard_%j.out

cd $SLURM_SUBMIT_DIR

NODE=$(hostname -s)
PORT=$(shuf -i 6006-6999 -n 1)

echo "TensorBoard running on: http://${NODE}:${PORT}"
echo "SSH tunnel command:"
echo "  ssh -N -L ${PORT}:${NODE}:${PORT} ${USER}@login.cluster.edu"

tensorboard --logdir=./logs --port=${PORT} --bind_all
```

---

## 6. Conda 环境任务

```bash
#!/bin/bash
#SBATCH --job-name=conda_job
#SBATCH --output=conda_%j.out

cd $SLURM_SUBMIT_DIR

# 初始化 conda
source ~/.bashrc
conda activate my_env

# 或者直接指定 conda 路径
# source /path/to/conda/etc/profile.d/conda.sh
# conda activate my_env

echo "Conda environment: $CONDA_DEFAULT_ENV"
python script.py
```

---

## 常用 SBATCH 参数速查

| 参数 | 说明 | 示例 |
|------|------|------|
| `--job-name` | 作业名称 | `--job-name=my_job` |
| `--partition` | 分区 | `--partition=gpu` |
| `--nodes` | 节点数 | `--nodes=2` |
| `--ntasks` | 总任务数 | `--ntasks=16` |
| `--ntasks-per-node` | 每节点任务数 | `--ntasks-per-node=8` |
| `--cpus-per-task` | 每任务 CPU 数 | `--cpus-per-task=4` |
| `--gres` | 通用资源 | `--gres=gpu:2` |
| `--mem` | 内存 | `--mem=32G` |
| `--time` | 时间限制 | `--time=24:00:00` |
| `--output` | 标准输出 | `--output=%j.out` |
| `--error` | 标准错误 | `--error=%j.err` |
| `--array` | 数组作业 | `--array=1-100` |
| `--mail-type` | 邮件类型 | `--mail-type=ALL` |
| `--mail-user` | 邮箱 | `--mail-user=user@email.com` |
| `--dependency` | 依赖关系 | `--dependency=afterok:12345` |

---

## 环境变量

在作业脚本中可以使用以下 Slurm 环境变量：

| 变量 | 说明 |
|------|------|
| `SLURM_JOB_ID` | 作业 ID |
| `SLURM_JOB_NAME` | 作业名称 |
| `SLURM_SUBMIT_DIR` | 提交目录 |
| `SLURM_SUBMIT_HOST` | 提交主机 |
| `SLURM_JOB_NODELIST` | 分配的节点列表 |
| `SLURM_JOB_NUM_NODES` | 节点数 |
| `SLURM_NTASKS` | 总任务数 |
| `SLURM_CPUS_PER_TASK` | 每任务 CPU 数 |
| `SLURM_ARRAY_TASK_ID` | 数组任务 ID |
| `SLURM_ARRAY_JOB_ID` | 数组作业 ID |

---

## 7. uv 环境管理

**uv 是推荐的 Python 环境管理工具**，比 conda/pip 更快更可靠。

### uv 常用命令

| 命令 | 说明 |
|------|------|
| `uv init` | 初始化新项目 |
| `uv add <package>` | 添加依赖 |
| `uv remove <package>` | 移除依赖 |
| `uv sync` | 同步依赖到虚拟环境 |
| `uv run <cmd>` | 在项目环境中运行命令 |
| `uvx <tool>` | 运行一次性工具（无需安装） |

### 使用 pyproject.toml 的项目

```bash
#!/bin/bash
#SBATCH --job-name=uv_project
#SBATCH --output=uv_%j.out

cd $SLURM_SUBMIT_DIR

# 项目结构示例:
# .
# ├── pyproject.toml
# ├── uv.lock
# ├── src/
# │   └── train.py
# └── data/

# 运行训练
uv run python src/train.py --epochs 100
```

### 使用 requirements.txt

```bash
#!/bin/bash
#SBATCH --job-name=uv_req
#SBATCH --output=uv_req_%j.out

cd $SLURM_SUBMIT_DIR

# uv 会自动从 requirements.txt 安装
uv run --with-requirements requirements.txt python script.py
```

### GPU 项目 + uv

```bash
#!/bin/bash
#SBATCH --job-name=uv_gpu
#SBATCH --partition=gpu
#SBATCH --gres=gpu:1
#SBATCH --cpus-per-task=4
#SBATCH --output=uv_gpu_%j.out

cd $SLURM_SUBMIT_DIR

# 确保 pyproject.toml 包含 torch 等依赖
# [project.dependencies]
# torch = ">=2.0"
# numpy = ">=1.24"

echo "Job ID: $SLURM_JOB_ID"
echo "GPU: $CUDA_VISIBLE_DEVICES"

# 显示 GPU 信息
nvidia-smi

# 运行训练
uv run python train.py

echo "End time: $(date)"
```

### 使用 uvx 运行工具

```bash
#!/bin/bash
#SBATCH --job-name=uvx_job
#SBATCH --output=uvx_%j.out

cd $SLURM_SUBMIT_DIR

# 运行单次任务，无需项目环境
# uvx 会自动下载并运行工具

# 数据处理
uvx --with pandas python process_data.py

# 运行 jupyter (一次性)
uvx --from jupyter jupyter lab --no-browser --port=8888

# 运行特定版本的工具
uvx ruff@0.1.0 check .
```

### 多节点 + uv

```bash
#!/bin/bash
#SBATCH --job-name=uv_ddp
#SBATCH --partition=gpu
#SBATCH --gres=gpu:4
#SBATCH --nodes=2
#SBATCH --ntasks-per-node=4
#SBATCH --output=uv_ddp_%j.out

cd $SLURM_SUBMIT_DIR

# 获取 master 节点
MASTER=$(scontrol show hostnames $SLURM_JOB_NODELIST | head -n 1)

# 分布式训练
srun uv run python -m torch.distributed.launch \
    --nnodes=$SLURM_JOB_NUM_NODES \
    --nproc_per_node=4 \
    --master_addr=$MASTER \
    --master_port=29500 \
    train_ddp.py
```

### 数组作业 + uv

```bash
#!/bin/bash
#SBATCH --job-name=uv_array
#SBATCH --array=1-100%10
#SBATCH --output=uv_array_%A_%a.out

cd $SLURM_SUBMIT_DIR

# 每个数组任务使用相同的项目环境
uv run python process.py --id $SLURM_ARRAY_TASK_ID
```

### uv 环境初始化脚本

```bash
#!/bin/bash
#SBATCH --job-name=uv_setup
#SBATCH --output=uv_setup_%j.out

cd $SLURM_SUBMIT_DIR

# 首次设置项目
if [ ! -f "pyproject.toml" ]; then
    uv init --name my_project
fi

# 添加常用依赖
uv add numpy pandas scikit-learn matplotlib

# 如果是 GPU 项目
uv add torch --index-url https://download.pytorch.org/whl/cu118

# 同步环境
uv sync

echo "uv 环境设置完成"
uv run python -c "import torch; print(f'PyTorch: {torch.__version__}')"
```

### 性能提示

1. **使用 uv.lock** - 提交 `uv.lock` 文件确保可复现性
2. **缓存虚拟环境** - uv 的 `.venv` 可以在多次作业间复用
3. **预同步** - 首次运行前执行 `uv sync` 加速后续作业
