# Typvia 同步服务 · Cloudflare Tunnel

[English](guide.md) · 简体中文

一条不需要公网 IP、也不暴露任何端口的自托管路径:服务端只监听 compose 内部网络,
`cloudflared` 主动向 Cloudflare 边缘拨出连接,TLS 在 Cloudflare 侧终结。服务端只存密文与
元数据,所以 Cloudflare 和主机都读不到你的片段。

## 前置条件

- 一台能跑 Docker Compose 的机器 —— NAS、家里的电脑或 VPS 都行。**不需要公网 IP**。
- 一个托管在 Cloudflare 的域名。免费套餐就够。

## 步骤

1. **创建 tunnel** —— Cloudflare 控制台 → Zero Trust → Networks → Tunnels → Create a
   tunnel(选 Cloudflared 类型)→ 起个名字,比如 `typvia-sync` → 复制 token。
2. **添加公开主机名** —— 在该 tunnel 的 Public Hostname 标签页里,把比如
   `sync.example.com` 映射到服务 `http://typvia-sync:8787`。compose 里的服务名就是内部
   主机名。
3. **把文件放到主机上** —— 把本目录拷过去,然后 `cp .env.example .env`,把
   `TUNNEL_TOKEN` 设成你的 token。`.env` 永远不提交到任何仓库。
4. **启动** —— `docker compose up -d --build`。
5. **验证** —— `curl https://sync.example.com/healthz` 应当成功。客户端里进入
   同步设置 → 自托管服务器,填 `https://sync.example.com`。

## 运维

- **备份** —— 数据就是 `sync-data` 卷里的单个 SQLite 数据库(WAL 模式)。
  `docker compose exec` 帮不上忙:镜像是 distroless 的,没有 shell。先停掉服务再在文件层面
  备份该卷,或者从宿主机跑 `sqlite3 .backup`(卷的路径用 `docker volume inspect` 查)。
- **升级** —— 拉取新代码后 `docker compose up -d --build`。迁移在启动时于事务内执行,
  失败整体回滚。
- **端口** —— **不要**给 `typvia-sync` 加 `ports:` 映射。「tunnel 是唯一入口」正是这套方案的
  安全边界。
- **token 泄露了怎么办** —— 在 Cloudflare 控制台轮换 tunnel token,更新 `.env` 后重启。
  该 token 授权的是一条 tunnel 连接,不是数据访问权;读取数据仍然需要你设备上持有的账户密钥。

## 为什么不用 Workers

服务端是一个有状态的单二进制,数据落在 SQLite 上,而 Workers 没有持久文件系统。移植过去
等于重写一个不同的服务端,而不是部署这一个。
