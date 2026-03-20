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
from typing import Any, Dict, List, Optional, Tuple

# 全局配置路径（固定全局安装）
SKILL_DIR = Path.home() / ".claude" / "skills" / "slurm-assistant"
CONFIG_FILE = SKILL_DIR / "config.json"
JOBS_FILE = SKILL_DIR / "jobs.json"


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
        SKILL_DIR.mkdir(parents=True, exist_ok=True)
        CONFIG_FILE.write_text(json.dumps(self.config, indent=2, ensure_ascii=False))

    def is_configured(self) -> bool:
        return "mode" in self.config

    def get_mode(self) -> str:
        return self.config.get("mode", "local")

    def get_cluster_info(self) -> Dict[str, Any]:
        return self.config.get("cluster", {})


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
                die("远程模式缺少 host 或 username 配置")

            ssh_cmd = ["ssh", "-p", str(port)] + self.ssh_opts.copy()

            if jump_host:
                ssh_cmd.extend(["-J", jump_host])

            ssh_cmd.append(f"{username}@{host}")
            ssh_cmd.append(cmd)

            result = subprocess.run(
                ssh_cmd,
                capture_output=True,
                text=True,
                creationflags=creation_flags
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
                print_success(f"复制完成: {src} -> {dst}")
                return True
            except Exception as e:
                print_error(f"复制失败: {e}")
                return False

        cluster = self.config.get_cluster_info()
        host = cluster.get("host", "")
        port = cluster.get("port", 22)
        username = cluster.get("username", "")
        jump_host = cluster.get("jump_host", "")

        if not host or not username:
            die("远程模式缺少 host 或 username 配置")

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
        print_info(f"传输: {' '.join(scp_cmd)}")

        try:
            result = subprocess.run(
                scp_cmd,
                capture_output=True,
                text=True,
                creationflags=creation_flags
            )
            if result.returncode == 0:
                print_success(f"传输完成: {src} -> {dst}")
                return True
            else:
                print_error(f"传输失败: {result.stderr}")
                return False
        except Exception as e:
            print_error(f"传输异常: {e}")
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
    """解析 GRES 字符串，返回 (gpu数量, gpu型号)"""
    match = re.search(r'gpu(?::(\w+))?:(\d+)', gres_str.lower())
    if match:
        gpu_type = match.group(1) or "unknown"
        gpu_count = int(match.group(2))
        return gpu_count, gpu_type
    return 0, ""


def parse_cpu_alloc(alloc_str: str) -> Tuple[int, int, int, int]:
    """解析 CPU 分配字符串 A/I/O/T，返回 (allocated, idle, other, total)"""
    parts = alloc_str.split('/')
    if len(parts) == 4:
        return int(parts[0]), int(parts[1]), int(parts[2]), int(parts[3])
    return 0, 0, 0, 0


def calculate_optimal_cpus(executor: SlurmExecutor, partition: str, gres: Optional[str] = None) -> int:
    """
    智能计算合理的 CPU 数量
    返回: min(节点剩余 CPU 数, 节点 CPU 总数/节点显卡总数)
    """
    # 获取分区节点信息
    cmd = f"sinfo -p {partition} -N -h -o '%N|%C|%G'"
    output = executor.run(cmd)
    
    nodes_info = []
    for line in output.splitlines():
        if not line.strip():
            continue
        parts = line.split('|')
        if len(parts) >= 3:
            node, cpu, gres = parts[0], parts[1], parts[2]
            cpu_a, cpu_i, cpu_o, cpu_t = parse_cpu_alloc(cpu)
            gpu_count, gpu_type = parse_gpu_gres(gres)
            
            nodes_info.append({
                'cpu_idle': cpu_i,
                'cpu_total': cpu_t,
                'gpu_total': gpu_count
            })
    
    if not nodes_info:
        return 1  # 默认值
    
    # 计算 GPU 节点的平均 CPU/GPU 比例
    gpu_nodes = [n for n in nodes_info if n['gpu_total'] > 0]
    
    if gpu_nodes:
        # GPU 节点：计算 CPU 总数 / GPU 总数，取平均
        avg_cpus_per_gpu = sum(n['cpu_total'] // max(n['gpu_total'], 1) for n in gpu_nodes) // len(gpu_nodes)
        # 取最小的空闲 CPU 数
        min_idle_cpu = min(n['cpu_idle'] for n in gpu_nodes)
        
        return max(1, min(min_idle_cpu, avg_cpus_per_gpu))
    else:
        # CPU 节点：使用最小的空闲 CPU 数
        return max(1, min(n['cpu_idle'] for n in nodes_info))


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
        # Windows 可能的密钥位置
        if platform.system() == "Windows":
            # 检查用户目录下的 .ssh
            if (Path.home() / key_file).exists():
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


def check_ssh_connection(host: str, port: int, username: str, jump_host: str = "") -> Tuple[bool, str]:
    """
    检查 SSH 连接和免密登录（跨平台）
    返回: (是否成功, 错误信息)
    """
    ssh_cmd = ["ssh", "-p", str(port), "-o", "ConnectTimeout=5", "-o", "BatchMode=yes"]

    if jump_host:
        ssh_cmd.extend(["-J", jump_host])

    ssh_cmd.append(f"{username}@{host}")
    ssh_cmd.append("echo ok")

    try:
        result = subprocess.run(
            ssh_cmd,
            capture_output=True,
            text=True,
            creationflags=subprocess.CREATE_NO_WINDOW if platform.system() == "Windows" else 0
        )

        if result.returncode == 0 and "ok" in result.stdout:
            return True, ""
        elif "Permission denied" in result.stderr or "publickey" in result.stderr:
            return False, "SSH 免密登录未配置"
        elif "Could not resolve hostname" in result.stderr:
            return False, "无法解析主机名"
        elif "Connection refused" in result.stderr:
            return False, "连接被拒绝，请检查端口"
        elif "Connection timed out" in result.stderr:
            return False, "连接超时"
        else:
            return False, result.stderr.strip() or "未知错误"
    except FileNotFoundError:
        return False, "SSH 客户端未找到"
    except Exception as e:
        return False, str(e)


def cmd_init(args):
    """初始化配置"""
    config = ConfigManager()

    if args.check:
        # 配置检查模式
        result = {
            "configured": config.is_configured(),
            "local_slurm_available": False,
            "ssh_key_configured": False,
            "ssh_connection_ok": False,
            "config_valid": False
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
                print_success("配置文件: 已加载")

                mode = config.get_mode()
                cluster = config.get_cluster_info()

                if mode == "local":
                    if result["local_slurm_available"]:
                        print_success("本地 Slurm: 可用")
                    else:
                        print_warning("本地 Slurm: 不可用")
                else:
                    print_info(f"集群: {cluster.get('name', '未知')}")
                    print_info(f"地址: {cluster.get('username', '未知')}@{cluster.get('host', '未知')}:{cluster.get('port', 22)}")

                    if result["ssh_connection_ok"]:
                        print_success("SSH 连接: 成功（免密登录已配置）")
                    else:
                        print_warning(f"SSH 连接: 失败 ({result.get('ssh_error', '未知错误')})")
            else:
                print_warning("配置文件: 未配置")

                if result["local_slurm_available"]:
                    print_info("检测到本地 Slurm，可使用本地模式")
                else:
                    print_info("未检测到本地 Slurm")

                if result["ssh_key_configured"]:
                    print_info("SSH 密钥: 已配置")
                else:
                    print_warning("SSH 密钥: 未配置")
        return

    if not args.mode:
        die("必须指定 --mode (local 或 remote)")

    if args.mode == "local":
        config.config = {
            "mode": "local",
            "cluster": {"name": args.cluster_name or "local"}
        }
        print_info("配置模式: 本地")
    else:
        if not args.host or not args.username:
            die("远程模式必须指定 --host 和 --username")

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
        print_info("配置模式: 远程")
        print_info(f"集群: {args.cluster_name or 'remote'}")
        print_info(f"地址: {args.username}@{args.host}:{args.port or 22}")
        if args.jump_host:
            print_info(f"跳板机: {args.jump_host}")

    config.save()
    print_success(f"配置已保存到 {CONFIG_FILE}")

    if args.mode == "remote":
        print_info("检查 SSH 连接...")

        # 先检查 SSH 密钥
        if not check_ssh_key_exists():
            print_warning("未检测到 SSH 密钥，可能需要配置免密登录")
            print_info("参考: https://docs.github.com/zh/authentication/connecting-to-github-with-ssh/generating-a-new-ssh-key-and-adding-it-to-the-ssh-agent")

        executor = SlurmExecutor(config)
        try:
            result = executor.run("echo '连接成功'")
            if "连接成功" in result:
                print_success("SSH 连接成功")
            else:
                print_warning("SSH 连接失败，请配置免密登录")
                print_info("运行以下命令配置免密登录:")
                print(f"  ssh-copy-id -p {args.port or 22} {args.username}@{args.host}")
        except Exception as e:
            print_warning(f"SSH 连接失败: {e}")
            print_info("请检查:")
            print_info("  1. 主机地址是否正确")
            print_info("  2. 是否已配置免密登录")
            print_info("  3. 网络连接是否正常")
    else:
        # 本地模式检查 Slurm
        if not check_local_slurm():
            print_warning("未检测到本地 Slurm 命令")
            print_info("请确保:")
            print_info("  1. 您在 Slurm 集群的登录节点上")
            print_info("  2. Slurm 命令（sinfo, squeue 等）可用")
        else:
            print_success("检测到本地 Slurm")


# ============================================================================
# 命令: status
# ============================================================================

def cmd_status(args):
    """查看资源状态"""
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

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
    """显示 GPU 节点状态"""
    cmd = "sinfo -N -o '%N|%G|%C|%P'"
    if partition:
        cmd += f" -p {partition}"
    
    output = executor.run(cmd)
    
    gpu_nodes = []
    for line in output.splitlines()[1:]:
        if not line.strip():
            continue
        parts = line.split('|')
        if len(parts) >= 4:
            node_name, gres, cpu_alloc, part = parts[0], parts[1], parts[2], parts[3]
            
            if 'gpu' in gres.lower():
                gpu_count, gpu_type = parse_gpu_gres(gres)
                cpu_a, cpu_i, cpu_o, cpu_t = parse_cpu_alloc(cpu_alloc)
                gpu_nodes.append({
                    'node': node_name,
                    'partition': part,
                    'gpu_type': gpu_type.upper() if gpu_type else 'GPU',
                    'gpu_total': gpu_count,
                    'cpu_idle': cpu_i,
                    'cpu_total': cpu_t,
                    'cpu_alloc': cpu_a
                })
    
    if not gpu_nodes:
        print_info("未找到 GPU 节点")
        return
    
    gpu_nodes.sort(key=lambda x: (x['partition'], x['node']))

    # 安全过滤：只允许合法节点名
    valid_nodes = [n['node'] for n in gpu_nodes if validate_node_name(n['node'])]
    if not valid_nodes:
        print_info("未找到合法的 GPU 节点")
        return

    nodes_str = ','.join(valid_nodes)
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
    
    print(f"{'节点':<20} {'分区':<12} {'GPU 空闲/总数':<15} {'CPU 空闲/总数':<15} {'GPU型号'}")
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
# 命令: node-info
# ============================================================================

def cmd_node_info(args):
    """查看节点详情"""
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

    if not args.node:
        die("必须指定节点名称")

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
        die("请先运行 init 配置")

    if not args.node:
        die("必须指定节点名称")

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
    
    print(f"节点: {args.node}")
    print()
    
    print(f"[RUNNING] 运行中的作业 ({len(running_jobs)} 个)")
    if running_jobs:
        print(f"{'JOBID':<10} {'名称':<25} {'用户':<12} {'运行时长':<12} {'内存'}")
        print("-" * 75)
        for job in running_jobs:
            print(f"{job['id']:<10} {job['name'][:24]:<25} {job['user']:<12} "
                  f"{job['time']:<12} {job['mem']}")
    else:
        print("  无")
    
    print()
    
    print(f"[PENDING] 排队中的作业 ({len(pending_jobs)} 个)")
    if pending_jobs:
        print(f"{'JOBID':<10} {'名称':<25} {'用户':<12} {'排队时长':<12} {'内存'}")
        print("-" * 75)
        for job in pending_jobs:
            print(f"{job['id']:<10} {job['name'][:24]:<25} {job['user']:<12} "
                  f"{job['wait_time']:<12} {job['mem']}")
    else:
        print("  无")
    
    print()
    print("=" * 50)
    print(f"总计: {len(running_jobs)} 个运行中, {len(pending_jobs)} 个排队中")


# ============================================================================
# 命令: partition-info
# ============================================================================

def cmd_partition_info(args):
    """查看分区详细信息"""
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

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
        print(f"分区: {partition}")
        print(f"{'='*60}")
        
        gpu_nodes = {k: v for k, v in p_nodes.items() if v['has_gpu']}
        cpu_nodes = {k: v for k, v in p_nodes.items() if not v['has_gpu']}
        
        if gpu_nodes:
            print(f"\n[GPU 节点] ({len(gpu_nodes)} 个)")
            print(f"{'节点':<18} {'GPU 空闲/总数':<14} {'CPU 空闲/总数':<14} {'作业数':<8} {'内存'}")
            print("-" * 75)
            for node, info in sorted(gpu_nodes.items()):
                gpu_idle = max(0, info['gpu_total'] - info['gpu_used'])
                print(f"{node:<18} {gpu_idle}/{info['gpu_total']:<12} "
                      f"{info['cpu_idle']}/{info['cpu_total']:<12} "
                      f"{info['jobs']:<8} {info['mem']}")
        
        if cpu_nodes:
            print(f"\n[CPU 节点] ({len(cpu_nodes)} 个)")
            print(f"{'节点':<18} {'CPU 空闲/总数':<14} {'作业数':<8} {'内存'}")
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
        die("请先运行 init 配置")

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
        print_info("未找到匹配的 GPU 节点")
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
    
    print(f"{'节点':<20} {'分区':<12} {'GPU 空闲/总数':<15} {'CPU 空闲/总数':<15} {'GPU型号'}")
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
    """申请交互式资源"""
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

    if not args.partition:
        die("必须指定分区 (-p)")

    # 本地模式下，alloc 是交互式命令，不适合脚本调用
    if config.get_mode() == "local":
        print_warning("本地模式检测：salloc 是交互式命令")
        print_info("请在终端直接运行以下命令：")
        cpus = args.cpus if args.cpus > 1 else "自动"
        cmd = f"salloc -p {args.partition}"
        if args.cpus > 1:
            cmd += f" --cpus-per-task={args.cpus}"
        if args.gres:
            cmd += f" --gres={args.gres}"
        cmd += f" --time={args.time}"
        if args.max_wait:
            cmd += f" --wait={args.max_wait}"
        print(f"  {cmd}")
        return

    executor = SlurmExecutor(config)

    # 智能计算 CPU 数量（如果未指定）
    cpus = args.cpus
    if cpus == 1:
        # 尝试智能计算
        optimal_cpus = calculate_optimal_cpus(executor, args.partition, args.gres)
        if optimal_cpus > 1:
            cpus = optimal_cpus
            print_info(f"自动计算 CPU 数量: {cpus}")

    # 构建命令
    cmd = f"salloc -p {args.partition} --cpus-per-task={cpus} --time={args.time}"

    if args.gres:
        cmd += f" --gres={args.gres}"

    if args.max_wait:
        # 添加等待时间限制
        cmd += f" --wait={args.max_wait}"
        print_info(f"最大等待时间: {args.max_wait}")

    print_info(f"申请资源: {cmd}")
    output = executor.run(cmd)
    print(output)


def cmd_release(args):
    """释放资源"""
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

    if not args.job_id:
        die("必须指定作业 ID")

    executor = SlurmExecutor(config)
    print_warning(f"即将释放资源: 作业 {args.job_id}")
    executor.run(f"scancel {args.job_id}")
    print_success("资源已释放")


def cmd_run(args):
    """srun 运行命令"""
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

    if not args.command:
        die("必须指定要运行的命令")

    executor = SlurmExecutor(config)
    cmd = f"srun {' '.join(args.command)}"
    output = executor.run(cmd)
    print(output)


def cmd_submit(args):
    """提交作业"""
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

    if not args.script:
        die("必须指定脚本路径")

    executor = SlurmExecutor(config)
    output = executor.run(f"sbatch '{args.script}'")
    print(output)

    match = re.search(r'Submitted batch job (\d+)', output)
    if match:
        job_id = match.group(1)
        _record_job(job_id, args.script)
        print_success(f"作业已提交: {job_id}")


def _record_job(job_id: str, script: str):
    """记录作业到历史"""
    SKILL_DIR.mkdir(parents=True, exist_ok=True)
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
        die("请先运行 init 配置")

    executor = SlurmExecutor(config)

    if args.id:
        cmd = f"squeue -j {args.id}"
    else:
        # 使用环境变量获取用户名，更安全可靠
        username = os.environ.get('USER') or os.environ.get('USERNAME')
        if not username:
            die("无法获取用户名")
        cmd = f"squeue -u {username}"

    cmd += " -o '%.8i %.9P %.30j %.8u %.2t %.10M %.6D %R'"
    output = executor.run(cmd)
    print(output)


def cmd_log(args):
    """查看作业日志"""
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

    if not args.job_id:
        die("必须指定作业 ID")

    executor = SlurmExecutor(config)
    log_file = f"slurm-{args.job_id}.out"

    if args.follow:
        # tail -f 会无限阻塞，不适合通过脚本调用
        # 给出提示让用户直接运行
        if config.get_mode() == "local":
            print_warning("本地模式：请直接在终端运行以下命令查看实时日志：")
            print(f"  tail -f {log_file}")
        else:
            print_warning("实时日志跟踪不适合通过 SSH 脚本调用")
            print_info("建议使用以下命令直接查看最新日志：")
            print(f"  ssh -p {config.get_cluster_info().get('port', 22)} {config.get_cluster_info().get('username')}@{config.get_cluster_info().get('host')} 'tail -f {log_file}'")
        return

    output = executor.run(f"cat {log_file} 2>/dev/null || echo '日志文件不存在'")
    print(output)


def cmd_cancel(args):
    """取消作业"""
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

    if not args.job_ids:
        die("必须指定作业 ID")

    executor = SlurmExecutor(config)
    for job_id in args.job_ids:
        print_warning(f"取消作业: {job_id}")
        executor.run(f"scancel {job_id}")
    print_success(f"已取消 {len(args.job_ids)} 个作业")


def cmd_history(args):
    """作业历史"""
    if not JOBS_FILE.exists():
        print_info("暂无作业历史")
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
        print_info("作业历史文件损坏")


# ============================================================================
# 命令: upload / download
# ============================================================================

def cmd_upload(args):
    """上传文件/目录到集群"""
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

    if not args.local:
        die("必须指定本地路径")

    if not args.remote:
        die("必须指定远程路径")

    local_path = Path(args.local)
    if not local_path.exists():
        die(f"本地路径不存在: {args.local}")

    local_type = "目录" if local_path.is_dir() else "文件"
    print_info(f"本地{local_type}: {args.local} ({local_path.stat().st_size} 字节)" if local_path.is_file() else f"本地{local_type}: {args.local}")

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
        die("请先运行 init 配置")

    if not args.remote:
        die("必须指定远程路径")

    if not args.local:
        die("必须指定本地路径")

    executor = SlurmExecutor(config)

    # 检查远程文件/目录是否存在
    remote_exists, remote_type = executor.check_remote_exists(args.remote)
    if not remote_exists:
        die(f"远程路径不存在: {args.remote}")

    print_info(f"远程路径类型: {remote_type}")

    success = executor.transfer(
        src=args.remote,
        dst=args.local,
        download=True,
        recursive=args.recursive
    )

    if not success:
        sys.exit(1)


def cmd_exec(args):
    """
    在集群上执行任意命令（统一入口，避免多次授权）

    注意：此命令是核心功能，用于减少 SSH 连接的授权询问次数。
    所有需要直接在集群上执行的操作都应通过此命令进行。

    安全性：AI 模型在调用此命令前应评估命令安全性。
    """
    config = ConfigManager()
    if not config.is_configured():
        die("请先运行 init 配置")

    if not args.cmd:
        die("必须指定要执行的命令")

    command = args.cmd

    executor = SlurmExecutor(config)
    output = executor.run(command)

    # 输出结果
    if output:
        print(output)


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
    alloc_parser.add_argument("-g", "--gres", help="GRES 资源（如 gpu:1）")
    alloc_parser.add_argument("-c", "--cpus", type=int, default=0, help="CPU 数量（0=自动计算）")
    alloc_parser.add_argument("-t", "--time", default="1:00:00", help="作业时间限制")
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

    # exec - 在集群上执行任意命令（统一入口）
    exec_parser = subparsers.add_parser("exec", help="在集群上执行命令（统一入口，避免多次授权）")
    exec_parser.add_argument("-c", "--cmd", required=True, help="要执行的命令")

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
        "exec": cmd_exec,
    }

    if args.command in cmd_map:
        cmd_map[args.command](args)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
