#!/usr/bin/env python3
"""
Slurm Cluster Assistant CLI
跨平台支持：Windows/macOS/Linux
"""

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple

# ============================================================================
# 动态路径检测（支持多 Agent CLI）
# ============================================================================

def get_skill_dir() -> Path:
    """自动检测 skill 目录位置（基于脚本自身位置）"""
    script_path = Path(__file__).resolve()
    # 脚本在 scripts/ 子目录下，skill 目录是其父目录
    return script_path.parent.parent


def get_config_dir() -> Path:
    """获取配置目录（优先使用已存在的配置，其次 XDG 标准路径）"""
    skill_dir = get_skill_dir()

    # 1. 如果 skill 目录下已有配置文件，继续使用（便携式/向后兼容）
    skill_config = skill_dir / "config.json"
    if skill_config.exists():
        return skill_dir

    # 2. 使用 XDG 标准路径（配置与代码分离）
    xdg_config = os.environ.get("XDG_CONFIG_HOME")
    if xdg_config:
        return Path(xdg_config) / "slurm-assistant"
    return Path.home() / ".config" / "slurm-assistant"


def get_settings_file() -> Path:
    """获取 Agent settings.json 路径（支持多 Agent CLI）"""
    candidates = [
        Path.home() / ".claude" / "settings.json",      # Claude Code
        Path.home() / ".openclaw" / "settings.json",    # OpenCLAW
        Path.home() / ".codex" / "settings.json",       # Codex CLI
    ]
    for p in candidates:
        if p.parent.exists():
            return p
    # 默认返回第一个（兼容现有行为）
    return candidates[0]


# 全局路径（动态计算）
SKILL_DIR = get_skill_dir()
CONFIG_DIR = get_config_dir()
CONFIG_FILE = CONFIG_DIR / "config.json"
JOBS_FILE = CONFIG_DIR / "jobs.json"
SETTINGS_FILE = get_settings_file()


# ============================================================================
# Agent CLI 检测
# ============================================================================

def detect_current_agent_name() -> str:
    """检测当前 Agent CLI 名称"""
    # 按优先级检测
    if (Path.home() / ".claude").exists():
        return "claude-code"
    elif (Path.home() / ".codex").exists():
        return "codex"
    elif (Path.home() / ".openclaw").exists():
        return "openclaw"
    return "unknown"


class Colors:
    """终端颜色"""
    RED = '\033[91m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    BLUE = '\033[94m'
    RESET = '\033[0m'

    @classmethod
    def disable(cls):
        """禁用颜色（Windows 兼容）"""
        cls.RED = cls.GREEN = cls.YELLOW = cls.BLUE = cls.RESET = ''


# Windows 下禁用颜色
if platform.system() == "Windows":
    Colors.disable()


def print_info(msg: str):
    print(f"{Colors.BLUE}[INFO]{Colors.RESET} {msg}")


def print_success(msg: str):
    print(f"{Colors.GREEN}[OK]{Colors.RESET} {msg}")


def print_warning(msg: str):
    print(f"{Colors.YELLOW}[WARN]{Colors.RESET} {msg}")


def print_error(msg: str):
    print(f"{Colors.RED}[ERROR]{Colors.RESET} {msg}", file=sys.stderr)


def die(msg: str):
    print_error(msg)
    sys.exit(1)


class ConfigManager:
    """配置管理器"""

    def __init__(self):
        self.config: Dict[str, Any] = {}
        self._load()

    def _load(self):
        """加载配置"""
        if CONFIG_FILE.exists():
            try:
                self.config = json.loads(CONFIG_FILE.read_text())
            except json.JSONDecodeError:
                self.config = {}

    def save(self):
        """保存配置"""
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        CONFIG_FILE.write_text(json.dumps(self.config, indent=2, ensure_ascii=False))

    def is_configured(self) -> bool:
        return "mode" in self.config

    def get_mode(self) -> str:
        return self.config.get("mode", "local")

    def get_cluster_info(self) -> Dict[str, Any]:
        return self.config.get("cluster", {})

    def get_authorized_agents(self) -> List[str]:
        """获取已授权的 Agent 列表"""
        return self.config.get("authorized_agents", [])

    def is_current_agent_authorized(self) -> bool:
        """检查当前 Agent 是否已授权"""
        agent_name = detect_current_agent_name()
        return agent_name in self.get_authorized_agents()

    def authorize_current_agent(self):
        """授权当前 Agent"""
        agent_name = detect_current_agent_name()
        authorized = self.get_authorized_agents()
        if agent_name not in authorized:
            authorized.append(agent_name)
            self.config["authorized_agents"] = authorized
            self._save()

    def unauthorize_agent(self, agent_name: str = None):
        """取消指定 Agent 的授权（默认当前 Agent）"""
        if agent_name is None:
            agent_name = detect_current_agent_name()
        authorized = self.get_authorized_agents()
        if agent_name in authorized:
            authorized.remove(agent_name)
            self.config["authorized_agents"] = authorized
            self._save()


class SlurmExecutor:
    """Slurm 执行器"""

    def __init__(self, config: ConfigManager):
        self.config = config
        self._setup_ssh_opts()

    def _setup_ssh_opts(self):
        """设置 SSH 选项（跨平台）"""
        self.ssh_opts = [
            "-o", "StrictHostKeyChecking=accept-new",  # 安全：只接受新主机密钥
            "-o", "ConnectTimeout=10"
        ]

        # macOS/Linux 使用 ControlMaster 复用连接
        if platform.system() in ("Darwin", "Linux"):
            socket_dir = Path.home() / ".ssh" / "sockets"
            socket_dir.mkdir(parents=True, exist_ok=True)
            cluster = self.config.get_cluster_info()
            host = cluster.get("host", "")
            port = cluster.get("port", 22)
            username = cluster.get("username", "")
            socket_path = socket_dir / f"slurm-{username}@{host}:{port}"
            self.ssh_opts.extend([
                "-o", "ControlMaster=auto",
                "-o", f"ControlPath={socket_path}",
                "-o", "ControlPersist=600"
            ])

    def run(self, cmd: str) -> str:
        """在集群上执行命令"""
        mode = self.config.get_mode()

        # Windows 下不显示命令行窗口
        creation_flags = 0
        if platform.system() == "Windows":
            try:
                creation_flags = subprocess.CREATE_NO_WINDOW
            except AttributeError:
                pass

        if mode == "local":
            result = subprocess.run(
                cmd,
                shell=True,
                capture_output=True,
                text=True,
                creationflags=creation_flags
            )
            return result.stdout
        else:
            cluster = self.config.get_cluster_info()
            host = cluster.get("host", "")
            port = cluster.get("port", 22)
            username = cluster.get("username", "")
            jump_host = cluster.get("jump_host", "")

            if not host or not username:
                die("Remote mode requires host and username configuration")

            ssh_cmd = ["ssh", "-p", str(port)] + self.ssh_opts.copy()

            if jump_host:
                ssh_cmd.extend(["-J", jump_host])

            ssh_cmd.append(f"{username}@{host}")
            ssh_cmd.append(cmd)

            # Windows 下需要使用 shell=True
            use_shell = platform.system() == "Windows"
            result = subprocess.run(
                ssh_cmd,
                capture_output=True,
                text=True,
                shell=use_shell,
                creationflags=creation_flags if not use_shell else 0
            )
            return result.stdout

    def transfer(self, src: str, dst: str, download: bool = False, recursive: bool = False) -> bool:
        """使用 scp 传输文件"""
        mode = self.config.get_mode()

        # Windows 下不显示命令行窗口
        creation_flags = 0
        if platform.system() == "Windows":
            try:
                creation_flags = subprocess.CREATE_NO_WINDOW
            except AttributeError:
                pass

        if mode == "local":
            try:
                if recursive:
                    if Path(dst).exists():
                        shutil.copytree(src, dst, dirs_exist_ok=True)
                    else:
                        shutil.copytree(src, dst)
                else:
                    shutil.copy2(src, dst)
                print_success(f"Copy completed: {src} -> {dst}")
                return True
            except Exception as e:
                print_error(f"Copy failed: {e}")
                return False

        cluster = self.config.get_cluster_info()
        host = cluster.get("host", "")
        port = cluster.get("port", 22)
        username = cluster.get("username", "")
        jump_host = cluster.get("jump_host", "")

        if not host or not username:
            die("Remote mode requires host and username configuration")

        scp_cmd = ["scp", "-P", str(port), "-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=30"]

        if jump_host:
            scp_cmd.extend(["-J", jump_host])

        if recursive:
            scp_cmd.append("-r")

        remote_prefix = f"{username}@{host}:"
        if download:
            if not src.startswith(remote_prefix) and ":" not in src:
                src = remote_prefix + src
        else:
            if not dst.startswith(remote_prefix) and ":" not in dst:
                dst = remote_prefix + dst

        scp_cmd.extend([src, dst])
        print_info(f"Transferring: {' '.join(scp_cmd)}")

        try:
            result = subprocess.run(
                scp_cmd,
                capture_output=True,
                text=True,
                creationflags=creation_flags
            )
            if result.returncode == 0:
                print_success(f"Transfer completed: {src} -> {dst}")
                return True
            else:
                print_error(f"Transfer failed: {result.stderr}")
                return False
        except Exception as e:
            print_error(f"Transfer error: {e}")
            return False

    def check_remote_exists(self, remote_path: str) -> Tuple[bool, str]:
        """
        检查远程路径是否存在
        返回: (是否存在, 类型: 'file'/'dir'/'not_found')
        """
        # 展开波浪号
        test_path = remote_path
        if remote_path.startswith('~/'):
            test_path = '$HOME/' + remote_path[2:]
        elif remote_path == '~':
            test_path = '$HOME'

        test_cmd = f"test -f {test_path} && echo 'file' || (test -d {test_path} && echo 'dir' || echo 'not_found')"
        output = self.run(test_cmd).strip()

        if output == 'file':
            return True, 'file'
        elif output == 'dir':
            return True, 'dir'
        else:
            return False, 'not_found'


def parse_gpu_gres(gres_str: str) -> Tuple[int, str]:
    """解析 GRES 字符串，

    支持格式：
    - gpu:a100:4 (带型号)
    - gpu:4 (不带型号)
    - gpu:a100:4(SFabric) (带拓扑信息)

    返回 (gpu数量, gpu型号)
    """
    # 优先匹配 gpu:type:N 格式（型号不含数字开头）
    match = re.search(r'gpu:([a-zA-Z_]\w*):(\d+)', gres_str.lower())
    if match:
        return int(match.group(2)), match.group(1)

    # 其次匹配 gpu:N 格式（纯数字）
    match = re.search(r'gpu:(\d+)', gres_str.lower())
    if match:
        return int(match.group(1)), "unknown"

    return 0, ""


def parse_cpu_alloc(alloc_str: str) -> Tuple[int, int, int, int]:
    """解析 CPU 分配字符串 A/I/O/T，返回 (allocated, idle, other, total)"""
    parts = alloc_str.split('/')
    if len(parts) == 4:
        return int(parts[0]), int(parts[1]), int(parts[2]), int(parts[3])
    return 0, 0, 0, 0


# ============================================================================
# 资源检查数据结构
# ============================================================================

@dataclass
class NodeResourceInfo:
    """节点资源信息"""
    node_name: str
    partition: str
    gpu_total: int
    gpu_idle: int
    gpu_type: str
    cpu_total: int
    cpu_idle: int
    mem_total: int  # MB
    cpus_per_gpu: int  # = cpu_total // max(gpu_total, 1)


@dataclass
class ResourceCheckResult:
    """资源检查结果"""
    has_available: bool
    available_nodes: List[NodeResourceInfo]  # 按空闲资源排序
    best_node: Optional[NodeResourceInfo]
    recommended_cpus: int
    wait_estimate: Optional[str]
    message: str


def _get_nodes_info_from_scontrol(executor, partition: Optional[str] = None) -> List[NodeResourceInfo]:
    """
    使用 scontrol show node 获取节点信息（准确的 GPU 分配信息）

    返回 NodeResourceInfo 列表（排除 DRAIN 状态节点）
    """
    output = executor.run("scontrol show node")

    nodes = []
    current_node = None
    current_gres = None
    current_alloc_tres = None
    current_partition = None
    current_cpu_alloc = None
    current_cpu_total = None
    current_mem = None
    current_state = None

    for line in output.splitlines():
        line = line.strip()

        # NodeName 行，开始新节点
        if line.startswith('NodeName='):
            # 保存上一个节点的信息
            if current_node and current_state:
                # 跳过 DRAIN 状态节点
                if 'DRAIN' in current_state.upper():
                    # 重置并跳过
                    current_node = None
                    current_state = None
                else:
                    # 解析 GPU 信息
                    gpu_total, gpu_type = parse_gpu_gres(current_gres) if current_gres else (0, '')

                    # 从 AllocTRES 解析已分配 GPU
                    gpu_alloc = 0
                    if current_alloc_tres:
                        match = re.search(r'gres/gpu=(\d+)', current_alloc_tres)
                        if match:
                            gpu_alloc = int(match.group(1))

                    # 解析 CPU 信息
                    cpu_a = int(current_cpu_alloc) if current_cpu_alloc else 0
                    cpu_t = int(current_cpu_total) if current_cpu_total else 0
                    cpu_i = cpu_t - cpu_a

                    # 计算 cpus_per_gpu
                    cpus_per_gpu = cpu_t // max(gpu_total, 1) if gpu_total > 0 else cpu_t

                    nodes.append(NodeResourceInfo(
                        node_name=current_node,
                        partition=current_partition or 'unknown',
                        gpu_total=gpu_total,
                        gpu_idle=max(0, gpu_total - gpu_alloc),
                        gpu_type=gpu_type.upper() if gpu_type else 'N/A',
                        cpu_total=cpu_t,
                        cpu_idle=cpu_i,
                        mem_total=int(current_mem) if current_mem and current_mem.isdigit() else 0,
                        cpus_per_gpu=cpus_per_gpu
                    ))

            # 解析新节点
            match = re.search(r'NodeName=(\S+)', line)
            current_node = match.group(1) if match else None
            current_gres = None
            current_alloc_tres = None
            current_partition = None
            current_cpu_alloc = None
            current_cpu_total = None
            current_mem = None
            current_state = None

        # State 行
        elif line.startswith('State='):
            current_state = line.split('=', 1)[1]

        # Gres 行
        elif line.startswith('Gres='):
            current_gres = line.split('=', 1)[1]

        # Partitions 行
        elif line.startswith('Partitions='):
            current_partition = line.split('=', 1)[1]

        # AllocTRES 行
        elif line.startswith('AllocTRES='):
            current_alloc_tres = line.split('=', 1)[1]

        # CPUAlloc 行
        elif line.startswith('CPUAlloc='):
            match = re.search(r'CPUAlloc=(\d+)', line)
            if match:
                current_cpu_alloc = match.group(1)

        # CPUTot 行
        elif 'CPUTot=' in line:
            match = re.search(r'CPUTot=(\d+)', line)
            if match:
                current_cpu_total = match.group(1)

        # RealMemory 行
        elif line.startswith('RealMemory='):
            match = re.search(r'RealMemory=(\d+)', line)
            if match:
                current_mem = match.group(1)

    # 处理最后一个节点
    if current_node and current_state:
        # 跳过 DRAIN 状态节点
        if 'DRAIN' not in current_state.upper():
            gpu_total, gpu_type = parse_gpu_gres(current_gres) if current_gres else (0, '')
            gpu_alloc = 0
            if current_alloc_tres:
                match = re.search(r'gres/gpu=(\d+)', current_alloc_tres)
                if match:
                    gpu_alloc = int(match.group(1))

            cpu_a = int(current_cpu_alloc) if current_cpu_alloc else 0
            cpu_t = int(current_cpu_total) if current_cpu_total else 0
            cpu_i = cpu_t - cpu_a

            cpus_per_gpu = cpu_t // max(gpu_total, 1) if gpu_total > 0 else cpu_t

            nodes.append(NodeResourceInfo(
                node_name=current_node,
                partition=current_partition or 'unknown',
                gpu_total=gpu_total,
                gpu_idle=max(0, gpu_total - gpu_alloc),
                gpu_type=gpu_type.upper() if gpu_type else 'N/A',
                cpu_total=cpu_t,
                cpu_idle=cpu_i,
                mem_total=int(current_mem) if current_mem and current_mem.isdigit() else 0,
                cpus_per_gpu=cpus_per_gpu
            ))

    # 过滤分区
    if partition:
        nodes = [n for n in nodes if partition in n.partition]

    return nodes


def _get_node_gpu_usage(executor, node_name: str) -> int:
    """查询节点上已使用的 GPU 数量（已废弃，保留兼容性）"""
    return 0


def _estimate_wait_time(executor, partition: str) -> Optional[str]:
    """估算排队等待时间"""
    try:
        output = executor.run(f"squeue -p {partition} -t PENDING -h -o '%i'")
        pending_count = len([l for l in output.splitlines() if l.strip()])

        if pending_count == 0:
            return "可能很快（无排队作业）"
        elif pending_count < 5:
            return f"预计 {pending_count * 5}-{pending_count * 15} 分钟"
        else:
            return f"预计较长时间（{pending_count} 个作业排队中）"
    except Exception:
        return None


def check_partition_resources(
    executor,
    partition: str,
    gpu_count: int = 0,
    gpu_type: Optional[str] = None,
    min_cpus: int = 1
) -> ResourceCheckResult:
    """
    检查分区资源可用性（使用 scontrol show node 获取精确 GPU 分配信息）

    参数:
        executor: Slurm 执行器
        partition: 分区名称
        gpu_count: 需要的 GPU 数量（0 表示纯 CPU）
        gpu_type: GPU 型号要求（可选，如 'a100', 'v100'）
        min_cpus: 最少需要的 CPU 数量

    返回:
        ResourceCheckResult 包含资源检查结果和建议
    """
    # 1. 使用 scontrol 获取节点信息
    try:
        nodes_info = _get_nodes_info_from_scontrol(executor, partition)
    except Exception as e:
        return ResourceCheckResult(
            has_available=False,
            available_nodes=[],
            best_node=None,
            recommended_cpus=1,
            wait_estimate=None,
            message=f"查询分区信息失败: {e}"
        )

    if not nodes_info:
        return ResourceCheckResult(
            has_available=False,
            available_nodes=[],
            best_node=None,
            recommended_cpus=1,
            wait_estimate=None,
            message=f"分区 {partition} 没有找到符合条件的节点"
        )

    # 2. 如果指定了 GPU 型号，过滤不符合的节点
    if gpu_type:
        nodes_info = [n for n in nodes_info if gpu_type.lower() in n.gpu_type.lower()]

    # 3. 筛选满足条件的可用节点
    available_nodes = []
    for node in nodes_info:
        if gpu_count > 0:
            # GPU 请求：节点空闲 GPU >= 请求 GPU 数量，且有空闲 CPU
            if node.gpu_idle >= gpu_count and node.cpu_idle >= min_cpus:
                available_nodes.append(node)
        else:
            # 纯 CPU 请求（GPU 节点也可用）
            if node.cpu_idle >= min_cpus:
                available_nodes.append(node)

    # 4. 按空闲资源排序（优先选择空闲资源最充足的节点）
    if gpu_count > 0:
        # GPU 请求：按空闲 GPU 数量降序，再按空闲 CPU 降序
        available_nodes.sort(key=lambda n: (n.gpu_idle, n.cpu_idle), reverse=True)
    else:
        # CPU 请求：按空闲 CPU 降序
        available_nodes.sort(key=lambda n: n.cpu_idle, reverse=True)

    # 5. 计算推荐的 CPU 数量
    best_node = available_nodes[0] if available_nodes else None
    if best_node:
        if gpu_count > 0:
            # GPU 节点：推荐 CPU = min(空闲CPU, 每GPU对应CPU * 请求GPU数)
            recommended_cpus = min(
                best_node.cpu_idle,
                best_node.cpus_per_gpu * gpu_count
            )
        else:
            # CPU 节点：使用空闲 CPU
            recommended_cpus = best_node.cpu_idle
        recommended_cpus = max(1, recommended_cpus)
    else:
        recommended_cpus = 1

    # 6. 生成消息
    if available_nodes:
        message = f"找到 {len(available_nodes)} 个可用节点"
    elif nodes_info:
        # 有节点但资源不足
        if gpu_count > 0:
            total_gpu_idle = sum(n.gpu_idle for n in nodes_info)
            if total_gpu_idle < gpu_count:
                message = f"资源不足：分区共有 {total_gpu_idle} 张空闲 GPU，但请求 {gpu_count} 张"
            else:
                message = f"资源不足：空闲 GPU 分散在多个节点，无单个节点满足 {gpu_count} 张 GPU 的需求"
        else:
            max_idle = max(n.cpu_idle for n in nodes_info)
            message = f"资源不足：节点最大空闲 CPU 为 {max_idle}，但请求 {min_cpus}"
    else:
        message = f"分区 {partition} 没有符合条件的节点"

    # 7. 估算等待时间（仅当资源不足时）
    wait_estimate = None
    if not available_nodes:
        wait_estimate = _estimate_wait_time(executor, partition)

    return ResourceCheckResult(
        has_available=len(available_nodes) > 0,
        available_nodes=available_nodes,
        best_node=best_node,
        recommended_cpus=recommended_cpus,
        wait_estimate=wait_estimate,
        message=message
    )


# ============================================================================
# 命令: init
# ============================================================================

def check_ssh_key_exists() -> bool:
    """检查 SSH 密钥是否存在（跨平台）"""
    ssh_dir = Path.home() / ".ssh"
    if not ssh_dir.exists():
        return False

    # 检查常见的私钥文件
    key_files = ["id_ed25519", "id_rsa", "id_ecdsa", "id_dsa"]
    for key_file in key_files:
        if (ssh_dir / key_file).exists():
            return True

    # Windows 特殊检查
    if platform.system() == "Windows":
        # 检查用户目录直接下的密钥文件（某些 SSH 客户端的位置）
        for key_file in key_files:
            if (Path.home() / key_file).exists():
                return True
        # 检查 .ppk 文件（PuTTY 密钥）
        for key_file in key_files:
            if (ssh_dir / f"{key_file}.ppk").exists():
                return True

    return False


def validate_node_name(node_name: str) -> bool:
    """
    验证节点名是否安全（防止命令注入）
    只允许字母、数字、连字符、下划线和点
    """
    import re
    pattern = r'^[a-zA-Z0-9._-]+$'
    return bool(re.match(pattern, node_name))


def check_local_slurm() -> bool:
    """检查本地是否有 Slurm（跨平台）"""
    try:
        # 检查 sinfo 命令是否可用
        result = subprocess.run(
            ["sinfo", "--version"],
            capture_output=True,
            creationflags=subprocess.CREATE_NO_WINDOW if platform.system() == "Windows" else 0
        )
        return result.returncode == 0
    except FileNotFoundError:
        return False
    except Exception:
        return False


def check_ssh_connection(host: str, port: int, username: str, jump_host: str = "", retry: bool = True) -> Tuple[bool, str]:
    """
    检查 SSH 连接和免密登录（跨平台）
    返回: (是否成功, 错误信息)

    参数:
        retry: 是否在失败时重试一次（默认 True）
    """
    # Windows 上需要更长的超时时间
    timeout = 15 if platform.system() == "Windows" else 10

    # 首先检查 SSH 是否可用
    if platform.system() == "Windows":
        try:
            # Windows 上先测试 ssh 命令是否可用
            test_result = subprocess.run(
                ["ssh", "-V"],
                capture_output=True,
                creationflags=subprocess.CREATE_NO_WINDOW
            )
            if test_result.returncode != 0:
                return False, "SSH command not found or not working"
        except Exception:
            return False, "SSH command not available"

    ssh_cmd = ["ssh", "-p", str(port), f"-o", f"ConnectTimeout={timeout}", "-o", "BatchMode=yes"]

    if jump_host:
        ssh_cmd.extend(["-J", jump_host])

    ssh_cmd.append(f"{username}@{host}")
    ssh_cmd.append("echo ok")

    max_attempts = 2 if retry else 1

    for attempt in range(max_attempts):
        try:
            # Windows 下尝试不使用 shell，避免参数解析问题
            use_shell = platform.system() == "Windows" and False

            result = subprocess.run(
                ssh_cmd,
                capture_output=True,
                text=True,
                shell=use_shell,
                creationflags=subprocess.CREATE_NO_WINDOW if platform.system() == "Windows" and not use_shell else 0
            )

            if result.returncode == 0 and "ok" in result.stdout:
                return True, ""
            elif "Permission denied" in result.stderr or "publickey" in result.stderr:
                return False, "SSH passwordless login not configured"
            elif "Could not resolve hostname" in result.stderr:
                return False, "Cannot resolve hostname"
            elif "Connection refused" in result.stderr:
                return False, "Connection refused, check port"
            elif "Connection timed out" in result.stderr or "timed out" in result.stderr.lower():
                # 重试超时
                if attempt < max_attempts - 1:
                    continue
                return False, f"Connection timeout after {timeout}s"
            else:
                # 如果是第一次失败且允许重试，再试一次
                if attempt < max_attempts - 1 and "ok" not in result.stdout:
                    continue
                # 返回详细的错误信息用于调试
                stderr = result.stderr.strip() if result.stderr else ""
                stdout = result.stdout.strip() if result.stdout else ""
                error_details = stderr or stdout or "Unknown error"
                return False, f"Connection failed: {error_details}"
        except FileNotFoundError:
            return False, "SSH client not found"
        except Exception as e:
            if attempt < max_attempts - 1:
                continue
            return False, str(e)

    return False, "Connection failed after retry"


def cmd_init(args):
    """初始化配置"""
    config = ConfigManager()

    # 处理授权/取消授权
    if args.authorize:
        config.authorize_current_agent()
        print_success(f"Agent '{detect_current_agent_name()}' authorized")
        return

    if args.unauthorize:
        config.unauthorize_agent()
        print_info(f"Agent '{detect_current_agent_name()}' authorization revoked")
        return

    if args.check:
        # 配置检查模式
        result = {
            "configured": config.is_configured(),
            "local_slurm_available": False,
            "ssh_key_configured": False,
            "ssh_connection_ok": False,
            "config_valid": False,
            "current_agent": detect_current_agent_name(),
            "authorized_agents": config.get_authorized_agents(),
            "current_agent_authorized": config.is_current_agent_authorized()
        }

        # 检查本地 Slurm
        result["local_slurm_available"] = check_local_slurm()

        # 检查 SSH 密钥
        result["ssh_key_configured"] = check_ssh_key_exists()

        if config.is_configured():
            mode = config.get_mode()
            cluster = config.get_cluster_info()

            if mode == "remote":
                host = cluster.get("host", "")
                port = cluster.get("port", 22)
                username = cluster.get("username", "")
                jump_host = cluster.get("jump_host", "")

                if host and username:
                    # 检查 SSH 连接（除非使用 --fast 模式）
                    if getattr(args, 'fast', False):
                        # 快速模式：跳过 SSH 连接测试
                        result["ssh_connection_ok"] = True  # 假设连接正常
                        result["ssh_connection_skipped"] = True
                        result["config_valid"] = True
                    else:
                        ssh_ok, ssh_error = check_ssh_connection(host, port, username, jump_host)
                        result["ssh_connection_ok"] = ssh_ok
                        result["ssh_error"] = ssh_error if not ssh_ok else ""
                        result["config_valid"] = ssh_ok
                else:
                    result["config_valid"] = False
                    result["config_error"] = "缺少 host 或 username"
            else:
                # 本地模式
                result["config_valid"] = result["local_slurm_available"]
                if not result["local_slurm_available"]:
                    result["config_error"] = "本地未检测到 Slurm"
        else:
            result["config_valid"] = False
            result["config_error"] = "未配置"

        if args.output_json:
            print(json.dumps(result, ensure_ascii=False))
        else:
            # 人性化输出
            if config.is_configured():
                print_success("Config: loaded")

                mode = config.get_mode()
                cluster = config.get_cluster_info()

                if mode == "local":
                    if result["local_slurm_available"]:
                        print_success("Local Slurm: available")
                    else:
                        print_warning("Local Slurm: not available")
                else:
                    print_info(f"Cluster: {cluster.get('name', 'unknown')}")
                    print_info(f"Address: {cluster.get('username', 'unknown')}@{cluster.get('host', 'unknown')}:{cluster.get('port', 22)}")

                    if result["ssh_connection_ok"]:
                        print_success("SSH: connected (passwordless configured)")
                    else:
                        print_warning(f"SSH: failed ({result.get('ssh_error', 'unknown error')})")
            else:
                print_warning("Config: not configured")

                if result["local_slurm_available"]:
                    print_info("Local Slurm detected, can use local mode")
                else:
                    print_info("Local Slurm not detected")

                if result["ssh_key_configured"]:
                    print_info("SSH key: configured")
                else:
                    print_warning("SSH key: not configured")
        return

    if not args.mode:
        die("Must specify --mode (local or remote)")

    if args.mode == "local":
        config.config = {
            "mode": "local",
            "cluster": {"name": args.cluster_name or "local"}
        }
        print_info("Mode: local")
    else:
        if not args.host or not args.username:
            die("Remote mode requires --host and --username")

        config.config = {
            "mode": "remote",
            "cluster": {
                "name": args.cluster_name or "remote",
                "host": args.host,
                "port": args.port or 22,
                "username": args.username,
                "jump_host": args.jump_host or ""
            }
        }
        print_info("Mode: remote")
        print_info(f"Cluster: {args.cluster_name or 'remote'}")
        print_info(f"Address: {args.username}@{args.host}:{args.port or 22}")
        if args.jump_host:
            print_info(f"Jump host: {args.jump_host}")

    config.save()
    print_success(f"Configuration saved to {CONFIG_FILE}")

    if args.mode == "remote":
        print_info("Checking SSH connection...")

        # 先检查 SSH 密钥
        if not check_ssh_key_exists():
            print_warning("SSH key not detected, passwordless login may not be configured")
            print_info("Reference: https://docs.github.com/en/authentication/connecting-to-github-with-ssh/generating-a-new-ssh-key-and-adding-it-to-the-ssh-agent")

        executor = SlurmExecutor(config)
        try:
            result = executor.run("echo 'Connection Successful'")
            if "Connection Successful" in result:
                print_success("SSH connection successful")
            else:
                print_warning("SSH connection failed, please configure passwordless login")
                print_info("Run the following command to configure passwordless login:")
                print(f"  ssh-copy-id -p {args.port or 22} {args.username}@{args.host}")
        except Exception as e:
            print_warning(f"SSH connection failed: {e}")
            print_info("Please check:")
            print_info("  1. Host address is correct")
            print_info("  2. Passwordless login is configured")
            print_info("  3. Network connection is normal")
    else:
        # 本地模式检查 Slurm
        if not check_local_slurm():
            print_warning("Local Slurm commands not detected")
            print_info("Please ensure:")
            print_info("  1. You are on the Slurm cluster login node")
            print_info("  2. Slurm commands (sinfo, squeue, etc.) are available")
        else:
            print_success("Local Slurm detected")


# ============================================================================
# 命令: status
# ============================================================================

def cmd_status(args):
    """查看资源状态"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    executor = SlurmExecutor(config)

    if args.gpu:
        _show_gpu_status(executor, args.partition)
    elif args.nodes:
        cmd = "sinfo -N -o '%P %N %C %G %m'"
        if args.partition:
            cmd += f" -p {args.partition}"
        output = executor.run(cmd)
        print(output)
    else:
        cmd = "sinfo -o '%P %A %C %m %D'"
        if args.partition:
            cmd += f" -p {args.partition}"
        output = executor.run(cmd)
        print(output)


def _show_gpu_status(executor: SlurmExecutor, partition: Optional[str] = None):
    """显示 GPU 节点状态（使用 scontrol show node 获取精确 GPU 分配信息）"""
    # 使用 scontrol show node 获取节点信息
    output = executor.run("scontrol show node")

    # 解析 scontrol 输出
    # 格式：
    # NodeName=gpu-a40-1 Arch=x86_64 CoresPerSocket=1
    #    Gres=gpu:a40:2
    #    Partitions=gpu-a40
    #    AllocTRES=cpu=16,gres/gpu=2

    gpu_nodes = []
    current_node = None
    current_gres = None
    current_alloc_tres = None
    current_partition = None

    for line in output.splitlines():
        line = line.strip()

        # NodeName 行，开始新节点
        if line.startswith('NodeName='):
            # 保存上一个节点的信息
            if current_node and current_gres and 'gpu' in current_gres.lower():
                gpu_total, gpu_type = parse_gpu_gres(current_gres)
                if gpu_total > 0:
                    # 从 AllocTRES 解析已分配 GPU
                    gpu_alloc = 0
                    if current_alloc_tres:
                        match = re.search(r'gres/gpu=(\d+)', current_alloc_tres)
                        if match:
                            gpu_alloc = int(match.group(1))

                    gpu_idle = max(0, gpu_total - gpu_alloc)
                    gpu_nodes.append({
                        'node': current_node,
                        'partition': current_partition or 'unknown',
                        'gpu_type': gpu_type.upper() if gpu_type else 'GPU',
                        'gpu_total': gpu_total,
                        'gpu_idle': gpu_idle,
                        'gpu_alloc': gpu_alloc
                    })

            # 解析新节点的 NodeName
            match = re.search(r'NodeName=(\S+)', line)
            current_node = match.group(1) if match else None
            current_gres = None
            current_alloc_tres = None
            current_partition = None

        # Gres 行
        elif line.startswith('Gres='):
            current_gres = line.split('=', 1)[1]

        # Partitions 行
        elif line.startswith('Partitions='):
            current_partition = line.split('=', 1)[1]

        # AllocTRES 行
        elif line.startswith('AllocTRES='):
            current_alloc_tres = line.split('=', 1)[1]

    # 处理最后一个节点
    if current_node and current_gres and 'gpu' in current_gres.lower():
        gpu_total, gpu_type = parse_gpu_gres(current_gres)
        if gpu_total > 0:
            gpu_alloc = 0
            if current_alloc_tres:
                match = re.search(r'gres/gpu=(\d+)', current_alloc_tres)
                if match:
                    gpu_alloc = int(match.group(1))

            gpu_idle = max(0, gpu_total - gpu_alloc)
            gpu_nodes.append({
                'node': current_node,
                'partition': current_partition or 'unknown',
                'gpu_type': gpu_type.upper() if gpu_type else 'GPU',
                'gpu_total': gpu_total,
                'gpu_idle': gpu_idle,
                'gpu_alloc': gpu_alloc
            })

    if not gpu_nodes:
        print_info("No GPU nodes found")
        return

    # 按分区和节点名排序
    gpu_nodes.sort(key=lambda x: (x['partition'], x['node']))

    # 如果指定了分区，过滤
    if partition:
        gpu_nodes = [n for n in gpu_nodes if partition in n['partition']]

    if not gpu_nodes:
        print_info(f"No GPU nodes found in partition {partition}")
        return

    print(f"{'Node':<20} {'Partition':<15} {'GPU Idle/Total':<15} {'GPU Type'}")
    print("-" * 75)

    for node in gpu_nodes:
        print(f"{node['node']:<20} {node['partition']:<15} "
              f"{node['gpu_idle']}/{node['gpu_total']:<13} "
              f"{node['gpu_type']}")


# ============================================================================
# 命令: node-info
# ============================================================================

def cmd_node_info(args):
    """查看节点详情"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.node:
        die("Must specify node name")

    executor = SlurmExecutor(config)
    output = executor.run(f"scontrol show node {args.node}")
    print(output)


# ============================================================================
# 命令: node-jobs
# ============================================================================

def cmd_node_jobs(args):
    """查看节点上的作业"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.node:
        die("Must specify node name")

    executor = SlurmExecutor(config)
    
    running_output = executor.run(
        f"squeue -w {args.node} -t RUNNING -h -o '%i|%j|%u|%T|%M|%m'"
    )
    
    pending_output = executor.run(
        f"squeue -t PENDING -h -o '%i|%j|%u|%T|%M|%P|%m'"
    )
    
    running_jobs = []
    pending_jobs = []
    
    for line in running_output.splitlines():
        if not line.strip():
            continue
        parts = line.split('|')
        if len(parts) >= 6:
            running_jobs.append({
                'id': parts[0],
                'name': parts[1],
                'user': parts[2],
                'status': parts[3],
                'time': parts[4],
                'mem': parts[5]
            })
    
    if pending_output.strip():
        node_info = executor.run(f"sinfo -N -n {args.node} -h -o '%P'")
        node_partitions = [p.strip() for p in node_info.splitlines() if p.strip()]
        
        for line in pending_output.splitlines():
            if not line.strip():
                continue
            parts = line.split('|')
            if len(parts) >= 7:
                job_partition = parts[5]
                if job_partition in node_partitions:
                    pending_jobs.append({
                        'id': parts[0],
                        'name': parts[1],
                        'user': parts[2],
                        'status': parts[3],
                        'wait_time': parts[4],
                        'mem': parts[6]
                    })
    
    print(f"Node: {args.node}")
    print()
    
    print(f"[RUNNING] Running jobs ({len(running_jobs)})")
    if running_jobs:
        print(f"{'JOBID':<10} {'Name':<25} {'User':<12} {'Runtime':<12} {'Memory'}")
        print("-" * 75)
        for job in running_jobs:
            print(f"{job['id']:<10} {job['name'][:24]:<25} {job['user']:<12} "
                  f"{job['time']:<12} {job['mem']}")
    else:
        print("  None")
    
    print()
    
    print(f"[PENDING] Pending jobs ({len(pending_jobs)})")
    if pending_jobs:
        print(f"{'JOBID':<10} {'Name':<25} {'User':<12} {'WaitTime':<12} {'Memory'}")
        print("-" * 75)
        for job in pending_jobs:
            print(f"{job['id']:<10} {job['name'][:24]:<25} {job['user']:<12} "
                  f"{job['wait_time']:<12} {job['mem']}")
    else:
        print("  None")
    
    print()
    print("=" * 50)
    print(f"Total: {len(running_jobs)} running, {len(pending_jobs)} pending")


# ============================================================================
# 命令: partition-info
# ============================================================================

def cmd_partition_info(args):
    """查看分区详细信息"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    executor = SlurmExecutor(config)
    
    if args.partition:
        nodes_output = executor.run(f"sinfo -p {args.partition} -N -h -o '%N|%C|%G|%m'")
        jobs_output = executor.run(
            f"squeue -p {args.partition} -t RUNNING -h -o '%i|%N|%b|%M'"
        )
    else:
        nodes_output = executor.run("sinfo -N -h -o '%N|%P|%C|%G|%m'")
        jobs_output = executor.run("squeue -t RUNNING -h -o '%i|%N|%b|%M'")
    
    nodes = {}
    for line in nodes_output.splitlines():
        if not line.strip():
            continue
        if args.partition:
            parts = line.split('|')
            if len(parts) >= 4:
                node, cpu, gres, mem = parts[0], parts[1], parts[2], parts[3]
                partition = args.partition
            else:
                continue
        else:
            parts = line.split('|')
            if len(parts) >= 5:
                node, partition, cpu, gres, mem = parts[0], parts[1], parts[2], parts[3], parts[4]
            else:
                continue
        
        cpu_a, cpu_i, cpu_o, cpu_t = parse_cpu_alloc(cpu)
        has_gpu = 'gpu' in gres.lower()
        gpu_count, gpu_type = parse_gpu_gres(gres) if has_gpu else (0, '')
        
        if partition not in nodes:
            nodes[partition] = {}
        
        nodes[partition][node] = {
            'cpu_idle': cpu_i,
            'cpu_total': cpu_t,
            'cpu_alloc': cpu_a,
            'has_gpu': has_gpu,
            'gpu_total': gpu_count,
            'gpu_type': gpu_type,
            'mem': mem,
            'jobs': 0,
            'gpu_used': 0
        }
    
    for line in jobs_output.splitlines():
        if not line.strip():
            continue
        parts = line.split('|')
        if len(parts) >= 4:
            job_id, node_list, gres, run_time = parts[0], parts[1], parts[2], parts[3]
            for node in node_list.split(','):
                if not node.strip():
                    continue
                for p, p_nodes in nodes.items():
                    if node in p_nodes:
                        p_nodes[node]['jobs'] += 1
                        match = re.search(r'gpu:\w*:?(\d+)', gres.lower())
                        if match:
                            p_nodes[node]['gpu_used'] += int(match.group(1))
    
    for partition, p_nodes in sorted(nodes.items()):
        print(f"\n{'='*60}")
        print(f"Partition: {partition}")
        print(f"{'='*60}")
        
        gpu_nodes = {k: v for k, v in p_nodes.items() if v['has_gpu']}
        cpu_nodes = {k: v for k, v in p_nodes.items() if not v['has_gpu']}
        
        if gpu_nodes:
            print(f"\n[GPU Nodes] ({len(gpu_nodes)})")
            print(f"{'Node':<18} {'GPU Idle/Total':<14} {'CPU Idle/Total':<14} {'Jobs':<8} {'Memory'}")
            print("-" * 75)
            for node, info in sorted(gpu_nodes.items()):
                gpu_idle = max(0, info['gpu_total'] - info['gpu_used'])
                print(f"{node:<18} {gpu_idle}/{info['gpu_total']:<12} "
                      f"{info['cpu_idle']}/{info['cpu_total']:<12} "
                      f"{info['jobs']:<8} {info['mem']}")
        
        if cpu_nodes:
            print(f"\n[CPU Nodes] ({len(cpu_nodes)})")
            print(f"{'Node':<18} {'CPU Idle/Total':<14} {'Jobs':<8} {'Memory'}")
            print("-" * 55)
            for node, info in sorted(cpu_nodes.items()):
                print(f"{node:<18} {info['cpu_idle']}/{info['cpu_total']:<12} "
                      f"{info['jobs']:<8} {info['mem']}")


# ============================================================================
# 命令: find-gpu
# ============================================================================

def cmd_find_gpu(args):
    """查找 GPU 资源"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    executor = SlurmExecutor(config)
    
    output = executor.run("sinfo -N -o '%N|%G|%C|%P'")
    
    gpu_nodes = []
    for line in output.splitlines()[1:]:
        if not line.strip():
            continue
        parts = line.split('|')
        if len(parts) >= 4:
            node, gres, cpu, partition = parts[0], parts[1], parts[2], parts[3]
            
            if 'gpu' in gres.lower():
                gpu_count, gpu_type = parse_gpu_gres(gres)
                cpu_a, cpu_i, cpu_o, cpu_t = parse_cpu_alloc(cpu)
                
                if args.gpu_type and args.gpu_type.lower() not in gpu_type.lower():
                    continue
                
                gpu_nodes.append({
                    'node': node,
                    'partition': partition,
                    'gpu_type': gpu_type.upper() if gpu_type else 'GPU',
                    'gpu_total': gpu_count,
                    'cpu_idle': cpu_i,
                    'cpu_total': cpu_t
                })
    
    if not gpu_nodes:
        print_info("No matching GPU nodes found")
        return
    
    nodes_str = ','.join([n['node'] for n in gpu_nodes])
    jobs_output = executor.run(f"squeue -t RUNNING -h -o '%N|%b' -w {nodes_str}")
    
    node_gpu_used = {}
    for line in jobs_output.splitlines():
        if not line.strip():
            continue
        parts = line.split('|')
        if len(parts) >= 2:
            node = parts[0]
            gres = parts[1]
            match = re.search(r'gpu:\w*:?(\d+)', gres.lower())
            if match:
                used = int(match.group(1))
                node_gpu_used[node] = node_gpu_used.get(node, 0) + used
    
    print(f"{'Node':<20} {'Partition':<12} {'GPU Idle/Total':<15} {'CPU Idle/Total':<15} {'GPU Type'}")
    print("-" * 90)
    
    for node in gpu_nodes:
        node_name = node['node']
        gpu_used = node_gpu_used.get(node_name, 0)
        gpu_idle = max(0, node['gpu_total'] - gpu_used)
        
        print(f"{node_name:<20} {node['partition']:<12} "
              f"{gpu_idle}/{node['gpu_total']:<13} "
              f"{node['cpu_idle']}/{node['cpu_total']:<13} "
              f"{node['gpu_type']}")


# ============================================================================
# 命令: alloc / release / run / submit / jobs / log / cancel / history
# ============================================================================

def cmd_alloc(args):
    """申请交互式资源（优化版）"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.partition:
        die("Must specify partition (-p)")

    executor = SlurmExecutor(config)

    # 1. 解析用户需求
    gpu_count = 0
    gpu_type = None
    if args.gres:
        gpu_count, gpu_type = parse_gpu_gres(args.gres)

    min_cpus = max(1, args.cpus) if args.cpus > 0 else 1

    # 2. 检查资源可用性
    print_info(f"检查分区 {args.partition} 的资源可用性...")
    check_result = check_partition_resources(
        executor,
        args.partition,
        gpu_count=gpu_count,
        gpu_type=gpu_type,
        min_cpus=min_cpus
    )

    # 3. 打印资源状态
    _print_resource_status(check_result)

    # 4. 处理资源不足的情况
    if not check_result.has_available:
        print_warning(check_result.message)
        if check_result.wait_estimate:
            print_info(f"等待时间估计: {check_result.wait_estimate}")

        print_info("您可以：")
        print("  1. 等待资源释放（继续提交，进入排队）")
        print("  2. 减少资源请求（如减少 GPU 数量）")
        print("  3. 尝试其他分区")

        if args.check_only:
            return

        # 让用户决定是否继续排队
        print_info("继续申请资源（将进入排队）...")
    else:
        if args.check_only:
            print_success("资源检查完成，有空闲资源可用")
            return

    # 5. 确定使用的节点
    best_node = check_result.best_node
    if args.nodelist:
        # 用户指定了节点，使用用户的
        best_node = None  # 不自动选择
    elif best_node:
        print_info(f"推荐节点: {best_node.node_name}")

    # 6. 计算 CPU 数量
    if args.cpus > 0:
        cpus = args.cpus
        print_info(f"使用指定的 CPU 数量: {cpus}")
    else:
        cpus = check_result.recommended_cpus
        print_info(f"自动计算 CPU 数量: {cpus}")

    # 7. 本地模式特殊处理
    if config.get_mode() == "local":
        print_warning("Local mode: salloc is an interactive command")
        print_info("请在终端直接运行以下命令:")
        cmd = _build_salloc_command(args, cpus, best_node)
        print(f"  {cmd}")
        return

    # 8. 构建并执行 salloc 命令
    cmd = _build_salloc_command(args, cpus, best_node)
    print_info(f"执行命令: {cmd}")
    output = executor.run(cmd)
    print(output)


def _print_resource_status(result):
    """打印资源状态"""
    if result.available_nodes:
        print(f"\n可用节点 ({len(result.available_nodes)} 个):")
        print(f"{'节点':<20} {'GPU空闲/总数':<14} {'CPU空闲/总数':<14} {'GPU型号':<12}")
        print("-" * 70)
        for node in result.available_nodes[:5]:  # 最多显示 5 个
            print(f"{node.node_name:<20} "
                  f"{node.gpu_idle}/{node.gpu_total:<12} "
                  f"{node.cpu_idle}/{node.cpu_total:<12} "
                  f"{node.gpu_type:<12}")
        if len(result.available_nodes) > 5:
            print(f"... 还有 {len(result.available_nodes) - 5} 个节点")
        print()
    else:
        print_warning("没有找到可用节点")


def _build_salloc_command(args, cpus: int, best_node=None) -> str:
    """构建 salloc 命令"""
    cmd_parts = [f"salloc -p {args.partition}"]

    # CPU 数量
    cmd_parts.append(f"--cpus-per-task={cpus}")

    # GRES（GPU）
    if args.gres:
        cmd_parts.append(f"--gres={args.gres}")

    # 时间限制（仅当用户明确指定时才添加）
    if args.time:
        cmd_parts.append(f"--time={args.time}")

    # 内存（仅当用户明确指定时才添加）
    if args.mem:
        cmd_parts.append(f"--mem={args.mem}")

    # 节点选择
    if args.nodelist:
        cmd_parts.append(f"-w {args.nodelist}")
    elif best_node:
        cmd_parts.append(f"-w {best_node.node_name}")

    # 最大等待时间
    if args.max_wait:
        cmd_parts.append(f"--wait={args.max_wait}")

    return " ".join(cmd_parts)


def cmd_release(args):
    """释放资源"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.job_id:
        die("Must specify job ID")

    executor = SlurmExecutor(config)
    print_warning(f"Releasing resources: job {args.job_id}")
    executor.run(f"scancel {args.job_id}")
    print_success("Resources released")


def cmd_run(args):
    """srun 运行命令"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.command:
        die("Must specify command to run")

    executor = SlurmExecutor(config)
    cmd = f"srun {' '.join(args.command)}"
    output = executor.run(cmd)
    print(output)


def cmd_submit(args):
    """提交作业"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.script:
        die("Must specify script path")

    executor = SlurmExecutor(config)
    output = executor.run(f"sbatch '{args.script}'")
    print(output)

    match = re.search(r'Submitted batch job (\d+)', output)
    if match:
        job_id = match.group(1)
        _record_job(job_id, args.script)
        print_success(f"Job submitted: {job_id}")


def _record_job(job_id: str, script: str):
    """记录作业到历史"""
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    jobs_data = {"jobs": []}
    if JOBS_FILE.exists():
        try:
            jobs_data = json.loads(JOBS_FILE.read_text())
        except json.JSONDecodeError:
            pass
    jobs_data["jobs"].append({
        "job_id": job_id,
        "script": script,
        "submitted_at": datetime.now().isoformat(),
        "status": "PENDING"
    })
    JOBS_FILE.write_text(json.dumps(jobs_data, indent=2, ensure_ascii=False))


def cmd_jobs(args):
    """查看作业状态"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    executor = SlurmExecutor(config)

    if args.id:
        cmd = f"squeue -j {args.id}"
    else:
        # 使用环境变量获取用户名，更安全可靠
        username = os.environ.get('USER') or os.environ.get('USERNAME')
        if not username:
            die("Unable to get username")
        cmd = f"squeue -u {username}"

    cmd += " -o '%.8i %.9P %.30j %.8u %.2t %.10M %.6D %R'"
    output = executor.run(cmd)
    print(output)


def cmd_log(args):
    """查看作业日志"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.job_id:
        die("Must specify job ID")

    executor = SlurmExecutor(config)
    log_file = f"slurm-{args.job_id}.out"

    if args.follow:
        # tail -f 会无限阻塞，不适合通过脚本调用
        # 给出提示让用户直接运行
        if config.get_mode() == "local":
            print_warning("Local mode: Please run the following command directly in your terminal to view real-time logs:")
            print(f"  tail -f {log_file}")
        else:
            print_warning("Real-time log tracking is not suitable for SSH script calls")
            print_info("Recommended: Use the following command to view logs directly:")
            print(f"  ssh -p {config.get_cluster_info().get('port', 22)} {config.get_cluster_info().get('username')}@{config.get_cluster_info().get('host')} 'tail -f {log_file}'")
        return

    output = executor.run(f"cat {log_file} 2>/dev/null || echo 'Log file not found'")
    print(output)


def cmd_cancel(args):
    """取消作业"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.job_ids:
        die("Must specify job ID")

    executor = SlurmExecutor(config)
    for job_id in args.job_ids:
        print_warning(f"Cancelling job: {job_id}")
        executor.run(f"scancel {job_id}")
    print_success(f"Cancelled {len(args.job_ids)} job(s)")


def cmd_history(args):
    """作业历史"""
    if not JOBS_FILE.exists():
        print_info("No job history")
        return

    try:
        jobs_data = json.loads(JOBS_FILE.read_text())
        for job in jobs_data.get("jobs", []):
            status = job.get("status", "UNKNOWN")
            job_id = job.get("job_id", "?")
            script = job.get("script", "unnamed")
            submitted = job.get("submitted_at", "?")
            print(f"[{status}] {job_id} - {script} ({submitted})")
    except json.JSONDecodeError:
        print_info("Job history file corrupted")


# ============================================================================
# 命令: upload / download
# ============================================================================

def cmd_upload(args):
    """上传文件/目录到集群"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.local:
        die("Must specify local path")

    if not args.remote:
        die("Must specify remote path")

    local_path = Path(args.local)
    if not local_path.exists():
        die(f"Local path does not exist: {args.local}")

    local_type = "directory" if local_path.is_dir() else "file"
    print_info(f"Local {local_type}: {args.local} ({local_path.stat().st_size} bytes)" if local_path.is_file() else f"Local {local_type}: {args.local}")

    is_dir = local_path.is_dir()

    executor = SlurmExecutor(config)
    success = executor.transfer(
        src=args.local,
        dst=args.remote,
        download=False,
        recursive=is_dir or args.recursive
    )

    if not success:
        sys.exit(1)


def cmd_download(args):
    """从集群下载文件/目录"""
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.remote:
        die("Must specify remote path")

    if not args.local:
        die("Must specify local path")

    executor = SlurmExecutor(config)

    # 检查远程文件/目录是否存在
    remote_exists, remote_type = executor.check_remote_exists(args.remote)
    if not remote_exists:
        die(f"Remote path does not exist: {args.remote}")

    print_info(f"Remote path type: {remote_type}")

    success = executor.transfer(
        src=args.remote,
        dst=args.local,
        download=True,
        recursive=args.recursive
    )

    if not success:
        sys.exit(1)


def cmd_ssh_test(args):
    """SSH 连接诊断"""
    config = ConfigManager()

    # 使用命令行参数或配置
    host = args.host if args.host else config.get_cluster_info().get("host", "")
    port = args.port if args.port else config.get_cluster_info().get("port", 22)
    username = args.username if args.username else config.get_cluster_info().get("username", "")

    if not host or not username:
        print_error("Host and username are required (use --host and --username, or run init first)")
        return

    print_info(f"SSH Diagnostics for {username}@{host}:{port}")
    print()

    # 1. 检查平台信息
    print_info("Platform Information:")
    print(f"  OS: {platform.system()} {platform.release()}")
    print(f"  Python: {platform.python_version()}")
    print(f"  SSH Command: ssh")
    print()

    # 2. 检查 SSH 密钥
    print_info("SSH Key Check:")
    key_found = check_ssh_key_exists()
    if key_found:
        print_success("  SSH keys found")
    else:
        print_warning("  No SSH keys found in ~/.ssh/")
    print()

    # 3. 测试 SSH 连接
    print_info("SSH Connection Test:")
    print(f"  Testing: ssh -p {port} {username}@{host} 'echo ok'")

    ssh_ok, ssh_error = check_ssh_connection(host, port, username)
    if ssh_ok:
        print_success("  SSH connection successful!")
    else:
        print_error(f"  SSH connection failed: {ssh_error}")
    print()

    # 4. Windows 特定提示
    if platform.system() == "Windows":
        print_info("Windows-specific checks:")
        # 检查是否有 Git Bash
        git_bash_paths = [
            "C:/Program Files/Git/bin/sh.exe",
            "C:/Program Files/Git/usr/bin/ssh.exe",
            str(Path.home() / "AppData/Local/Programs/Git/bin/ssh.exe")
        ]
        git_bash_found = any(Path(p).exists() for p in git_bash_paths)
        if git_bash_found:
            print_success("  Git Bash SSH found")
        else:
            print_warning("  Git Bash SSH not found")

        # 检查 Windows OpenSSH
        win_ssh = Path("C:/Windows/System32/OpenSSH/ssh.exe")
        if win_ssh.exists():
            print_success("  Windows OpenSSH found")
            print_info("  Note: Windows OpenSSH may require different key format")
        else:
            print_warning("  Windows OpenSSH not found in System32")

        print()
        print_info("Windows SSH Tips:")
        print("  1. Use Git Bash for best compatibility")
        print("  2. Ensure keys are in ~/.ssh/ (not .ppk format)")
        print("  3. Test manually: ssh -p PORT user@host")
        print("  4. Check firewall and antivirus settings")


def cmd_exec(args):
    """
    在集群上执行任意命令（统一入口，避免多次授权）

    注意：此命令是核心功能，用于减少 SSH 连接的授权询问次数。
    所有需要直接在集群上执行的操作都应通过此命令进行。

    安全性：AI 模型在调用此命令前应评估命令安全性。
    """
    config = ConfigManager()
    if not config.is_configured():
        die("Please run 'init' first to configure")

    if not args.cmd:
        die("Must specify command to execute")

    command = args.cmd

    executor = SlurmExecutor(config)
    output = executor.run(command)

    # 输出结果
    if output:
        print(output)


def cmd_path(args):
    """打印脚本和配置路径信息"""
    paths = {
        "script": str(Path(__file__).resolve()),
        "skill_dir": str(SKILL_DIR),
        "config_dir": str(CONFIG_DIR),
        "config_file": str(CONFIG_FILE),
        "jobs_file": str(JOBS_FILE),
        "settings_file": str(SETTINGS_FILE) if SETTINGS_FILE else None,
    }

    if args.json:
        print(json.dumps(paths, indent=2))
    else:
        print(f"Script:      {paths['script']}")
        print(f"Skill Dir:   {paths['skill_dir']}")
        print(f"Config Dir:  {paths['config_dir']}")
        print(f"Config:      {paths['config_file']}")
        print(f"Jobs:        {paths['jobs_file']}")
        print(f"Settings:    {paths['settings_file'] or '(no agent detected)'}")


# ============================================================================
# 主函数
# ============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Slurm Cluster Assistant CLI",
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    subparsers = parser.add_subparsers(dest="command", help="命令")

    # init
    init_parser = subparsers.add_parser("init", help="初始化配置")
    init_parser.add_argument("--check", action="store_true")
    init_parser.add_argument("--output-json", action="store_true")
    init_parser.add_argument("--fast", action="store_true", help="快速模式：跳过 SSH 连接测试")
    init_parser.add_argument("--mode", choices=["local", "remote"])
    init_parser.add_argument("--cluster-name")
    init_parser.add_argument("--host")
    init_parser.add_argument("--port", type=int, default=22)
    init_parser.add_argument("--username")
    init_parser.add_argument("--jump-host")
    init_parser.add_argument("--authorize", action="store_true", help="授权当前 Agent")
    init_parser.add_argument("--unauthorize", action="store_true", help="取消当前 Agent 授权")

    # status
    status_parser = subparsers.add_parser("status", help="查看资源状态")
    status_parser.add_argument("-p", "--partition")
    status_parser.add_argument("-n", "--nodes", action="store_true")
    status_parser.add_argument("--gpu", action="store_true", help="显示 GPU 节点详情")

    # node-info
    node_parser = subparsers.add_parser("node-info", help="查看节点详情")
    node_parser.add_argument("node", nargs="?")

    # node-jobs
    node_jobs_parser = subparsers.add_parser("node-jobs", help="查看节点上的作业")
    node_jobs_parser.add_argument("node", nargs="?", help="节点名称")

    # partition-info
    partition_parser = subparsers.add_parser("partition-info", help="查看分区详细信息")
    partition_parser.add_argument("-p", "--partition", help="指定分区")

    # find-gpu
    gpu_parser = subparsers.add_parser("find-gpu", help="查找 GPU 资源")
    gpu_parser.add_argument("gpu_type", nargs="?", help="GPU 型号（可选，不指定则显示所有）")

    # alloc
    alloc_parser = subparsers.add_parser("alloc", help="申请交互式资源")
    alloc_parser.add_argument("-p", "--partition", required=True, help="分区")
    alloc_parser.add_argument("-g", "--gres", help="GRES 资源（如 gpu:1, gpu:a100:2）")
    alloc_parser.add_argument("-c", "--cpus", type=int, default=0, help="CPU 数量（0=自动计算，默认自动）")
    alloc_parser.add_argument("-t", "--time", default=None, help="时间限制（不指定则使用分区默认值）")
    alloc_parser.add_argument("--mem", default=None, help="内存需求（如 16G）")
    alloc_parser.add_argument("-w", "--nodelist", help="指定节点列表")
    alloc_parser.add_argument("--check-only", action="store_true", help="仅检查资源，不实际申请")
    alloc_parser.add_argument("--max-wait", help="最大等待时间（如 10:00 或 5 表示 5 分钟）")

    # release
    release_parser = subparsers.add_parser("release", help="释放资源")
    release_parser.add_argument("job_id", nargs="?")

    # run
    run_parser = subparsers.add_parser("run", help="srun 运行命令")
    run_parser.add_argument("command", nargs="*")

    # submit
    submit_parser = subparsers.add_parser("submit", help="提交作业")
    submit_parser.add_argument("script", nargs="?")

    # jobs
    jobs_parser = subparsers.add_parser("jobs", help="查看作业状态")
    jobs_parser.add_argument("--id", "-i")

    # log
    log_parser = subparsers.add_parser("log", help="查看作业日志")
    log_parser.add_argument("job_id", nargs="?")
    log_parser.add_argument("-f", "--follow", action="store_true")

    # cancel
    cancel_parser = subparsers.add_parser("cancel", help="取消作业")
    cancel_parser.add_argument("job_ids", nargs="*")

    # history
    subparsers.add_parser("history", help="作业历史")

    # upload / download
    upload_parser = subparsers.add_parser("upload", help="上传文件/目录")
    upload_parser.add_argument("local", nargs="?")
    upload_parser.add_argument("remote", nargs="?")
    upload_parser.add_argument("-r", "--recursive", action="store_true")

    download_parser = subparsers.add_parser("download", help="下载文件/目录")
    download_parser.add_argument("remote", nargs="?")
    download_parser.add_argument("local", nargs="?")
    download_parser.add_argument("-r", "--recursive", action="store_true")

    # ssh-test - SSH 连接诊断
    ssh_test_parser = subparsers.add_parser("ssh-test", help="SSH connection diagnostics")
    ssh_test_parser.add_argument("--host", help="Remote host (optional, uses config if not specified)")
    ssh_test_parser.add_argument("--port", type=int, help="SSH port (optional, uses config if not specified)")
    ssh_test_parser.add_argument("--username", help="Username (optional, uses config if not specified)")

    # exec - 在集群上执行任意命令（统一入口）
    exec_parser = subparsers.add_parser("exec", help="在集群上执行命令（统一入口，避免多次授权）")
    exec_parser.add_argument("-c", "--cmd", required=True, help="要执行的命令")

    # path - 打印路径信息
    path_parser = subparsers.add_parser("path", help="打印脚本和配置路径信息")
    path_parser.add_argument("--json", action="store_true", help="JSON 格式输出")

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return

    cmd_map = {
        "init": cmd_init,
        "status": cmd_status,
        "node-info": cmd_node_info,
        "node-jobs": cmd_node_jobs,
        "partition-info": cmd_partition_info,
        "find-gpu": cmd_find_gpu,
        "alloc": cmd_alloc,
        "release": cmd_release,
        "run": cmd_run,
        "submit": cmd_submit,
        "jobs": cmd_jobs,
        "log": cmd_log,
        "cancel": cmd_cancel,
        "history": cmd_history,
        "upload": cmd_upload,
        "download": cmd_download,
        "ssh-test": cmd_ssh_test,
        "exec": cmd_exec,
        "path": cmd_path,
    }

    if args.command in cmd_map:
        cmd_map[args.command](args)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
