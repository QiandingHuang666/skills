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
uv run python "$SCRIPT" exec -c 'ls -lh /home/share/Official/'
```

### 3. 搜索相关资源（如果有具体名称）

```bash
uv run python "$SCRIPT" exec -c 'find /home/share/Official/ -iname "*关键字*" 2>/dev/null | head -20'
```

### 4. 处理结果

- **找到资源**：告知用户可以直接使用，提供软链接命令
- **未找到**：继续执行用户的下载/安装请求

---

## 示例对话

**用户：** "帮我下载 ImageNet 数据集"

**AI 应该：**
1. 先检查：`uv run python "$SCRIPT" exec -c 'find /home/share/Official/ -iname "*imagenet*" 2>/dev/null'`
2. 如果找到：告知用户 "公共目录已有 ImageNet，无需下载，可以使用软链接直接使用"
3. 如果未找到：才执行下载操作

---

## 相关文档

- `references/use_gzu.md` - 贵州大学 HPC 配置
