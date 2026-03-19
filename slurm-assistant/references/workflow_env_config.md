# 环境配置流程

在 Slurm 集群上配置开发环境的规范流程。

---

## 推荐方案

| 工具 | 负责内容 | 使用场景 |
|------|---------|---------|
| **uv（默认）** | Python 包管理、工具安装 | 大部分场景 |
| **conda（按需）** | CUDA、C 编译器、底层系统库 | 节点配置不符合需求时 |

**核心原则：**
- **优先使用 uv**：集群节点已配置 CUDA、编译器等环境，uv 安装 Python 包即可
- **按需使用 conda**：只有当项目要求的 CUDA 版本、编译器版本与节点配置不符时，才需要 conda

---

## 环境配置流程

### 第一步：检查集群环境

```bash
# 检查节点 CUDA 版本
uv run python "$SCRIPT" exec -c "nvidia-smi | grep 'CUDA Version'"

# 检查 CUDA 运行时库
uv run python "$SCRIPT" exec -c "nvcc --version"

# 检查 Python 版本
uv run python "$SCRIPT" exec -c "python --version"

# 检查编译器
uv run python "$SCRIPT" exec -c "gcc --version"

# 检查 conda 是否可用
uv run python "$SCRIPT" exec -c "which conda"

# 检查 uv 是否可用
uv run python "$SCRIPT" exec -c "which uv"
```

### 第二步：询问项目需求

使用 `AskUserQuestion` 收集需求：

```json
{
  "questions": [
    {
      "question": "请描述您的项目需求",
      "options": [
        "深度学习项目（使用 GPU）",
        "科学计算（需要数值库）",
        "普通 Python 项目",
        "需要特定 CUDA 版本",
        "需要特定编译器/系统库",
        "克隆已有项目"
      ]
    }
  ]
}
```

### 第三步：判断是否需要 conda

根据用户需求和集群环境对比，判断是否需要 conda：

| 用户需求 | 节点已有环境 | 需要conda？ | 推荐方案 |
|---------|-------------|-----------|---------|
| 深度学习 | CUDA 12.x | 否 | uv + torch（预编译版） |
| 深度学习 | CUDA 11.x | 否 | uv + torch（预编译版） |
| 深度学习 | 需要 CUDA 11.8，节点是 12.x | 是 | conda 安装 CUDA 11.8 |
| 科学计算 | gcc 11+ | 否 | uv 安装包 |
| 科学计算 | 需要 gcc 9，节点是 13 | 是 | conda 安装 gcc 9 |
| 普通Python | 任意 | 否 | uv 即可 |

### 第四步：配置环境

#### 场景 A：不需要 conda（大部分场景）

**示例：深度学习项目使用节点 CUDA**

```bash
# 1. 创建项目目录
uv run python "$SCRIPT" exec -c "mkdir -p ~/project && cd ~/project"

# 2. 使用 uv 初始化项目
uv run python "$SCRIPT" exec -c "cd ~/project && uv venv"

# 3. 安装 PyTorch（使用预编译版，自动检测节点 CUDA）
uv run python "$SCRIPT" exec -c "cd ~/project && uv pip install torch torchvision"

# 4. 验证 CUDA 可用
uv run python "$SCRIPT" exec -c "cd ~/project && uv run python -c 'import torch; print(f\"CUDA available: {torch.cuda.is_available()}\")'"
```

**作业脚本：**
```bash
#!/bin/bash
#SBATCH --job-name=training
#SBATCH --partition=gpu
#SBATCH --gres=gpu:1

cd $SLURM_SUBMIT_DIR

# 直接使用 uv，无需激活环境
uv run python train.py
```

#### 场景 B：需要 conda（特殊版本需求）

**示例：需要特定 CUDA 版本**

```bash
# 1. 配置 conda 通道（重要！）
uv run python "$SCRIPT" exec -c "conda config --add channels conda-forge"
uv run python "$SCRIPT" exec -c "conda config --add channels nvidia"
uv run python "$SCRIPT" exec -c "conda config --add channels pytorch"
uv run python "$SCRIPT" exec -c "conda config --set channel_priority strict"

# 2. 创建 conda 环境并安装 CUDA toolkit
uv run python "$SCRIPT" exec -c "conda create -n myproject python=3.10 cuda-toolkit=11.8 -c nvidia -y"

# 3. 配置环境变量（见下方"conda 环境配置细节"）

# 4. 使用 uv 安装 Python 包（匹配 CUDA 版本）
uv run python "$SCRIPT" exec -c "conda activate myproject && uv pip install torch torchvision --index-url https://download.pytorch.org/whl/cu118"
```

---

## conda 环境配置细节

当需要使用 conda 时，必须正确配置底层环境。

### CUDA 环境配置

**第一步：配置 conda 通道**

```bash
# 1. 设置优先级通道（按优先级排序）
uv run python "$SCRIPT" exec -c "conda config --add channels conda-forge"
uv run python "$SCRIPT" exec -c "conda config --add channels nvidia"
uv run python "$SCRIPT" exec -c "conda config --add channels pytorch"

# 2. 设置通道优先级（conda-forge 最高）
uv run python "$SCRIPT" exec -c "conda config --set channel_priority strict"

# 3. 验证通道配置
uv run python "$SCRIPT" exec -c "conda config --show channels"
```

**第二步：安装 CUDA toolkit**

```bash
# 方案 A：安装完整 CUDA toolkit（推荐，包含 nvcc）
# 使用 nvidia 通道
uv run python "$SCRIPT" exec -c "conda create -n myenv python=3.10 cuda-toolkit=11.8 -c nvidia -y"

# 方案 B：安装 cuda-runtime（轻量，不含 nvcc）
uv run python "$SCRIPT" exec -c "conda create -n myenv python=3.10 cuda-runtime=11.8 -c nvidia -y"

# 方案 C：使用 conda-forge（可能有更好的兼容性）
uv run python "$SCRIPT" exec -c "conda create -n myenv python=3.10 cuda-toolkit=11.8 -c conda-forge -y"
```

**第三步：配置环境变量**

```bash
# 1. 激活 conda 环境
uv run python "$SCRIPT" exec -c "source ~/.bashrc && conda activate myenv"

# 2. 设置 CUDA_HOME
uv run python "$SCRIPT" exec -c "conda activate myenv && export CUDA_HOME=\$CONDA_PREFIX"

# 3. 添加 nvcc 到 PATH
uv run python "$SCRIPT" exec -c "conda activate myenv && export PATH=\$CUDA_HOME/bin:\$PATH"

# 4. 设置动态链接库路径
uv run python "$SCRIPT" exec -c "conda activate myenv && export LD_LIBRARY_PATH=\$CUDA_HOME/lib:\$LD_LIBRARY_PATH"

# 5. 持久化配置
uv run python "$SCRIPT" exec -c "echo 'export CUDA_HOME=\$CONDA_PREFIX' >> ~/.bashrc"
uv run python "$SCRIPT" exec -c "echo 'export PATH=\$CUDA_HOME/bin:\$PATH' >> ~/.bashrc"
uv run python "$SCRIPT" exec -c "echo 'export LD_LIBRARY_PATH=\$CUDA_HOME/lib:\$LD_LIBRARY_PATH' >> ~/.bashrc"
```

**第四步：验证 CUDA 配置**

```bash
# 1. 验证 CUDA_HOME
uv run python "$SCRIPT" exec -c "conda activate myenv && echo \$CUDA_HOME"

# 2. 验证 nvcc
uv run python "$SCRIPT" exec -c "conda activate myenv && which nvcc"
uv run python "$SCRIPT" exec -c "conda activate myenv && nvcc --version"

# 3. 验证 CUDA 运行时库
uv run python "$SCRIPT" exec -c "conda activate myenv && python -c 'import torch; print(f\"CUDA available: {torch.cuda.is_available()}\")'"

# 4. 验证动态链接库
uv run python "$SCRIPT" exec -c "conda activate myenv && ldd \$CONDA_PREFIX/lib/python3.*/site-packages/torch/lib/libtorch_cuda.so | grep cuda"
```

### 动态链接库配置

```bash
# 1. 查找 conda 环境中的 CUDA 库
uv run python "$SCRIPT" exec -c "conda activate myenv && find \$CONDA_PREFIX -name 'libcudart.so*'"

# 2. 列出所有可能的库路径
uv run python "$SCRIPT" exec -c "conda activate myenv && ls -la \$CONDA_PREFIX/lib/ | grep cuda"

# 3. 添加所有相关路径到 LD_LIBRARY_PATH
uv run python "$SCRIPT" exec -c "conda activate myenv && export LD_LIBRARY_PATH=\$CONDA_PREFIX/lib:\$CONDA_PREFIX/lib/python3.*/site-packages/nvidia/cuda_runtime/lib:\$LD_LIBRARY_PATH"

# 4. 持久化配置（在 ~/.bashrc 中添加）
uv run python "$SCRIPT" exec -c "echo 'export LD_LIBRARY_PATH=\$CONDA_PREFIX/lib:\$LD_LIBRARY_PATH' >> ~/.bashrc"

# 5. 验证库可被找到
uv run python "$SCRIPT" exec -c "conda activate myenv && ldd \$CONDA_PREFIX/lib/python3.*/site-packages/torch/lib/libtorch_cuda.so | grep 'not found'"
```

**如果库仍然找不到：**

```bash
# 1. 检查具体缺少哪个库
uv run python "$SCRIPT" exec -c "conda activate myenv && python -c 'import torch; torch.cuda.is_available()'"

# 2. 查找该库在 conda 环境中的位置
uv run python "$SCRIPT" exec -c "conda activate myenv && find \$CONDA_PREFIX -name '缺失的库名.so*'"

# 3. 将该库所在目录添加到 LD_LIBRARY_PATH
uv run python "$SCRIPT" exec -c "conda activate myenv && export LD_LIBRARY_PATH=\$(dirname \$(find \$CONDA_PREFIX -name '缺失的库名.so*')):\$LD_LIBRARY_PATH"
```

### 编译器配置

```bash
# 1. 安装特定版本的 gcc
uv run python "$SCRIPT" exec -c "conda install -n myenv gcc_linux-64=9 -c conda-forge"

# 2. 验证编译器
uv run python "$SCRIPT" exec -c "conda activate myenv && gcc --version"

# 3. 配置编译器路径
uv run python "$SCRIPT" exec -c "conda activate myenv && export CC=\$CONDA_PREFIX/bin/gcc"
```

### 完整配置脚本

```bash
#!/bin/bash
# conda 环境 CUDA 完整配置脚本

# 第一步：配置 conda 通道
echo "配置 conda 通道..."
conda config --add channels conda-forge
conda config --add channels nvidia
conda config --add channels pytorch
conda config --set channel_priority strict
conda config --show channels

# 第二步：激活环境
source ~/.bashrc
conda activate myenv

# 第三步：配置 CUDA 环境变量
echo "配置 CUDA 环境变量..."
export CUDA_HOME=$CONDA_PREFIX
export PATH=$CUDA_HOME/bin:$PATH
export LD_LIBRARY_PATH=$CUDA_HOME/lib:$LD_LIBRARY_PATH
export LD_LIBRARY_PATH=$CONDA_PREFIX/lib/python3.*/site-packages/nvidia/cuda_runtime/lib:$LD_LIBRARY_PATH

# 第四步：配置编译器
export CC=$CONDA_PREFIX/bin/gcc
export CXX=$CONDA_PREFIX/bin/g++

# 第五步：验证配置
echo "=== 验证配置 ==="
echo "CUDA_HOME: $CUDA_HOME"
echo "PATH: $PATH | tr ':' '\n' | grep cuda"
echo "LD_LIBRARY_PATH: $LD_LIBRARY_PATH | tr ':' '\n' | grep cuda"

echo "=== NVCC 版本 ==="
nvcc --version

echo "=== GCC 版本 ==="
gcc --version

echo "=== CUDA 库检查 ==="
find $CONDA_PREFIX -name "libcudart.so*" -type f
find $CONDA_PREFIX -name "libcuda.so*" -type f

echo "=== Python CUDA 验证 ==="
python -c "import torch; print(f'PyTorch CUDA available: {torch.cuda.is_available()}'); print(f'PyTorch CUDA version: {torch.version.cuda}')"

# 第六步：持久化配置（添加到 ~/.bashrc）
echo "持久化配置到 ~/.bashrc..."
{
    echo "# Conda CUDA 环境 - myenv"
    echo "export CUDA_HOME=\$CONDA_PREFIX"
    echo "export PATH=\$CUDA_HOME/bin:\$PATH"
    echo "export LD_LIBRARY_PATH=\$CUDA_HOME/lib:\$LD_LIBRARY_PATH"
} >> ~/.bashrc

echo "配置完成！请运行 'source ~/.bashrc' 使配置生效。"
```

---

## 工具缺失处理

### uv 缺失

```bash
# 官方安装脚本（推荐）
uv run python "$SCRIPT" exec -c "curl -LsSf https://astral.sh/uv/install.sh | sh"

# 安装后验证
uv run python "$SCRIPT" exec -c "source ~/.bashrc && uv --version"
```

### conda 缺失

只有确定需要 conda 时才安装：

```json
{
  "questions": [
    {
      "question": "检测到需要特定 CUDA/编译器版本，但 conda 不可用。是否安装？",
      "options": ["安装 Miniconda", "调整项目以使用节点环境"]
    }
  ]
}
```

```bash
# 安装 Miniconda
uv run python "$SCRIPT" exec -c "wget https://repo.anaconda.com/miniconda/Miniconda3-latest-Linux-x86_64.sh -O ~/miniconda.sh"
uv run python "$SCRIPT" exec -c "bash ~/miniconda.sh -b -p ~/miniconda"
uv run python "$SCRIPT" exec -c "rm ~/miniconda.sh"
uv run python "$SCRIPT" exec -c "~/miniconda/bin/conda init bash"
uv run python "$SCRIPT" exec -c "source ~/.bashrc"
```

---

## 常见场景

### 场景 1：新建深度学习项目（使用节点 CUDA）

```bash
# 1. 检查节点 CUDA 版本
uv run python "$SCRIPT" exec -c "nvidia-smi | grep 'CUDA Version'"
# 输出：CUDA Version: 12.2

# 2. 用户接受使用节点 CUDA，使用 uv 配置
uv run python "$SCRIPT" exec -c "mkdir -p ~/project && cd ~/project && uv venv"
uv run python "$SCRIPT" exec -c "cd ~/project && uv pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu121"

# 3. 验证
uv run python "$SCRIPT" exec -c "cd ~/project && uv run python -c 'import torch; print(torch.cuda.is_available())'"
```

### 场景 2：需要特定 CUDA 版本

用户："项目需要 CUDA 11.8"

```bash
# 1. 检查节点 CUDA
uv run python "$SCRIPT" exec -c "nvidia-smi | grep 'CUDA Version'"
# 输出：CUDA Version: 12.2

# 2. 配置 conda 通道
uv run python "$SCRIPT" exec -c "conda config --add channels conda-forge"
uv run python "$SCRIPT" exec -c "conda config --add channels nvidia"
uv run python "$SCRIPT" exec -c "conda config --add channels pytorch"
uv run python "$SCRIPT" exec -c "conda config --set channel_priority strict"

# 3. 需要使用 conda
uv run python "$SCRIPT" exec -c "conda create -n cuda118 python=3.10 cuda-toolkit=11.8 -c nvidia -y"

# 4. 配置环境
uv run python "$SCRIPT" exec -c "conda activate cuda118 && export CUDA_HOME=\$CONDA_PREFIX"
uv run python "$SCRIPT" exec -c "conda activate cuda118 && export PATH=\$CUDA_HOME/bin:\$PATH"
uv run python "$SCRIPT" exec -c "conda activate cuda118 && export LD_LIBRARY_PATH=\$CUDA_HOME/lib:\$LD_LIBRARY_PATH"

# 5. 持久化配置
uv run python "$SCRIPT" exec -c "echo 'export CUDA_HOME=\$CONDA_PREFIX' >> ~/.bashrc"
uv run python "$SCRIPT" exec -c "echo 'export PATH=\$CUDA_HOME/bin:\$PATH' >> ~/.bashrc"
uv run python "$SCRIPT" exec -c "echo 'export LD_LIBRARY_PATH=\$CUDA_HOME/lib:\$LD_LIBRARY_PATH' >> ~/.bashrc"

# 6. 安装 PyTorch
uv run python "$SCRIPT" exec -c "conda activate cuda118 && uv pip install torch torchvision --index-url https://download.pytorch.org/whl/cu118"

# 7. 验证
uv run python "$SCRIPT" exec -c "conda activate cuda118 && nvcc --version"
uv run python "$SCRIPT" exec -c "conda activate cuda118 && python -c 'import torch; print(f\"CUDA available: {torch.cuda.is_available()}, Version: {torch.version.cuda}\")'"
```

### 场景 3：克隆已有项目

```bash
# 1. 克隆项目
uv run python "$SCRIPT" exec -c "git clone https://github.com/user/repo ~/repo"

# 2. 检查依赖文件
uv run python "$SCRIPT" exec -c "cat ~/repo/requirements.txt"
uv run python "$SCRIPT" exec -c "cat ~/repo/environment.yml"

# 3. 如果有 environment.yml，检查是否需要特殊 CUDA/编译器
#    如果不需要特殊版本，只用 uv 即可
uv run python "$SCRIPT" exec -c "cd ~/repo && uv venv"
uv run python "$SCRIPT" exec -c "cd ~/repo && uv pip install -r requirements.txt"

# 4. 如果有 environment.yml 且需要特殊版本，使用 conda
uv run python "$SCRIPT" exec -c "conda env create -f ~/repo/environment.yml"
```

---

## 环境验证清单

### uv 环境（无 conda）

```bash
# 1. Python 版本
uv run python "$SCRIPT" exec -c "cd ~/project && uv run python --version"

# 2. 包列表
uv run python "$SCRIPT" exec -c "cd ~/project && uv pip list"

# 3. CUDA 可用性（深度学习）
uv run python "$SCRIPT" exec -c "cd ~/project && uv run python -c 'import torch; print(f\"CUDA: {torch.cuda.is_available()}, Version: {torch.version.cuda}\")'"

# 4. 验证使用的是节点 CUDA
uv run python "$SCRIPT" exec -c "nvidia-smi --query-gpu=driver_version,cuda_version --format=csv"
```

### conda 环境

```bash
# 1. Python 版本
uv run python "$SCRIPT" exec -c "conda activate myenv && python --version"

# 2. CUDA 配置
uv run python "$SCRIPT" exec -c "conda activate myenv && echo \$CUDA_HOME"
uv run python "$SCRIPT" exec -c "conda activate myenv && nvcc --version"

# 3. 动态链接库
uv run python "$SCRIPT" exec -c "conda activate myenv && ldd \$CONDA_PREFIX/lib/python3.*/site-packages/torch/lib/libtorch_cuda.so"

# 4. 包列表
uv run python "$SCRIPT" exec -c "conda activate myenv && uv pip list"
```

---

## 故障排除

### 问题 1：CUDA 版本不匹配

```bash
# 检查节点 CUDA 版本
uv run python "$SCRIPT" exec -c "nvidia-smi | grep 'CUDA Version'"

# 检查 PyTorch 编译的 CUDA 版本
uv run python "$SCRIPT" exec -c "python -c 'import torch; print(torch.version.cuda)'"

# 解决方案：使用匹配的 PyTorch 版本或使用 conda 安装特定 CUDA
```

### 问题 2：动态链接库找不到

```bash
# 检查 LD_LIBRARY_PATH
uv run python "$SCRIPT" exec -c "echo \$LD_LIBRARY_PATH"

# 查找缺失的库
uv run python "$SCRIPT" exec -c "find ~ -name 'libcudart.so*'"

# 添加路径
uv run python "$SCRIPT" exec -c "export LD_LIBRARY_PATH=/path/to/cuda/lib:\$LD_LIBRARY_PATH"
```

### 问题 3：nvcc 未找到

```bash
# 检查 CUDA_HOME
uv run python "$SCRIPT" exec -c "echo \$CUDA_HOME"

# 设置 CUDA_HOME
uv run python "$SCRIPT" exec -c "export CUDA_HOME=/path/to/cuda"

# 添加到 PATH
uv run python "$SCRIPT" exec -c "export PATH=\$CUDA_HOME/bin:\$PATH"
```

---

## 最佳实践

1. **优先使用 uv**：大部分情况节点环境已足够，无需 conda
2. **按需使用 conda**：只在版本不匹配时使用
3. **验证配置**：使用 conda 后务必验证 CUDA_HOME、LD_LIBRARY_PATH、nvcc
4. **记录依赖**：
   - uv 项目：`uv pip freeze > requirements.txt`
   - conda 项目：`conda env export > environment.yml`
5. **定期清理**：`conda env remove -n old_env`

---

## 相关文档

- `references/workflow_job.md` - 作业脚本生成流程
- `references/commands.md` - 命令详细用法
