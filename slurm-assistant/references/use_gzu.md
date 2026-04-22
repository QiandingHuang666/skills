# 贵州大学 HPC 集群配置

## 遴集信息（自动填充）

- **地址**: 210.40.56.85
- **端口**: 21563

## 初始化流程

### 1. 选择集群

```json
{
  "questions": [
    {
      "question": "请选择您要连接的集群:",
      "options": ["贵州大学 HPC"]
    }
  ]
}
```

### 2. 询问用户名

```json
{
  "questions": [
    {
      "question": "请输入您的贵州大学 HPC 集群用户名:",
      "options": ["在此输入用户名"]
    }
  ]
}
```

### 3. 询问免密登录状态

```json
{
  "questions": [
    {
      "question": "您是否已配置免密登录?",
      "options": ["已配置", "未配置"]
    }
  ]
}
```

连接创建时直接使用 Rust client：

```bash
slurm-client connection add \
  --label gzu-cluster \
  --host 210.40.56.85 \
  --port 21563 \
  --user "<用户名>" \
  --kind cluster \
  --json
```

---

## 路径映射（重要）

贵州大学 HPC 提供三种访问方式，路径映射如下：

| 环境 | 个人目录 | 项目目录 | 公共集群目录 |
|------|----------|----------|--------------|
| **容器实例** | `/home/<username>` | `/groups/<project>/home/<username>` | `/groups/public_cluster/home/<username>` |
| **虚拟机实例** | `/webdav/MyData` | `/webdav/ProjectGroup(<project>)` | `/webdav/ProjectGroup(public_cluster)` |
| **公共集群** | `/users/<username>` | `/groups/<project id>/home/<username>` | `/home/<username>` |
| **WEB 页面** | 独占实例数据-<username> | 共享实例数据-<project显示名> | 共享实例数据-public_cluster |

### 使用场景

1. **容器实例**：通过 SSH 连接的实例（如 21810 端口），路径与集群略有不同
2. **虚拟机实例**：通过 WebDAV 访问，路径结构完全不同
3. **公共集群**：通过 SSH 连接集群（21563 端口），标准的 Slurm 环境

### AI 注意事项

当用户提到"实例"时，需要确认是容器实例还是虚拟机实例：
- 容器实例：SSH 连接，路径以 `/home/` 开头
- 虚拟机实例：WebDAV 访问，路径以 `/webdav/` 开头

---

## 路径映射验证（标准命令模板）

涉及贵州大学“实例”时，连接验证通过后先执行路径映射验证，再做安装/软链接/数据写操作。

### A. 容器实例（SSH）

先验证个人目录和公共集群映射目录：

```bash
slurm-client exec --connection <connection_id> --cmd 'whoami; ls -ld /home/<username> /groups/public_cluster/home/<username>' --json
```

再验证公共工具路径（如需安装 conda/uv）：

```bash
slurm-client exec --connection <connection_id> --cmd 'ls -ld /groups/public_cluster/home/share/Official /groups/public_cluster/home/share/Official/tools' --json
```

### B. 虚拟机实例（WebDAV）

先验证 WebDAV 根目录：

```bash
slurm-client exec --connection <connection_id> --cmd 'ls -ld /webdav /webdav/MyData "/webdav/ProjectGroup(public_cluster)"' --json
```

如果上面的引号路径在当前 shell 不兼容，可改为：

```bash
slurm-client exec --connection <connection_id> --cmd "ls -ld /webdav /webdav/MyData /webdav/ProjectGroup\\(public_cluster\\)" --json
```

### C. 公共集群（21563）

验证公共集群个人目录：

```bash
slurm-client exec --connection <connection_id> --cmd 'whoami; ls -ld /home/<username> /users/<username>' --json
```

---

## 输出要求（必须遵循）

- 输出中必须明确写出本次确认后的“个人目录 / 公共目录 / 项目目录”实际路径
- 若路径不存在或权限不足，先报告并停止后续重操作，不能继续猜测路径

---

## 相关文档

- 公共资源检查: `gzu_public_resources.md`
- 免密登录配置: `set_free_password.md`
