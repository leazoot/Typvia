# Typvia 同步服务器 · Cloudflare Tunnel 部署指南

零公网 IP、零端口暴露的自托管路径:服务器只在 compose 内网监听,cloudflared 反向连到 Cloudflare 边缘,TLS 由 Cloudflare 终结。服务器只存密文与元数据,Cloudflare 与主机都看不到明文。

## 前置

- 一台能跑 Docker Compose 的主机(NAS/家用机/VPS 均可,无需公网 IP)。
- 一个托管在 Cloudflare 的域名(免费计划即可)。

## 步骤

1. **创建隧道**:Cloudflare 仪表盘 → Zero Trust → Networks → Tunnels → Create a tunnel(Cloudflared 类型)→ 命名(如 `typvia-sync`)→ 复制 token。
2. **配置公共主机名**:在该隧道的 Public Hostname 里添加,例如 `sync.example.com` → Service 填 `http://typvia-sync:8787`(compose 服务名即内网主机名)。
3. **落地文件**:把本目录复制到部署主机;`cp .env.example .env`,把 token 填入 `TUNNEL_TOKEN`(`.env` 不入任何版本库)。
4. **启动**:`docker compose up -d --build`。
5. **首连校验**:`curl https://sync.example.com/healthz`(或按服务器版本的健康端点)应返回成功;客户端「同步设置 → 自建服务器」填 `https://sync.example.com`。

## 运维注意

- **备份**:数据都在 `sync-data` 卷的单个 SQLite 库(WAL 模式)。备份用 `docker compose exec typvia-sync` 不可行(distroless 无 shell),直接对卷做文件级备份前先 `docker compose stop typvia-sync`,或使用主机侧 sqlite3 的 `.backup`(卷路径见 `docker volume inspect`)。
- **升级**:拉新代码后 `docker compose up -d --build`;迁移在启动时自动执行、失败整体回滚(服务端事务纪律)。
- **端口**:不要为 typvia-sync 添加 `ports:` 映射——隧道是唯一入口,这正是本形态的安全边界。
- **token 泄露**:在 Cloudflare 仪表盘轮换隧道 token 并更新 `.env` 重启即可;token 只授权隧道连接,不授权数据访问(数据访问仍需客户端账户密钥)。

## 与 Workers 的关系

本模板不使用 Workers:服务器是有状态 SQLite 单二进制,Workers 无持久文件系统,改写等于新服务端项目。
