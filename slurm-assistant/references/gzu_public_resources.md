# 贵州大学 HPC 公共资源检查

贵州大学 HPC 集群特有功能：公共资源检查流程。

---

## 重要说明

当用户请求下载数据集或安装软件时，AI 必须先检查 `/home/share/Official/` 公共目录！

许多常用的数据集、模型、软件工具可能已经在公共目录中存在。重复下载会：
- 浪费存储空间和带宽
- 增加等待时间
- 可能违反集群使用规定

---

## AI 处理流程

### 1. 识别场景

当用户提到以下关键词时触发检查：
- "下载 dataset"、"下载数据集"、"download dataset"
- "安装软件"、"install"、"下载模型"
- 具体的数据集名称（如 ImageNet、COCO、LLaMA 等）

### 2. 执行检查

```bash
cargo run --quiet --bin slurm-client -- exec --connection <connection_id> --cmd 'ls -lh /home/share/Official/'
```

### 3. 搜索相关资源（如果有具体名称）

```bash
cargo run --quiet --bin slurm-client -- exec --connection <connection_id> --cmd 'find /home/share/Official/ -iname "*关键字*" 2>/dev/null | head -20'
```

### 4. 处理结果

- **找到资源**：告知用户可以直接使用，提供软链接命令
- **未找到**：继续执行用户的下载/安装请求

---

## LaTeX 安装

### 识别场景

当用户提到以下关键词时触发 LaTeX 检查和安装引导：
- "安装 latex"、"安装 LaTeX"、"install latex"
- "安装 texlive"、"安装 TeX Live"
- "pdflatex"、"xelatex"、"lualatex" 命令找不到
- 编译时报错："latex command not found"

### LaTeX 安装流程

**重要：贵州大学 HPC 集群已提供 TeX Live 安装脚本！**

1. **检查 LaTeX 是否已安装**

```bash
cargo run --quiet --bin slurm-client -- exec --connection <connection_id> --cmd "which pdflatex"
```

2. **如果未安装，引导用户使用集群提供的安装脚本**

```bash
cargo run --quiet --bin slurm-client -- exec --connection <connection_id> --cmd "sh /home/share/Official/tools/texlive/install.sh"
```

**安装说明：**
- 安装脚本位于：`/home/share/Official/tools/texlive/install.sh`
- 安装过程可能需要几分钟
- 安装后需要重新加载环境变量或重新登录

3. **验证安装**

```bash
cargo run --quiet --bin slurm-client -- exec --connection <connection_id> --cmd "which pdflatex && pdflatex --version"
```

### 常见 LaTeX 编译命令

```bash
# PDFLaTeX（英文文档）
pdflatex document.tex

# XeLaTeX（中文文档，推荐）
xelatex document.tex

# LuaLaTeX（高级功能）
lualatex document.tex
```

---

## 示例对话

**用户：** "帮我下载 ImageNet 数据集"

**AI 应该：**
1. 先检查：`cargo run --quiet --bin slurm-client -- exec --connection <connection_id> --cmd 'find /home/share/Official/ -iname "*imagenet*" 2>/dev/null'`
2. 如果找到：告知用户 "公共目录已有 ImageNet，无需下载，可以使用软链接直接使用"
3. 如果未找到：才执行下载操作

**用户：** "编译时报错 pdflatex not found"

**AI 应该：**
1. 检查 LaTeX 是否安装
2. 引导用户执行：`sh /home/share/Official/tools/texlive/install.sh`
3. 安装完成后重新编译

---

## 相关文档

**用户：** "帮我下载 ImageNet 数据集"

**AI 应该：**
1. 先检查：`cargo run --quiet --bin slurm-client -- exec --connection <connection_id> --cmd 'find /home/share/Official/ -iname "*imagenet*" 2>/dev/null'`
2. 如果找到：告知用户 "公共目录已有 ImageNet，无需下载，可以使用软链接直接使用"
3. 如果未找到：才执行下载操作

---

## 相关文档

- `references/use_gzu.md` - 贵州大学 HPC 配置
