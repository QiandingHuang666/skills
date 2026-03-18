# Agent Skills for Graduate Students

为高校研究生日常学习与科研定制的 Claude Code Skills 集合。

## 简介

本项目包含一系列专门为高校研究生设计的 Claude Code Skills，旨在帮助学生更高效地完成科研计算、集群使用等日常任务。

## 包含的 Skills

### Slurm Assistant

Slurm HPC 集群助手，为高校学生/教师定制，支持本地（集群上）和远程（集群外）两种使用模式。

**功能特性：**

- 集群资源状态查看（分区、节点、GPU可用性）
- 交互式资源申请与释放
- 作业脚本生成与提交
- 作业状态监控与日志查看
- 作业取消与管理
- SSH 免密登录配置引导
- 分区硬件配置缓存（避免重复查询）
- GPU 资源快速查找

**适用场景：**

- 提交训练任务到 GPU 集群
- 查看集群资源空闲情况
- 申请交互式开发环境
- 管理运行中的作业
- 生成 Slurm 作业脚本

**使用示例：**

```
"查看集群状态"
"帮我提交一个 GPU 训练作业"
"申请一个 A100 GPU 节点"
"我的作业进度怎么样"
"生成一个 PyTorch 训练脚本"
```

## 安装

### 全局安装

将 skill 复制到 Claude Code 的全局 skills 目录：

```bash
cp -r slurm-assistant ~/.claude/skills/
```

### 项目级安装

将 skill 放在项目的 `.claude/skills/` 目录下：

```bash
mkdir -p your-project/.claude/skills
cp -r slurm-assistant your-project/.claude/skills/
```

## 目录结构

```
.
├── README.md
└── slurm-assistant/
    ├── SKILL.md                 # Skill 主文档
    ├── scripts/
    │   └── slurm-cli.py         # 核心命令行工具
    └── references/
        ├── job_templates.md     # 作业脚本模板
        └── common_errors.md     # 常见错误解决方案
```

## 技术栈

- **Python 3.x** - 核心脚本语言
- **SSH** - 远程集群连接
- **Slurm** - HPC 作业调度系统
- **uv/uvx** - 推荐的 Python 环境管理工具

## 兼容性

- Bash shell
- uv/uvx（优先）或 python3

## 预设集群

目前内置以下集群的预设配置：

- 贵州大学 HPC 集群

更多集群配置可通过交互式配置向导添加。

## 贡献

欢迎提交 Issue 和 Pull Request 来添加新功能或支持更多集群。

## 许可证

MIT License
