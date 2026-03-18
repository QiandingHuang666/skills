#!/usr/bin/env python3
"""
Slurm Cluster Assistant CLI
统一命令接口，支持本地（集群上）和远程（集群外）两种模式
"""

import argparse
import json
import os
import subprocess
import sys
import shutil
from datetime import datetime
from pathlib import Path
from typing import Optional, Dict, Any, List

# 全局配置路径
SKILL_DIR = Path.home() / ".claude" / "skills" / "slurm-assistant"
CONFIG_FILE = SKILL_DIR / "config.json"
JOBS_FILE = SKILL_DIR / "jobs.json"
PARTITION_CACHE_FILE = SKILL_DIR / "partition_cache.json"


def _detect_project_root() -> Optional[Path]:
    """检测项目根目录（查找 .git 或 .claude 目录）"""
    current = Path.cwd()
    for parent in [current] + list(current.parents):
        if (parent / ".git").exists() or (parent / ".claude").exists():
            return parent
        if parent == Path.home():
            break
    return None


def _get_project_skill_dir() -> Optional[Path]:
    """获取项目级别的 skill 目录"""
    project_root = _detect_project_root()
    if project_root:
        skill_dir = project_root / ".claude" / "skills" / "slurm-assistant"
        if skill_dir.exists():
            return skill_dir
    return None


class Colors:
    """终端颜色"""
    RED = '\033[91m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    BLUE = '\033[94m'
    BOLD = '\033[1m'
    RESET = '\033[0m'


def print_info(msg: str):
    print(f"{Colors.BLUE}ℹ{Colors.RESET} {msg}")


def print_success(msg: str):
    print(f"{Colors.GREEN}✓{Colors.RESET} {msg}")


def print_warning(msg: str):
    print(f"{Colors.YELLOW}⚠{Colors.RESET} {msg}")


def print_error(msg: str):
    print(f"{Colors.RED}✗{Colors.RESET} {msg}", file=sys.stderr)


def confirm(prompt: str, default: bool = False) -> bool:
    """确认操作"""
    suffix = " [Y/n]" if default else " [y/N]"
    response = input(f"{prompt}{suffix}: ").strip().lower()
    if not response:
        return default
    return response in ('y', 'yes', '是')


class ConfigManager:
    """配置管理器（支持项目级别和全局级别配置）"""

    DEFAULT_CONFIG = {
        "mode": None,  # "local" or "remote"
        "cluster": {
            "name": "",
            "host": "",
            "port": 22,
            "username": "",
            "jump_host": None
        },
        "ssh": {
            "key_path": str(Path.home() / ".ssh" / "id_rsa"),
            "config_entry": ""
        },
        "defaults": {
            "partition": None,
            "keep_alive_duration": "24h"
        },
        "paths": {
            "home_on_cluster": "",
            "scratch_dir": ""
        }
    }

    def __init__(self, project_root: Optional[str] = None):
        """初始化配置管理器

        Args:
            project_root: 项目根目录（可选，自动检测）
        """
        self.project_root = Path(project_root) if project_root else _detect_project_root()
        self.project_skill_dir = (
            self.project_root / ".claude" / "skills" / "slurm-assistant"
            if self.project_root else None
        )
        # 确保配置目录存在
        SKILL_DIR.mkdir(parents=True, exist_ok=True)
        if self.project_skill_dir:
            self.project_skill_dir.mkdir(parents=True, exist_ok=True)

    def _load_global(self) -> Dict[str, Any]:
        """加载全局配置"""
        if CONFIG_FILE.exists():
            try:
                with open(CONFIG_FILE, 'r', encoding='utf-8') as f:
                    return json.load(f)
            except json.JSONDecodeError:
                return {}
        return {}

    def _load_project(self) -> Dict[str, Any]:
        """加载项目级别配置"""
        if not self.project_skill_dir:
            return {}
        config_file = self.project_skill_dir / "config.json"
        if config_file.exists():
            try:
                with open(config_file, 'r', encoding='utf-8') as f:
                    return json.load(f)
            except json.JSONDecodeError:
                return {}
        return {}

    def _deep_merge(self, base: Dict[str, Any], override: Dict[str, Any]) -> Dict[str, Any]:
        """深度合并两个配置字典"""
        result = base.copy()
        for key, value in override.items():
            if key in result and isinstance(result[key], dict) and isinstance(value, dict):
                result[key] = self._deep_merge(result[key], value)
            else:
                result[key] = value
        return result

    def load(self) -> Dict[str, Any]:
        """加载配置（合并项目级和全局级，项目配置优先）"""
        global_config = self._load_global()
        project_config = self._load_project()
        # 以默认配置为基础，合并全局配置，再合并项目配置
        result = self._deep_merge(self.DEFAULT_CONFIG.copy(), global_config)
        result = self._deep_merge(result, project_config)
        return result

    def save(self, config: Dict[str, Any], to_project: bool = False):
        """保存配置

        Args:
            config: 配置内容
            to_project: 是否保存到项目级别（默认保存到全局）
        """
        if to_project and self.project_skill_dir:
            config_path = self.project_skill_dir / "config.json"
            with open(config_path, 'w', encoding='utf-8') as f:
                json.dump(config, f, indent=2, ensure_ascii=False)
            print_success(f"配置已保存到项目配置: {config_path}")
        else:
            with open(CONFIG_FILE, 'w', encoding='utf-8') as f:
                json.dump(config, f, indent=2, ensure_ascii=False)
            print_success(f"配置已保存到全局配置: {CONFIG_FILE}")

    def get_config_sources(self) -> List[str]:
        """获取配置来源列表"""
        sources = []
        if self.project_skill_dir and (self.project_skill_dir / "config.json").exists():
            sources.append(f"项目配置: {self.project_skill_dir / 'config.json'}")
        if CONFIG_FILE.exists():
            sources.append(f"全局配置: {CONFIG_FILE}")
        return sources

    def is_configured(self) -> bool:
        """检查是否已配置（检查项目级或全局）"""
        # 检查项目级别配置
        if self.project_skill_dir and (self.project_skill_dir / "config.json").exists():
            project_config = self._load_project()
            if project_config.get("mode"):
                return True
        # 检查全局配置
        return self._load_global().get("mode") is not None


class JobTracker:
    """作业追踪器（支持项目级别和全局级别）"""

    def __init__(self, project_root: Optional[str] = None):
        """初始化作业追踪器

        Args:
            project_root: 项目根目录（可选，自动检测）
        """
        self.project_root = Path(project_root) if project_root else _detect_project_root()
        self.project_skill_dir = (
            self.project_root / ".claude" / "skills" / "slurm-assistant"
            if self.project_root else None
        )
        # 确保配置目录存在
        SKILL_DIR.mkdir(parents=True, exist_ok=True)
        if self.project_skill_dir:
            self.project_skill_dir.mkdir(parents=True, exist_ok=True)

    def _get_jobs_file(self) -> Path:
        """获取作业记录文件路径（优先项目级别）"""
        if self.project_skill_dir and (self.project_skill_dir / "jobs.json").exists():
            return self.project_skill_dir / "jobs.json"
        return JOBS_FILE

    def load(self) -> Dict[str, Any]:
        """加载作业记录"""
        jobs_file = self._get_jobs_file()
        if jobs_file.exists():
            try:
                with open(jobs_file, 'r', encoding='utf-8') as f:
                    return json.load(f)
            except json.JSONDecodeError:
                return {"jobs": []}
        return {"jobs": []}

    def save(self, data: Dict[str, Any]):
        """保存作业记录"""
        jobs_file = self._get_jobs_file()
        with open(jobs_file, 'w', encoding='utf-8') as f:
            json.dump(data, f, indent=2, ensure_ascii=False)

    def add_job(self, job_id: str, name: str, script: str,
                output_file: str, error_file: str = None):
        """添加作业记录"""
        data = self.load()
        job = {
            "job_id": job_id,
            "name": name,
            "script": script,
            "submitted_at": datetime.now().isoformat(),
            "status": "PENDING",
            "output_file": output_file,
            "error_file": error_file or output_file.replace(".out", ".err")
        }
        data["jobs"].append(job)
        self.save(data)
        print_info(f"作业已记录: {job_id}")

    def update_status(self, job_id: str, status: str):
        """更新作业状态"""
        data = self.load()
        for job in data["jobs"]:
            if job["job_id"] == job_id:
                job["status"] = status
                job["updated_at"] = datetime.now().isoformat()
                break
        self.save(data)

    def get_job(self, job_id: str) -> Optional[Dict[str, Any]]:
        """获取作业记录"""
        data = self.load()
        for job in data["jobs"]:
            if job["job_id"] == job_id:
                return job
        return None

    def list_jobs(self, since: str = None) -> List[Dict[str, Any]]:
        """列出作业"""
        data = self.load()
        jobs = data.get("jobs", [])
        if since:
            jobs = [j for j in jobs if j.get("submitted_at", "") >= since]
        return jobs


class PartitionCache:
    """分区节点信息缓存（支持项目级别和全局级别）"""

    def __init__(self, project_root: Optional[str] = None):
        self.project_root = Path(project_root) if project_root else _detect_project_root()
        self.project_skill_dir = (
            self.project_root / ".claude" / "skills" / "slurm-assistant"
            if self.project_root else None
        )
        # 确保配置目录存在
        SKILL_DIR.mkdir(parents=True, exist_ok=True)
        if self.project_skill_dir:
            self.project_skill_dir.mkdir(parents=True, exist_ok=True)

    def _get_cache_file(self) -> Path:
        """获取缓存文件路径（优先项目级别）"""
        if self.project_skill_dir and (self.project_skill_dir / "partition_cache.json").exists():
            return self.project_skill_dir / "partition_cache.json"
        return PARTITION_CACHE_FILE

    def load(self) -> Dict[str, Any]:
        """加载缓存"""
        cache_file = self._get_cache_file()
        if cache_file.exists():
            try:
                with open(cache_file, 'r', encoding='utf-8') as f:
                    return json.load(f)
            except json.JSONDecodeError:
                return {}
        return {}

    def save(self, data: Dict[str, Any]):
        """保存缓存"""
        cache_file = self._get_cache_file()
        with open(cache_file, 'w', encoding='utf-8') as f:
            json.dump(data, f, indent=2, ensure_ascii=False)

    def get_partitions(self, cluster_name: str) -> Optional[List[Dict[str, Any]]]:
        """获取集群的分区列表"""
        data = self.load()
        return data.get(cluster_name, {}).get("partitions")

    def set_partitions(self, cluster_name: str, partitions: List[Dict[str, Any]]):
        """设置集群的分区列表"""
        data = self.load()
        if cluster_name not in data:
            data[cluster_name] = {}
        data[cluster_name]["partitions"] = partitions
        data[cluster_name]["updated_at"] = datetime.now().isoformat()
        self.save(data)

    def get_nodes(self, cluster_name: str, partition: str) -> Optional[List[Dict[str, Any]]]:
        """获取分区下的节点列表"""
        data = self.load()
        cluster_data = data.get(cluster_name, {})
        partitions = cluster_data.get("partitions", [])
        for p in partitions:
            if p.get("name") == partition:
                return p.get("nodes", [])
        return None

    def is_cache_valid(self, cluster_name: str, max_age_hours: int = 24) -> bool:
        """检查缓存是否有效"""
        data = self.load()
        cluster_data = data.get(cluster_name)
        if not cluster_data:
            return False
        updated_at = cluster_data.get("updated_at")
        if not updated_at:
            return False
        try:
            from datetime import timedelta
            update_time = datetime.fromisoformat(updated_at)
            age = datetime.now() - update_time
            return age < timedelta(hours=max_age_hours)
        except:
            return False


class SlurmExecutor:
    """Slurm 命令执行器"""

    def __init__(self, config: Dict[str, Any]):
        self.config = config
        self.mode = config.get("mode", "local")

    def _detect_local_mode(self) -> bool:
        """检测是否在集群上"""
        # 检查 sinfo 命令是否可用
        result = shutil.which("sinfo")
        if result:
            return True
        # 尝试运行 sinfo
        try:
            subprocess.run(["sinfo", "--version"],
                         capture_output=True, check=True, timeout=5)
            return True
        except (subprocess.SubprocessError, FileNotFoundError):
            return False

    def _build_ssh_command(self, remote_cmd: str) -> List[str]:
        """构建 SSH 命令"""
        cluster = self.config.get("cluster", {})
        ssh_config = self.config.get("ssh", {})

        host = cluster.get("host", "")
        port = cluster.get("port", 22)
        username = cluster.get("username", "")
        jump_host = cluster.get("jump_host")

        ssh_cmd = ["ssh"]

        # 端口
        if port != 22:
            ssh_cmd.extend(["-p", str(port)])

        # 跳板机
        if jump_host:
            ssh_cmd.extend(["-J", jump_host])

        # 密钥
        key_path = ssh_config.get("key_path")
        if key_path and Path(key_path).exists():
            ssh_cmd.extend(["-i", key_path])

        # 用户名和主机
        if username:
            ssh_cmd.append(f"{username}@{host}")
        else:
            ssh_cmd.append(host)

        # 远程命令
        ssh_cmd.append(remote_cmd)

        return ssh_cmd

    def run(self, cmd: str, check: bool = True) -> subprocess.CompletedProcess:
        """执行命令（本地或远程）"""
        if self.mode == "local":
            # 本地模式：直接执行
            return subprocess.run(
                cmd,
                shell=True,
                capture_output=True,
                text=True,
                check=check
            )
        else:
            # 远程模式：通过 SSH 执行
            ssh_cmd = self._build_ssh_command(cmd)
            return subprocess.run(
                ssh_cmd,
                capture_output=True,
                text=True,
                check=check
            )

    def run_interactive(self, cmd: str) -> int:
        """执行交互式命令"""
        if self.mode == "local":
            return os.system(cmd)
        else:
            ssh_cmd = self._build_ssh_command(cmd)
            return subprocess.call(ssh_cmd)


class SlurmCLI:
    """Slurm CLI 主类"""

    def __init__(self, project_root: Optional[str] = None):
        self.project_root = project_root
        self.config_manager = ConfigManager(project_root=project_root)
        self.job_tracker = JobTracker(project_root=project_root)
        self.partition_cache = PartitionCache(project_root=project_root)
        self.config = self.config_manager.load()
        self.executor = SlurmExecutor(self.config)

    # ==================== 初始化和配置 ====================

    def cmd_init(self, args):
        """初始化配置"""
        # 检查模式
        if args.check:
            self._check_config_status(args.output_json)
            return

        # 如果提供了命令行参数，使用非交互式配置
        if args.mode or args.host or args.username:
            self._init_from_args(args)
            return

        # 否则使用交互式配置
        print_info("正在检测环境...")

        # 检测是否在集群上
        executor = SlurmExecutor({"mode": "local"})
        if executor._detect_local_mode():
            print_success("检测到当前已在 Slurm 集群上")
            self.config["mode"] = "local"
            self.config["cluster"]["name"] = input("请输入集群名称（可选）: ") or "本地集群"
            self.config_manager.save(self.config)
            print_success("配置完成！使用本地模式。")
            return

        print_info("未检测到 Slurm 环境，将配置远程连接")
        self._configure_remote()
        self._test_connection()

    def _check_config_status(self, output_json: bool = False):
        """检查配置状态"""
        config = self.config_manager.load()
        is_configured = self.config_manager.is_configured()
        config_sources = self.config_manager.get_config_sources()

        if output_json:
            status = {
                "configured": is_configured,
                "mode": config.get("mode"),
                "cluster": config.get("cluster", {}),
                "local_slurm_available": self._detect_local_slurm(),
                "config_sources": config_sources
            }
            print(json.dumps(status, indent=2, ensure_ascii=False))
        else:
            if is_configured:
                print_success(f"已配置，模式: {config.get('mode')}")
                # 显示配置来源
                if config_sources:
                    print_info(f"配置来源: {', '.join(config_sources)}")
                cluster = config.get("cluster", {})
                if cluster.get("name"):
                    print_info(f"集群: {cluster['name']}")
                if cluster.get("host"):
                    print_info(f"地址: {cluster['username']}@{cluster['host']}:{cluster.get('port', 22)}")
            else:
                print_info("尚未配置")

    def _detect_local_slurm(self) -> bool:
        """检测本地是否有 Slurm"""
        try:
            result = subprocess.run(
                ["sinfo", "--version"],
                capture_output=True, text=True, timeout=5
            )
            return result.returncode == 0
        except (subprocess.SubprocessError, FileNotFoundError):
            return False

    def _init_from_args(self, args):
        """从命令行参数初始化配置"""
        to_project = getattr(args, 'save_to_project', False)

        if args.mode == "local":
            self.config["mode"] = "local"
            self.config["cluster"]["name"] = args.cluster_name or "本地集群"
            self.config_manager.save(self.config, to_project=to_project)
            if to_project:
                print_success("配置完成！使用本地模式（项目级别）。")
            else:
                print_success("配置完成！使用本地模式。")
            return

        # 远程模式
        if not args.host or not args.username:
            print_error("远程模式需要 --host 和 --username 参数")
            sys.exit(1)

        self.config["mode"] = "remote"
        cluster = self.config.setdefault("cluster", {})
        cluster["name"] = args.cluster_name or "远程集群"
        cluster["host"] = args.host
        cluster["port"] = args.port or 22
        cluster["username"] = args.username
        cluster["jump_host"] = args.jump_host

        paths = self.config.setdefault("paths", {})
        paths["home_on_cluster"] = f"/home/{args.username}"

        self.config_manager.save(self.config, to_project=to_project)

        # 测试连接
        self._test_connection()

    def _configure_remote(self):
        """配置远程连接"""
        print(f"\n{Colors.BOLD}=== 配置远程集群连接 ==={Colors.RESET}\n")

        cluster = self.config.setdefault("cluster", {})
        ssh = self.config.setdefault("ssh", {})
        paths = self.config.setdefault("paths", {})

        # 集群名称
        cluster["name"] = input(f"集群名称 [{cluster.get('name', '我的集群')}]: ") or cluster.get("name", "我的集群")

        # 主机地址
        cluster["host"] = input(f"登录节点地址 [{cluster.get('host', '')}]: ") or cluster.get("host", "")
        if not cluster["host"]:
            print_error("必须提供主机地址")
            sys.exit(1)

        # 端口
        port_str = input(f"SSH 端口 [{cluster.get('port', 22)}]: ")
        cluster["port"] = int(port_str) if port_str else cluster.get("port", 22)

        # 用户名
        cluster["username"] = input(f"用户名 [{cluster.get('username', '')}]: ") or cluster.get("username", "")
        if not cluster["username"]:
            print_error("必须提供用户名")
            sys.exit(1)

        # 跳板机
        use_jump = confirm("是否需要跳板机？", default=False)
        if use_jump:
            cluster["jump_host"] = input("跳板机地址 (user@host:port): ") or None
        else:
            cluster["jump_host"] = None

        # 路径
        paths["home_on_cluster"] = input(f"集群上的 home 目录 [/{cluster['username']}]: ") or f"/home/{cluster['username']}"

        self.config["mode"] = "remote"
        self.config_manager.save(self.config)

    def _test_connection(self):
        """测试连接"""
        print_info("测试连接...")
        try:
            result = self.executor.run("echo 'Connection successful' && sinfo --version")
            print_success("连接成功！")
            print_info(f"Slurm 版本: {result.stdout.strip()}")
        except subprocess.CalledProcessError as e:
            print_error(f"连接失败: {e.stderr}")
            print_warning("请检查配置或运行 'slurm-cli.py setup-ssh' 配置密钥登录")

    def cmd_setup_ssh(self, args):
        """SSH 密钥配置引导"""
        print(f"\n{Colors.BOLD}=== SSH 密钥配置向导 ==={Colors.RESET}\n")

        ssh_dir = Path.home() / ".ssh"
        ssh_dir.mkdir(mode=0o700, exist_ok=True)

        default_key = ssh_dir / "id_rsa"

        # 检查是否已有密钥
        existing_keys = list(ssh_dir.glob("id_*")) + list(ssh_dir.glob("id_*.pub"))
        existing_keys = [k for k in existing_keys if ".pub" not in str(k) or str(k).endswith(".pub")]

        if existing_keys:
            print_info(f"发现已有密钥: {[k.name for k in existing_keys if not k.name.endswith('.pub')]}")
            if not confirm("是否生成新密钥？", default=False):
                key_path = input(f"使用哪个密钥 [{default_key}]: ") or str(default_key)
            else:
                key_path = self._generate_ssh_key()
        else:
            print_info("未发现 SSH 密钥")
            key_path = self._generate_ssh_key()

        # 配置 SSH config
        self._setup_ssh_config(key_path)

        # 复制公钥到集群
        self._copy_public_key(key_path)

        print_success("SSH 密钥配置完成！")

    def _generate_ssh_key(self) -> str:
        """生成 SSH 密钥"""
        key_path = input(f"密钥保存路径 [{Path.home() / '.ssh' / 'id_rsa'}]: ")
        if not key_path:
            key_path = str(Path.home() / ".ssh" / "id_rsa")

        key_type = input("密钥类型 [ed25519/rsa] (默认 ed25519): ") or "ed25519"

        cmd = ["ssh-keygen", "-t", key_type, "-f", key_path]
        if key_type == "rsa":
            cmd.extend(["-b", "4096"])

        print_info(f"正在生成 {key_type} 密钥...")
        subprocess.run(cmd)
        print_success(f"密钥已生成: {key_path}")

        return key_path

    def _setup_ssh_config(self, key_path: str):
        """配置 SSH config"""
        cluster = self.config.get("cluster", {})
        host = cluster.get("host", "")
        port = cluster.get("port", 22)
        username = cluster.get("username", "")
        jump_host = cluster.get("jump_host")

        if not host:
            print_warning("未配置集群信息，跳过 SSH config 配置")
            return

        config_path = Path.home() / ".ssh" / "config"
        entry_name = input(f"SSH config 条目名称 [{cluster.get('name', 'my-cluster')}]: ") or cluster.get("name", "my-cluster").replace(" ", "-").lower()

        entry = f"\n# {cluster.get('name', '集群')} - 由 slurm-assistant 配置\n"
        entry += f"Host {entry_name}\n"
        entry += f"    HostName {host}\n"
        entry += f"    User {username}\n"
        entry += f"    Port {port}\n"
        entry += f"    IdentityFile {key_path}\n"
        if jump_host:
            entry += f"    ProxyJump {jump_host}\n"

        if confirm(f"添加以下配置到 {config_path}？\n{entry}", default=True):
            with open(config_path, 'a') as f:
                f.write(entry)
            print_success("SSH config 已更新")

            # 更新配置
            self.config["ssh"]["config_entry"] = entry_name
            self.config_manager.save(self.config)

    def _copy_public_key(self, key_path: str):
        """复制公钥到集群"""
        pub_key_path = Path(key_path + ".pub")
        if not pub_key_path.exists():
            print_warning(f"未找到公钥文件: {pub_key_path}")
            return

        cluster = self.config.get("cluster", {})
        if not cluster.get("host"):
            print_warning("未配置集群信息，请手动复制公钥")
            return

        print_info(f"公钥内容: {pub_key_path.read_text().strip()}")
        if confirm("是否尝试自动复制公钥到集群？", default=True):
            print_info("请输入集群密码以复制公钥...")
            cmd = ["ssh-copy-id"]
            if cluster.get("port") != 22:
                cmd.extend(["-p", str(cluster["port"])])
            if cluster.get("jump_host"):
                cmd.extend(["-J", cluster["jump_host"]])
            cmd.append(f"{cluster['username']}@{cluster['host']}")
            result = subprocess.run(cmd)
            if result.returncode == 0:
                print_success("公钥已复制到集群")

                # 测试免密登录
                print_info("测试免密登录...")
                test_result = subprocess.run(
                    ["ssh", f"{cluster['username']}@{cluster['host']}",
                     "-p", str(cluster["port"]),
                     "echo '免密登录成功'"],
                    capture_output=True, text=True
                )
                if test_result.returncode == 0:
                    print_success("免密登录配置成功！")
            else:
                print_warning("自动复制失败，请手动将公钥添加到集群的 ~/.ssh/authorized_keys")

    # ==================== 资源查看 ====================

    def cmd_status(self, args):
        """查看资源状态"""
        self._check_configured()

        if args.nodes:
            self._show_nodes_status()
        elif args.partition:
            self._show_partition_status(args.partition)
        else:
            self._show_all_partitions()

    def _show_all_partitions(self):
        """显示所有分区状态"""
        print_info("查询分区状态...")
        try:
            result = self.executor.run("sinfo -o '%P %G %N %C %D'")
            print(f"\n{Colors.BOLD}分区状态:{Colors.RESET}")
            print(result.stdout)
        except subprocess.CalledProcessError as e:
            print_error(f"查询失败: {e.stderr}")

    def _show_partition_status(self, partition: str):
        """显示特定分区状态"""
        print_info(f"查询分区 {partition} 状态...")
        try:
            result = self.executor.run(f"sinfo -p {partition} -o '%N %C %G'")
            print(f"\n{Colors.BOLD}分区 {partition} 状态:{Colors.RESET}")
            print(result.stdout)
        except subprocess.CalledProcessError as e:
            print_error(f"查询失败: {e.stderr}")

    def _show_nodes_status(self):
        """显示所有节点状态"""
        print_info("查询节点状态...")
        try:
            result = self.executor.run("sinfo -N -o '%N %T %C %G'")
            print(f"\n{Colors.BOLD}节点状态:{Colors.RESET}")
            print(result.stdout)
        except subprocess.CalledProcessError as e:
            print_error(f"查询失败: {e.stderr}")

    def cmd_node_info(self, args):
        """查看节点详情"""
        self._check_configured()

        print_info(f"查询节点 {args.node} 详情...")
        try:
            result = self.executor.run(f"scontrol show node {args.node}")
            print(f"\n{Colors.BOLD}节点 {args.node} 详情:{Colors.RESET}")
            print(result.stdout)
        except subprocess.CalledProcessError as e:
            print_error(f"查询失败: {e.stderr}")

    # ==================== 交互式资源 ====================

    def cmd_alloc(self, args):
        """申请交互式资源"""
        self._check_configured()

        # 构建 salloc 命令
        cmd_parts = ["salloc"]

        if args.partition:
            cmd_parts.extend(["--partition", args.partition])
        if args.gres:
            cmd_parts.extend(["--gres", args.gres])
        if args.cpus:
            cmd_parts.extend(["--cpus-per-task", str(args.cpus)])
        if args.nodes:
            cmd_parts.extend(["--nodes", str(args.nodes)])
        if args.time:
            cmd_parts.extend(["--time", args.time])
        if args.mem:
            cmd_parts.extend(["--mem", args.mem])

        # 保活命令
        keep_alive = args.keep_alive or self.config.get("defaults", {}).get("keep_alive_duration", "24h")

        # 使用 tmux 运行 sleep 防止资源回收
        alloc_cmd = " ".join(cmd_parts)
        keep_cmd = f"tmux new-session -d -s slurm_keep 'sleep {keep_alive}'"

        print_info(f"申请资源: {alloc_cmd}")
        print_info(f"将在分配的节点上运行保活命令 (tmux + sleep {keep_alive})")

        # 先申请资源
        result = self.executor.run(alloc_cmd + " --test-only", check=False)
        if result.returncode != 0:
            print_warning(f"资源预检: {result.stderr}")

        if not confirm("确认申请资源？", default=True):
            print_info("已取消")
            return

        # 实际申请
        print_info("正在申请资源...")
        alloc_result = self.executor.run(alloc_cmd + " bash -c '" + keep_cmd + "'")

        if alloc_result.returncode == 0:
            print_success("资源已分配")
            print(alloc_result.stdout)
        else:
            print_error(f"申请失败: {alloc_result.stderr}")

    def cmd_release(self, args):
        """释放资源"""
        self._check_configured()

        # 危险操作确认
        print_warning(f"⚠️  危险操作：即将释放资源 {args.alloc_id}")
        if not confirm("确认释放？", default=False):
            print_info("已取消")
            return

        # 取消相关作业
        print_info(f"正在释放资源...")
        result = self.executor.run(f"scancel {args.alloc_id}", check=False)
        if result.returncode == 0:
            print_success("资源已释放")
        else:
            print_error(f"释放失败: {result.stderr}")

    # ==================== 运行命令 ====================

    def cmd_run(self, args):
        """使用 srun 运行命令"""
        self._check_configured()

        cmd_parts = ["srun"]

        if args.partition:
            cmd_parts.extend(["--partition", args.partition])
        if args.gres:
            cmd_parts.extend(["--gres", args.gres])
        if args.cpus:
            cmd_parts.extend(["--cpus-per-task", str(args.cpus)])
        if args.nodes:
            cmd_parts.extend(["--nodes", str(args.nodes)])
        if args.time:
            cmd_parts.extend(["--time", args.time])
        if args.mem:
            cmd_parts.extend(["--mem", args.mem])

        cmd_parts.append("--")
        cmd_parts.append(args.command)

        full_cmd = " ".join(cmd_parts)
        print_info(f"执行: {full_cmd}")

        # 交互式执行
        exit_code = self.executor.run_interactive(full_cmd)
        if exit_code == 0:
            print_success("命令执行完成")
        else:
            print_error(f"命令执行失败，退出码: {exit_code}")

    # ==================== 作业脚本 ====================

    def cmd_script_gen(self, args):
        """生成作业脚本"""
        script_name = args.name or "job"
        output_path = args.output or f"{script_name}.sh"

        print(f"\n{Colors.BOLD}=== 生成 Slurm 作业脚本 ==={Colors.RESET}\n")

        # 收集参数
        params = {
            "name": script_name,
            "partition": args.partition or input("分区 (留空使用默认): ") or None,
            "gres": args.gres or input("GRES (如 gpu:1，留空跳过): ") or None,
            "cpus": args.cpus or input("CPU 核心数 (留空使用默认): ") or None,
            "nodes": args.nodes or input("节点数 (默认 1): ") or "1",
            "output": f"logs/{script_name}_%j.out",
            "error": f"logs/{script_name}_%j.err",
        }

        # 时间和内存 - 只有明确要求才设置
        if args.time:
            params["time"] = args.time
        if args.mem:
            params["mem"] = args.mem

        # 其他选项
        params["mail_type"] = input("邮件通知类型 (如 ALL, END, 留空跳过): ") or None
        params["mail_user"] = input("邮箱地址 (留空跳过): ") or None

        # 命令
        print("\n请输入要执行的命令 (多行，输入空行结束):")
        commands = []
        while True:
            line = input("  ")
            if not line:
                break
            commands.append(line)

        if not commands:
            commands = ["# 在此处添加您的命令", "python your_script.py"]

        # 生成脚本
        script_content = self._generate_script_content(params, commands)

        # 确认保存位置
        output_path = input(f"\n保存路径 [{output_path}]: ") or output_path

        Path(output_path).write_text(script_content)
        print_success(f"脚本已生成: {output_path}")

        # 显示脚本内容
        if confirm("显示脚本内容？", default=True):
            print(f"\n{Colors.BOLD}--- 脚本内容 ---{Colors.RESET}")
            print(script_content)
            print(f"{Colors.BOLD}--- 结束 ---{Colors.RESET}\n")

        # 询问是否立即提交
        if confirm("立即提交作业？", default=False):
            self._submit_script(output_path)

    def _generate_script_content(self, params: Dict[str, Any], commands: List[str]) -> str:
        """生成脚本内容"""
        lines = ["#!/bin/bash", ""]

        # SBATCH 指令
        lines.append(f"#SBATCH --job-name={params['name']}")

        if params.get("partition"):
            lines.append(f"#SBATCH --partition={params['partition']}")
        if params.get("gres"):
            lines.append(f"#SBATCH --gres={params['gres']}")
        if params.get("cpus"):
            lines.append(f"#SBATCH --cpus-per-task={params['cpus']}")
        if params.get("nodes"):
            lines.append(f"#SBATCH --nodes={params['nodes']}")
        if params.get("time"):
            lines.append(f"#SBATCH --time={params['time']}")
        if params.get("mem"):
            lines.append(f"#SBATCH --mem={params['mem']}")

        lines.append(f"#SBATCH --output={params['output']}")
        lines.append(f"#SBATCH --error={params['error']}")

        if params.get("mail_type"):
            lines.append(f"#SBATCH --mail-type={params['mail_type']}")
        if params.get("mail_user"):
            lines.append(f"#SBATCH --mail-user={params['mail_user']}")

        lines.append("")
        lines.append("# ========== 环境设置 ==========")
        lines.append("")
        lines.append("# Python 环境选择 (优先级: uv > conda > module)")
        lines.append("# 方式 1: 使用 uv (推荐)")
        lines.append("# uv run python your_script.py")
        lines.append("# 或: uvx --with numpy --with pandas python script.py")
        lines.append("")
        lines.append("# 方式 2: 使用 conda")
        lines.append("# source ~/.bashrc && conda activate my_env")
        lines.append("")
        lines.append("# 方式 3: 使用模块系统")
        lines.append("# module load python/3.9 cuda/11.8")
        lines.append("")
        lines.append("# 设置工作目录")
        lines.append("cd $SLURM_SUBMIT_DIR")
        lines.append("")
        lines.append("# 创建日志目录")
        lines.append("mkdir -p logs")
        lines.append("")
        lines.append("# 打印作业信息")
        lines.append("echo \"Job ID: $SLURM_JOB_ID\"")
        lines.append("echo \"Running on: $(hostname)\"")
        lines.append("echo \"Start time: $(date)\"")
        lines.append("echo \"\"")
        lines.append("")
        lines.append("# ========== 主要任务 ==========")

        for cmd in commands:
            lines.append(cmd)

        lines.append("")
        lines.append("echo \"\"")
        lines.append("echo \"End time: $(date)\"")
        lines.append("")

        return "\n".join(lines)

    # ==================== 作业管理 ====================

    def cmd_submit(self, args):
        """提交作业"""
        self._check_configured()

        script_path = args.script
        if not Path(script_path).exists():
            print_error(f"脚本不存在: {script_path}")
            return

        self._submit_script(script_path)

    def _submit_script(self, script_path: str):
        """提交脚本并记录"""
        print_info(f"提交作业: {script_path}")

        try:
            result = self.executor.run(f"sbatch {script_path}")

            # 解析作业 ID
            output = result.stdout.strip()
            print(output)

            # 通常格式是 "Submitted batch job 12345"
            if "Submitted batch job" in output:
                job_id = output.split()[-1]

                # 提取脚本中的输出文件路径
                script_content = Path(script_path).read_text()
                output_file = self._extract_sbatch_param(script_content, "output") or f"slurm-{job_id}.out"
                error_file = self._extract_sbatch_param(script_content, "error") or output_file.replace(".out", ".err")

                # 记录作业
                self.job_tracker.add_job(
                    job_id=job_id,
                    name=Path(script_path).stem,
                    script=str(Path(script_path).absolute()),
                    output_file=output_file.replace("%j", job_id),
                    error_file=error_file.replace("%j", job_id)
                )

                print_success(f"作业已提交，ID: {job_id}")
                print_info(f"日志文件: {output_file.replace('%j', job_id)}")

        except subprocess.CalledProcessError as e:
            print_error(f"提交失败: {e.stderr}")

    def _extract_sbatch_param(self, content: str, param: str) -> Optional[str]:
        """从脚本中提取 SBATCH 参数"""
        import re
        pattern = rf"#SBATCH\s+--{param}=(.+)"
        match = re.search(pattern, content)
        return match.group(1).strip() if match else None

    def cmd_jobs(self, args):
        """查看作业状态"""
        self._check_configured()

        if args.id:
            self._show_job_detail(args.id)
        else:
            self._show_all_jobs()

    def _show_all_jobs(self):
        """显示所有作业"""
        print_info("查询作业状态...")
        try:
            result = self.executor.run("squeue -u $USER -o '%.10i %.20j %.8T %.10M %.6D %R'")
            print(f"\n{Colors.BOLD}您的作业:{Colors.RESET}")
            print(result.stdout)

            # 同时显示 skill 记录的作业
            tracked = self.job_tracker.list_jobs()
            if tracked:
                print(f"\n{Colors.BOLD}此 skill 提交的作业记录:{Colors.RESET}")
                for job in tracked[-10:]:  # 最近 10 个
                    status = job.get("status", "UNKNOWN")
                    status_color = Colors.GREEN if status == "COMPLETED" else (
                        Colors.YELLOW if status == "RUNNING" else Colors.BLUE
                    )
                    print(f"  {job['job_id']}: {job['name']} [{status_color}{status}{Colors.RESET}] @ {job['submitted_at']}")

        except subprocess.CalledProcessError as e:
            print_error(f"查询失败: {e.stderr}")

    def _show_job_detail(self, job_id: str):
        """显示作业详情"""
        print_info(f"查询作业 {job_id} 详情...")
        try:
            result = self.executor.run(f"scontrol show job {job_id}")
            print(f"\n{Colors.BOLD}作业 {job_id} 详情:{Colors.RESET}")
            print(result.stdout)

            # 显示 skill 记录的信息
            tracked = self.job_tracker.get_job(job_id)
            if tracked:
                print(f"\n{Colors.BOLD}Skill 记录:{Colors.RESET}")
                print(f"  脚本: {tracked.get('script')}")
                print(f"  输出: {tracked.get('output_file')}")
                print(f"  错误: {tracked.get('error_file')}")

        except subprocess.CalledProcessError as e:
            print_error(f"查询失败: {e.stderr}")

    def cmd_log(self, args):
        """查看作业日志"""
        self._check_configured()

        job_id = args.job_id

        # 先尝试从记录中获取日志路径
        tracked = self.job_tracker.get_job(job_id)
        if tracked:
            output_file = tracked.get("output_file")
            print_info(f"日志文件: {output_file}")
        else:
            # 使用默认路径
            output_file = f"slurm-{job_id}.out"
            print_info(f"使用默认日志路径: {output_file}")

        if args.follow:
            # 实时跟踪
            print_info("实时跟踪日志 (Ctrl+C 退出)...")
            self.executor.run_interactive(f"tail -f {output_file}")
        else:
            # 显示日志内容
            try:
                result = self.executor.run(f"cat {output_file}")
                print(f"\n{Colors.BOLD}=== 日志内容 ==={Colors.RESET}")
                print(result.stdout)
            except subprocess.CalledProcessError as e:
                print_error(f"读取日志失败: {e.stderr}")

    def cmd_cancel(self, args):
        """取消作业"""
        self._check_configured()

        job_ids = args.job_ids

        if args.all:
            # 取消所有作业
            print_warning("⚠️  危险操作：即将取消您的所有作业")
            if not confirm("确认取消所有作业？", default=False):
                print_info("已取消")
                return

            result = self.executor.run("scancel -u $USER")
            if result.returncode == 0:
                print_success("所有作业已取消")
                # 更新记录状态
                for job in self.job_tracker.list_jobs():
                    self.job_tracker.update_status(job["job_id"], "CANCELLED")
            else:
                print_error(f"取消失败: {result.stderr}")
            return

        # 取消指定作业
        for job_id in job_ids:
            # 获取作业信息用于确认
            tracked = self.job_tracker.get_job(job_id)

            print_warning(f"⚠️  危险操作：即将取消作业 {job_id}")
            if tracked:
                print(f"    作业名称：{tracked.get('name')}")
                print(f"    状态：{tracked.get('status')}")
                print(f"    提交时间：{tracked.get('submitted_at')}")

            if not confirm("确认取消？", default=False):
                print_info(f"跳过作业 {job_id}")
                continue

            result = self.executor.run(f"scancel {job_id}", check=False)
            if result.returncode == 0:
                print_success(f"作业 {job_id} 已取消")
                self.job_tracker.update_status(job_id, "CANCELLED")
            else:
                print_error(f"取消作业 {job_id} 失败: {result.stderr}")

    def cmd_history(self, args):
        """查看作业历史"""
        jobs = self.job_tracker.list_jobs(since=args.since)

        if not jobs:
            print_info("暂无作业记录")
            return

        print(f"\n{Colors.BOLD}作业历史记录:{Colors.RESET}\n")
        print(f"{'ID':<10} {'名称':<20} {'状态':<12} {'提交时间':<20} {'脚本'}")
        print("-" * 80)

        for job in jobs:
            status = job.get("status", "UNKNOWN")
            status_color = Colors.GREEN if status == "COMPLETED" else (
                Colors.YELLOW if status in ("RUNNING", "PENDING") else Colors.RED
            )
            print(f"{job['job_id']:<10} {job['name']:<20} {status_color}{status:<12}{Colors.RESET} {job['submitted_at'][:19]:<20} {job.get('script', 'N/A')}")

        if args.export:
            export_path = args.export
            Path(export_path).write_text(json.dumps({"jobs": jobs}, indent=2, ensure_ascii=False))
            print_success(f"历史记录已导出到: {export_path}")

    # ==================== 分区缓存管理 ====================

    def cmd_refresh_cache(self, args):
        """刷新分区节点缓存"""
        self._check_configured()

        cluster_name = self.config.get("cluster", {}).get("name", "default")
        print_info(f"正在获取 {cluster_name} 集群的分区节点信息...")

        try:
            partitions = self._fetch_all_partitions()
            if not partitions:
                print_warning("未获取到任何分区信息")
                return

            self.partition_cache.set_partitions(cluster_name, partitions)
            print_success(f"缓存已更新，共 {len(partitions)} 个分区")

            # 显示简要信息
            for p in partitions:
                nodes = p.get("nodes", [])
                gpu_nodes = [n for n in nodes if n.get("gres") and "gpu" in n.get("gres", "").lower()]
                print(f"  - {p['name']}: {len(nodes)} 节点, {len(gpu_nodes)} GPU 节点")

        except subprocess.CalledProcessError as e:
            print_error(f"获取分区信息失败: {e.stderr}")

    def _fetch_all_partitions(self) -> List[Dict[str, Any]]:
        """获取所有分区和节点信息"""
        partitions = []

        # 获取所有分区
        result = self.executor.run("sinfo -o '%P' --noheader", check=True)
        partition_lines = result.stdout.strip().split('\n')

        for part_line in partition_lines:
            if not part_line.strip():
                continue

            part_name = part_line.strip().rstrip('*')

            # 获取分区详情
            partition_data = {
                "name": part_name,
                "nodes": []
            }

            # 获取该分区的所有节点
            node_result = self.executor.run(f"sinfo -N -p {part_name} -o '%N' --noheader", check=False)
            if node_result.returncode == 0:
                node_names = node_result.stdout.strip().split('\n')
                node_names = [n.strip() for n in node_names if n.strip()]

                # 获取每个节点的详细信息
                for node_name in node_names:
                    node_info = self._fetch_node_info(node_name)
                    partition_data["nodes"].append(node_info)

            partitions.append(partition_data)

        return partitions

    def _fetch_node_info(self, node_name: str) -> Dict[str, Any]:
        """获取单个节点的硬件配置信息（不包含状态）"""
        node_info = {
            "name": node_name,
            "cpus": None,
            "memory": None,
            "gres": None,
            "gres_detail": None
        }

        try:
            result = self.executor.run(f"scontrol show node {node_name}", check=True)
            output = result.stdout

            # 解析 scontrol 输出，只获取硬件配置
            for line in output.split('\n'):
                line = line.strip()
                if 'RealMemory=' in line:
                    try:
                        mem_mb = int(line.split('RealMemory=')[1].split()[0])
                        node_info["memory"] = f"{mem_mb // 1024}G"
                    except:
                        pass
                elif 'Cores=' in line:
                    try:
                        cores = line.split('Cores=')[1].split()[0]
                        node_info["cpus"] = cores
                    except:
                        pass
                elif 'Gres=' in line:
                    # Gres=gpu:2(S:0-1) 或 Gres=gpu:a100:4(S:0-3)
                    gres = line.split('Gres=')[1].split()[0]
                    node_info["gres"] = gres
                    # 尝试获取更多 GPU 详情
                    if 'gpu:' in gres.lower():
                        node_info["gres_detail"] = self._parse_gpu_gres(gres)

        except subprocess.CalledProcessError:
            pass

        return node_info

    def _parse_gpu_gres(self, gres: str) -> Dict[str, Any]:
        """解析 GPU GRES 字符串，获取 GPU 详情"""
        # 示例: gpu:2(S:0-1) 或 gpu:a100:4(S:0-3)
        gpu_detail = {"type": "unknown", "count": 0}

        if "gpu:" not in gres.lower():
            return gpu_detail

        # 提取 GPU 数量和类型
        parts = gres.split('(')[0].strip()  # 去掉 (S:0-1) 部分
        if ':' in parts:
            # 有 GPU 类型，如 gpu:a100:4
            _, gpu_type, count_str = parts.split(':')
            gpu_detail["type"] = gpu_type
            gpu_detail["count"] = int(count_str)
        else:
            # 只有 gpu:N，如 gpu:2
            _, count_str = parts.split(':')
            gpu_detail["count"] = int(count_str)

        return gpu_detail

    def cmd_show_cache(self, args):
        """显示分区节点缓存"""
        self._check_configured()

        cluster_name = self.config.get("cluster", {}).get("name", "default")

        if not self.partition_cache.is_cache_valid(cluster_name):
            print_warning("缓存已过期或不存在，请先运行 refresh-cache 命令更新缓存")
            print_info("运行: python3 ~/.claude/skills/slurm-assistant/scripts/slurm-cli.py refresh-cache")
            return

        partitions = self.partition_cache.get_partitions(cluster_name)
        if not partitions:
            print_info("暂无缓存数据")
            return

        # 显示缓存信息
        cache_data = self.partition_cache.load()
        updated_at = cache_data.get(cluster_name, {}).get("updated_at", "unknown")

        print(f"\n{Colors.BOLD}集群分区节点信息{Colors.RESET}")
        print(f"集群: {cluster_name}")
        print(f"更新时间: {updated_at}")
        print(f"分区数量: {len(partitions)}\n")

        for p in partitions:
            part_name = p.get("name")
            nodes = p.get("nodes", [])

            # 统计 GPU 节点
            gpu_nodes = [n for n in nodes if n.get("gres") and "gpu" in n.get("gres", "").lower()]

            print(f"{Colors.BOLD}分区: {part_name}{Colors.RESET}")
            print(f"  总节点数: {len(nodes)}")
            print(f"  GPU 节点数: {len(gpu_nodes)}")

            if gpu_nodes:
                print(f"\n  {Colors.BLUE}GPU 节点详情:{Colors.RESET}")
                for node in gpu_nodes:
                    self._print_node_info(node, indent=4)
            print()

    def _print_node_info(self, node: Dict[str, Any], indent: int = 0):
        """打印节点硬件配置信息"""
        prefix = " " * indent
        name = node.get("name", "unknown")
        cpus = node.get("cpus", "?")
        memory = node.get("memory", "?")
        gres = node.get("gres", "")
        gres_detail = node.get("gres_detail", {})

        print(f"{prefix}{Colors.BOLD}{name}{Colors.RESET}")

        if gres_detail:
            gpu_type = gres_detail.get("type", "unknown")
            gpu_count = gres_detail.get("count", 0)
            print(f"{prefix}  GPU: {gpu_type} x {gpu_count}")
        else:
            print(f"{prefix}  GRES: {gres}")

        print(f"{prefix}  CPU: {cpus} 核心")
        print(f"{prefix}  内存: {memory}")

    def cmd_find_gpu(self, args):
        """查找可用 GPU 资源"""
        self._check_configured()

        cluster_name = self.config.get("cluster", {}).get("name", "default")
        partitions = self.partition_cache.get_partitions(cluster_name)

        if not partitions:
            print_warning("缓存未初始化，正在自动刷新...")
            self.cmd_refresh_cache(args)
            partitions = self.partition_cache.get_partitions(cluster_name)

        # 查找符合要求的 GPU 节点
        req_count = args.count
        req_type = args.type.lower() if args.type else None

        print(f"\n{Colors.BOLD}查找 GPU 资源:{Colors.RESET}")
        print(f"需求: {req_type or '任意'} GPU x {req_count}\n")

        found_nodes = []

        for p in partitions:
            nodes = p.get("nodes", [])
            for node in nodes:
                gres_detail = node.get("gres_detail", {})
                if not gres_detail or gres_detail.get("count", 0) == 0:
                    continue

                gpu_type = gres_detail.get("type", "unknown")
                gpu_count = gres_detail.get("count", 0)

                # 检查 GPU 数量是否满足
                if gpu_count < req_count:
                    continue

                # 检查 GPU 类型是否匹配
                if req_type is not None and req_type not in gpu_type.lower():
                    continue

                # 动态查询节点当前状态
                node_state = self._get_node_state(node["name"])
                is_available = "idle" in node_state.lower() or "mix" in node_state.lower()

                if is_available:
                    found_nodes.append({
                        "partition": p.get("name"),
                        "node": node,
                        "state": node_state,
                        "available_gpus": gpu_count,
                        "gpu_type": gpu_type
                    })

        if found_nodes:
            print_success(f"找到 {len(found_nodes)} 个符合条件的节点:\n")
            for item in found_nodes:
                node = item["node"]
                state = item["state"]
                state_color = Colors.GREEN if "idle" in state.lower() or "mix" in state.lower() else Colors.RED
                print(f"  {Colors.BOLD}{node['name']}{Colors.RESET} [{item['partition']}]")
                print(f"    GPU: {item['gpu_type']} x {item['available_gpus']}")
                print(f"    状态: {state_color}{state}{Colors.RESET}")
                print(f"    内存: {node.get('memory')}")
                print(f"    CPU: {node.get('cpus')} 核心")
                print()
        else:
            print_warning("未找到符合要求的 GPU 节点")
            print_info("可用选项:")
            print_info("  - 查看所有分区: slurm-cli.py show-cache")
            print_info("  - 刷新缓存: slurm-cli.py refresh-cache")

    def _get_node_state(self, node_name: str) -> str:
        """动态获取节点当前状态"""
        try:
            result = self.executor.run(f"sinfo -N -n {node_name} -o '%T' --noheader", check=True)
            return result.stdout.strip()
        except subprocess.CalledProcessError:
            return "unknown"

    # ==================== 辅助方法 ====================

    def _check_configured(self):
        """检查是否已配置"""
        if not self.config_manager.is_configured():
            print_error("尚未配置集群信息")
            print_info("请运行: slurm-cli.py init")
            sys.exit(1)


def main():
    parser = argparse.ArgumentParser(
        description="Slurm 集群助手 - 统一命令接口",
        formatter_class=argparse.RawDescriptionHelpFormatter
    )

    subparsers = parser.add_subparsers(dest="command", help="可用命令")

    # init
    init_parser = subparsers.add_parser("init", help="初始化配置")
    init_parser.add_argument("--mode", choices=["local", "remote"], help="运行模式")
    init_parser.add_argument("--cluster-name", help="集群名称")
    init_parser.add_argument("--host", help="登录节点地址")
    init_parser.add_argument("--port", type=int, help="SSH 端口")
    init_parser.add_argument("--username", help="用户名")
    init_parser.add_argument("--jump-host", help="跳板机地址")
    init_parser.add_argument("--check", action="store_true", help="仅检查配置状态")
    init_parser.add_argument("--output-json", action="store_true", help="以 JSON 格式输出配置状态")
    init_parser.add_argument("--project-root", help="项目根目录（自动检测）")
    init_parser.add_argument("--save-to-project", action="store_true", help="保存配置到项目级别")

    # setup-ssh
    ssh_parser = subparsers.add_parser("setup-ssh", help="SSH 密钥配置引导")

    # status
    status_parser = subparsers.add_parser("status", help="查看资源状态")
    status_parser.add_argument("--partition", "-p", help="指定分区")
    status_parser.add_argument("--nodes", "-n", action="store_true", help="显示节点状态")

    # node-info
    node_parser = subparsers.add_parser("node-info", help="查看节点详情")
    node_parser.add_argument("node", help="节点名称")

    # alloc
    alloc_parser = subparsers.add_parser("alloc", help="申请交互式资源")
    alloc_parser.add_argument("--partition", "-p", help="分区")
    alloc_parser.add_argument("--gres", "-g", help="GRES (如 gpu:1)")
    alloc_parser.add_argument("--cpus", "-c", type=int, help="CPU 核心数")
    alloc_parser.add_argument("--nodes", "-N", type=int, help="节点数")
    alloc_parser.add_argument("--time", "-t", help="时间限制")
    alloc_parser.add_argument("--mem", "-m", help="内存")
    alloc_parser.add_argument("--keep-alive", "-k", help="保活时间 (默认 24h)")

    # release
    release_parser = subparsers.add_parser("release", help="释放资源")
    release_parser.add_argument("alloc_id", help="资源/作业 ID")

    # run
    run_parser = subparsers.add_parser("run", help="运行命令")
    run_parser.add_argument("command", help="要执行的命令")
    run_parser.add_argument("--partition", "-p", help="分区")
    run_parser.add_argument("--gres", "-g", help="GRES")
    run_parser.add_argument("--cpus", "-c", type=int, help="CPU 核心数")
    run_parser.add_argument("--nodes", "-N", type=int, help="节点数")
    run_parser.add_argument("--time", "-t", help="时间限制")
    run_parser.add_argument("--mem", "-m", help="内存")

    # script-gen
    script_parser = subparsers.add_parser("script-gen", help="生成作业脚本")
    script_parser.add_argument("--name", "-n", help="作业名称")
    script_parser.add_argument("--output", "-o", help="输出文件路径")
    script_parser.add_argument("--partition", "-p", help="分区")
    script_parser.add_argument("--gres", "-g", help="GRES")
    script_parser.add_argument("--cpus", "-c", type=int, help="CPU 核心数")
    script_parser.add_argument("--nodes", "-N", type=int, help="节点数")
    script_parser.add_argument("--time", "-t", help="时间限制 (仅在明确指定时使用)")
    script_parser.add_argument("--mem", "-m", help="内存 (仅在明确指定时使用)")

    # submit
    submit_parser = subparsers.add_parser("submit", help="提交作业")
    submit_parser.add_argument("script", help="脚本路径")

    # jobs
    jobs_parser = subparsers.add_parser("jobs", help="查看作业状态")
    jobs_parser.add_argument("--id", help="作业 ID")

    # log
    log_parser = subparsers.add_parser("log", help="查看作业日志")
    log_parser.add_argument("job_id", help="作业 ID")
    log_parser.add_argument("--follow", "-f", action="store_true", help="实时跟踪")

    # cancel
    cancel_parser = subparsers.add_parser("cancel", help="取消作业")
    cancel_parser.add_argument("job_ids", nargs="*", help="作业 ID")
    cancel_parser.add_argument("--all", "-a", action="store_true", help="取消所有作业")

    # history
    history_parser = subparsers.add_parser("history", help="查看作业历史")
    history_parser.add_argument("--since", help="起始时间")
    history_parser.add_argument("--export", help="导出到文件")

    # refresh-cache
    refresh_cache_parser = subparsers.add_parser("refresh-cache", help="刷新分区节点缓存")

    # show-cache
    show_cache_parser = subparsers.add_parser("show-cache", help="显示分区节点缓存")

    # find-gpu
    find_gpu_parser = subparsers.add_parser("find-gpu", help="查找可用 GPU 资源")
    find_gpu_parser.add_argument("--count", "-c", type=int, default=1, help="需要的 GPU 数量 (默认 1)")
    find_gpu_parser.add_argument("--type", "-t", help="GPU 类型 (如 a100, v100, 3090)")

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        return

    # 获取项目根目录（如果提供了）
    project_root = getattr(args, 'project_root', None)
    cli = SlurmCLI(project_root=project_root)

    # 路由到对应命令
    cmd_map = {
        "init": cli.cmd_init,
        "setup-ssh": cli.cmd_setup_ssh,
        "status": cli.cmd_status,
        "node-info": cli.cmd_node_info,
        "alloc": cli.cmd_alloc,
        "release": cli.cmd_release,
        "run": cli.cmd_run,
        "script-gen": cli.cmd_script_gen,
        "submit": cli.cmd_submit,
        "jobs": cli.cmd_jobs,
        "log": cli.cmd_log,
        "cancel": cli.cmd_cancel,
        "history": cli.cmd_history,
        "refresh-cache": cli.cmd_refresh_cache,
        "show-cache": cli.cmd_show_cache,
        "find-gpu": cli.cmd_find_gpu,
    }

    if args.command in cmd_map:
        cmd_map[args.command](args)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
