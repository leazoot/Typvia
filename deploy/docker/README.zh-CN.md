# Typvia 同步服务 · Docker

[English](README.md) · 简体中文

两个文件,两种跑法:

- **`compose.image.yml`** —— 给部署主机用。运行已发布的镜像,主机上**不需要源码、不需要
  Go、不需要任何构建工具链**。
- **`docker-compose.yml`** —— 给开发机用。从本仓库源码构建镜像,然后运行。

想让远程可访问又完全不暴露端口,见 [`../cloudflare/`](../cloudflare/guide.zh-CN.md)
(Cloudflare Tunnel)。

## 部署主机:拉取并运行

镜像由 `.github/workflows/publish-sync-server.yml` 推送到 GHCR,名为
`ghcr.io/<owner>/typvia-sync`,标签有 `latest`(默认分支)、`sha-xxxxxxx`(每次提交)
以及 `1.2.3` / `1.2`(推送 `v1.2.3` 标签时)。你要是跑自己的 fork,把 owner 换成你自己的。

```sh
mkdir -p ~/typvia-sync && cd ~/typvia-sync
curl -fsSL -o docker-compose.yml \
  https://raw.githubusercontent.com/leazoot/Typvia/main/deploy/docker/compose.image.yml
echo 'TYPVIA_IMAGE=ghcr.io/leazoot/typvia-sync:latest' > .env
docker compose up -d
curl http://127.0.0.1:8787/healthz    # {"status":"ok"}
```

生产环境请把 `latest` 换成确定的版本(`:1.2.3`)或提交(`:sha-xxxxxxx`)——这样回滚只是改
一行加一次 `up -d`。

**GHCR 的 package 默认是私有的。** 首次推送之后,去 GitHub → Packages → 对应 package →
Package settings → Change visibility → Public,主机才能匿名拉取。想保持私有的话,先在主机
上登录:

```sh
echo <带 read:packages 权限的 PAT> | docker login ghcr.io -u <owner> --password-stdin
```

镜像是多架构的(linux/amd64 + linux/arm64),x86 的 VPS 和 arm 的 NAS 拉同一个标签即可。

## 开发机:从源码构建

这条命令要在**仓库根目录**执行 —— 镜像需要 `apps/sync-server/`,所以构建上下文是仓库根:

```sh
docker compose -f deploy/docker/docker-compose.yml up -d --build
```

## 连接客户端

设置 → 同步 → 设置同步 → 自托管服务器,填入服务器地址。服务端没有账号可注册:第一台设备
创建账户并注册自己,之后每台设备都用配对码加入。

## 只要二进制,不用容器

```sh
cd apps/sync-server && go run ./cmd/typvia-sync-server
```

它监听 `127.0.0.1:8787`,把 `typvia-sync.db` 写在工作目录下。全部配置就是两个环境变量——
`TYPVIA_SYNC_LISTEN` 和 `TYPVIA_SYNC_DB`(`-listen` / `-db` 命令行参数优先)。交叉编译一个
静态二进制丢到服务器上只需一行,因为 SQLite 驱动是纯 Go 的,产物没有任何依赖:

```sh
GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build ./cmd/typvia-sync-server
```

## 让其他设备访问得到

两个 compose 文件都把端口绑在 `127.0.0.1` 上,这是有意的。客户端对服务器地址做严格校验——
**https,或者 loopback 上的 http**——并且从不关闭证书验证。所以要让别的设备访问到,只有两条路:

- 在前面放 Caddy 或 nginx 终结 TLS 并反向代理到 `127.0.0.1:8787`,端口映射仍然留在
  loopback 上;或者
- 换用 [`../cloudflare/`](../cloudflare/guide.zh-CN.md),它完全不开端口。把那份 compose 文件里的
  `build:` 换成 `image: ${TYPVIA_IMAGE}`,它同样不需要源码。

直接绑到 `0.0.0.0` 只会得到一个明文监听端口,客户端根本不会跟它说话。

## 运维

- **备份** —— 数据就是 `sync-data` 卷里的单个 SQLite 数据库(WAL 模式)。镜像是 distroless
  的,没有 shell 可以 exec 进去:用 `docker compose stop` 停掉服务后在文件层面备份该卷,或者
  从宿主机跑 `sqlite3 .backup`(卷的路径用 `docker volume inspect` 查)。
- **升级** —— `docker compose pull && docker compose up -d`(从源码构建的那份用
  `up -d --build`)。迁移在启动时于事务内执行,失败整体回滚。
- **服务端看得到什么** —— 只有密文和元数据。它不持有任何密钥,代码里也没有解密路径。
