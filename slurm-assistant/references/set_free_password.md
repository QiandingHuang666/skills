# 配置 SSH 免密登录

## 前置条件

- 本地已安装 OpenSSH 客户端
- 知道集群的用户名和地址

## 配置步骤

### 1. 检查本地是否已有 SSH 密钥对

```bash
ls -la ~/.ssh/id_*.pub
```

如果没有，生成新的密钥对：

```bash
ssh-keygen -t ed25519 -C "your_email@example.com"
```

按 Enter 使用默认路径，可设置空密码（直接按 Enter）。

### 2. 将公钥上传到集群

**方法 A：使用 ssh-copy-id（推荐）**

```bash
ssh-copy-id -p 端口 用户名@集群地址
```

**方法 B：手动上传**

1. 查看公钥内容：
```bash
cat ~/.ssh/id_ed25519.pub
# 或
cat ~/.ssh/id_rsa.pub
```

2. 登录集群，执行：
```bash
ssh -p 端口 用户名@集群地址
```

3. 在集群上执行：
```bash
mkdir -p ~/.ssh && chmod 700 ~/.ssh
echo "你的公钥内容" >> ~/.ssh/authorized_keys
chmod 600 ~/.ssh/authorized_keys
```

### 3. 验证免密登录

```bash
ssh -p 端口 用户名@集群地址 "echo 连接成功"
```

如果输出"连接成功"，则配置完成。

## 常见问题

### 仍然需要密码？

1. 检查权限：
```bash
# 本地
ls -la ~/.ssh/id_* 
# 应该是 600 或 400

# 集群上
ls -la ~/.ssh/authorized_keys
# 应该是 600
ls -la ~/.ssh
# 应该是 700
```

2. 检查 SSH 配置：
```bash
# 集群上的 /etc/ssh/sshd_config 应包含：
PubkeyAuthentication yes
```

3. 查看 SSH 日志：
```bash
ssh -v -p 端口 用户名@集群地址
```
