# Typvia 同步服务器 · Docker 启动

两个文件,两种用法:

- **`compose.image.yml`** —— 部署主机上用。拉一个已发布的镜像跑,**主机上不需要源码、Go 或构建工具链**。
- **`docker-compose.yml`** —— 开发机上用。从本仓库源码构建镜像再跑。

要零端口暴露的远程访问,见 [`../cloudflare/`](../cloudflare/guide.md)(Cloudflare Tunnel 形态)。

## 部署主机:拉镜像跑(一键)

镜像由 `.github/workflows/publish-sync-server.yml` 推到 GHCR:`ghcr.io/<owner>/typvia-sync`,标签有 `latest`(main 分支)、`sha-xxxxxxx`(每次提交)、`1.2.3` / `1.2`(打 `v1.2.3` 标签时)。

```sh
mkdir -p ~/typvia-sync && cd ~/typvia-sync
curl -fsSL -o docker-compose.yml \
  https://raw.githubusercontent.com/<owner>/<repo>/main/deploy/docker/compose.image.yml
echo 'TYPVIA_IMAGE=ghcr.io/<owner>/typvia-sync:latest' > .env
docker compose up -d
curl http://127.0.0.1:8787/healthz    # {"status":"ok"}
```

生产建议把 `latest` 换成具体版本(`:1.2.3`)或提交(`:sha-xxxxxxx`)——回滚就是改一行 tag 再 `up -d`。

**GHCR 包默认是私有的**。首次推送后到 GitHub → Packages → 该包 → Package settings → Change visibility 设为 Public,主机才能匿名拉取;想保持私有就在主机上先登录:

```sh
echo <read:packages 权限的 PAT> | docker login ghcr.io -u <owner> --password-stdin
```

镜像是 multi-arch(linux/amd64 + linux/arm64),x86 VPS 与 arm NAS 都能直接拉。

## 开发机:从源码构建

在**仓库根目录**执行(镜像要 `apps/sync-server/` 源码,构建上下文是仓库根):

```sh
docker compose -f deploy/docker/docker-compose.yml up -d --build
```

## 客户端接入

设置 → 同步 → 设置同步 → 自建服务器,填服务器地址。服务端不需要建号——第一台设备自己创建账户并注册设备,第二台起走配对码。

## 只要二进制,不要容器

```sh
cd apps/sync-server && go run ./cmd/typvia-sync-server
```

默认 `127.0.0.1:8787`,库文件 `typvia-sync.db` 落在工作目录。两个环境变量即全部配置:`TYPVIA_SYNC_LISTEN`、`TYPVIA_SYNC_DB`(命令行 `-listen` / `-db` 优先)。交叉编译出静态二进制丢到服务器也是一条:`GOOS=linux GOARCH=amd64 CGO_ENABLED=0 go build ./cmd/typvia-sync-server`(纯 Go SQLite 驱动,产物无依赖)。

## 换成远程可访问

两份 compose 都把端口绑在 `127.0.0.1`,这是刻意的:客户端对地址有硬校验——**https,或 http 回环**,证书验证永不关闭。所以另一台设备要连上,必须二选一:

- 前面放 Caddy / nginx 终结 TLS,反代到 `127.0.0.1:8787`(端口映射保持在回环);
- 或改用 `../cloudflare/`,连端口都不用开。把那份 compose 里的 `build:` 换成 `image: ${TYPVIA_IMAGE}`,同样不需要源码。

直接把端口绑到 `0.0.0.0` 只会得到一个客户端拒绝连接的明文监听。

## 运维

- **备份**:数据是 `sync-data` 卷里的单个 SQLite 库(WAL)。镜像是 distroless,进不去 shell——先 `docker compose stop` 再对卷做文件级备份,或用主机侧 `sqlite3 .backup`(卷路径见 `docker volume inspect`)。
- **升级**:`docker compose pull && docker compose up -d`(源码构建那份是 `up -d --build`)。迁移在启动时自动执行、失败整体回滚。
- **服务器看得到什么**:只有密文与元数据,没有任何解密能力(零解密红线)。
