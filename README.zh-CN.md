<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/assets/logo-on-dark.png">
  <img src=".github/assets/logo-on-light.png" alt="Typvia" width="380">
</picture>

**保存一次,随处输入。**

开源、本地优先、端到端加密的跨平台文本调用器,
支持 Windows、macOS、iOS 与 Android。

[![CI](https://github.com/leazoot/Typvia/actions/workflows/ci.yml/badge.svg)](https://github.com/leazoot/Typvia/actions/workflows/ci.yml)
[![客户端 MPL-2.0](https://img.shields.io/badge/%E5%AE%A2%E6%88%B7%E7%AB%AF-MPL--2.0-blue)](LICENSE)
[![同步服务 AGPL-3.0](https://img.shields.io/badge/%E5%90%8C%E6%AD%A5%E6%9C%8D%E5%8A%A1-AGPL--3.0-blue)](apps/sync-server/LICENSE)
[![平台](https://img.shields.io/badge/%E5%B9%B3%E5%8F%B0-Windows%20%7C%20macOS%20%7C%20iOS%20%7C%20Android-lightgrey)](#支持平台)

[English](README.md) · 简体中文

</div>

> **当前状态:早期开发中,尚无发行版本。**

## 为什么做它

每天重复敲的那些文本——命令、SQL、代码块、提示词、固定回复、地址、
Token——散落在笔记、聊天记录、剪贴板和密码管理器里。现有的每个去处都要放弃点什么:
文本扩展工具没有移动端,也不适合放敏感内容;剪贴板管理器只记短期;
笔记软件的调用链路太长;密码管理器不擅长代码和模板。

Typvia 把它们收进同一个本地加密存储,并放在一次击键之外——
桌面上如此,手机上则直接做进键盘里。

## 它能做什么

**收集** —— 片段以文件夹与标签组织,类型分为文本、代码、命令、提示词、模板、
密文、AI 动作与链接。可从已有的扩展工具配置文件和 CSV 导入。

**找回** —— 悬浮于任意应用之上的全局面板;基于 SQLite FTS5 的全文搜索,
带中日韩分词;可选的本地语义搜索;按你的真实使用情况排序。
按 50,000 条片段设计:1 万条搜索 50ms 内,5 万条 150ms 内,由仓库内的基准测试守住。

**输入** —— 通过托管的展开引擎按缩写触发,或直接从面板插入。
移动端的入口是真正的键盘:iOS 自定义键盘、Android 输入法,
另有分享入口与浏览器扩展。

**填充** —— 模板带有类型化变量,在插入的那一刻完成填写,
而不是先粘贴再手动改。

**保护** —— 敏感片段进保险库,静态加密、生物识别解锁。
保险库内容不会进入展开引擎的配置文件、普通全文索引、日志、崩溃报告或 AI 请求。

**同步** —— 可选的端到端加密多设备同步,扫码配对并做一次读码核对。
自己跑内置的服务端,或者用 WebDAV。服务端只持有密文,代码里没有任何解密路径。

**辅助** —— 可选的 AI,用于拟标题、打标签、抽取模板变量。
自带 Key,或指向本地模型。默认关闭,并且有出网闸门:
保险库内容与任何疑似凭据都无法离开设备。

## 支持平台

| 平台    | 最低版本  | 调用入口                             |
| ------- | --------- | ------------------------------------ |
| Windows | 10        | 主应用、全局面板、托盘、缩写触发     |
| macOS   | 12        | 主应用、全局面板、菜单栏、缩写触发   |
| iOS     | 16        | 主应用、自定义键盘、分享扩展、小组件 |
| Android | 9(API 28) | 主应用、输入法、分享目标             |

开发在 macOS 上进行。Windows 侧的实现基于同一套抽象编写,并在 CI 中编译通过,
但尚未在真实 Windows 机器上跑过——请视为未验证。
Linux 暂不是支持目标,但 CI 会为每次推送到 `main` 的提交构建 `.deb` 与 AppImage 产物。

## 安全

Typvia 是一个本就要用来放凭据的地方,所以安全模型是产品的一部分,而不是脚注:

- 内容在设备上加密。密钥材料只存在于平台安全存储中——Keychain、Keystore、DPAPI——
  永不上传、永不写日志。
- 同步服务端按敌对方对待:它只看得到密文与路由用的元数据,
  并且没有任何一条代码路径能解开一条记录。
- 敏感片段在结构上被排除在所有容易泄露明文的地方之外:
  展开引擎配置、普通搜索索引、日志、崩溃报告、错误信息、云端 AI 请求与测试快照。
- 全部功能离线可用。同步、账户与 AI 各自可选,也各自可以完全关掉。

完整的密码学设计——密钥层级、密文格式、服务端到底收到什么、配对与恢复,
以及守住每条断言的测试——见 [`SECURITY_MODEL.md`](SECURITY_MODEL.md)(英文)。
威胁模型、明确**不**防御的部分,以及如何私下报告漏洞,见
[`SECURITY.md`](SECURITY.md)。

## 自托管同步服务

服务端是单个静态 Go 二进制加一个 SQLite 文件。
部署主机上既不需要源码,也不需要工具链:

```sh
mkdir -p ~/typvia-sync && cd ~/typvia-sync
curl -fsSL -o docker-compose.yml \
  https://raw.githubusercontent.com/leazoot/Typvia/main/deploy/docker/compose.image.yml
echo 'TYPVIA_IMAGE=ghcr.io/leazoot/typvia-sync:latest' > .env
docker compose up -d
curl http://127.0.0.1:8787/healthz
```

多架构镜像随每次推送发布到 GHCR。从源码构建、升级与备份见
[`deploy/docker/`](deploy/docker/README.md);
不暴露任何端口的隧道部署见 [`deploy/cloudflare/`](deploy/cloudflare/guide.md)。

## 从源码构建

需要 Node 22.22+、pnpm 10.30+、带 2024 edition 的 stable Rust 工具链,
以及用于同步服务的 Go 1.25+。构建移动端及其原生层还需要 Xcode 或 Android SDK 与 NDK。

```sh
pnpm install
pnpm dev            # 运行桌面应用

pnpm lint           # clippy + eslint
pnpm typecheck
pnpm test           # cargo test + vitest
pnpm build
```

完整工具链、目录结构,以及代码评审会照着执行的规则,见
[`CONTRIBUTING.md`](CONTRIBUTING.md)。

## 仓库结构

```text
apps/desktop            Tauri 2 桌面应用(Windows、macOS)
apps/mobile             Tauri 2 移动应用(iOS、Android)
apps/sync-server        Go 同步服务端 —— 只存密文,可自托管
apps/browser-extension  浏览器扩展
apps/browser-host       扩展的原生消息宿主
crates/core             数据模型、SQLite 存储、用例编排
crates/crypto           密钥派生、AEAD、安全存储 trait
crates/search           FTS5 索引与加权排序
crates/semantic         本地语义搜索内核
crates/sync             端到端加密同步客户端
crates/template         模板解析与渲染
crates/ai               AI Provider 适配层与出网闸门
crates/espanso-adapter  展开引擎的配置、导入与生命周期
crates/host-service     两个应用壳共用的 IPC 编排层
crates/mobile-ffi       供移动原生层调用的 UniFFI 接口
packages/ui             共享 React 组件库与设计 tokens
packages/shared         共享 TypeScript 类型与类型化 IPC 层
native/                 Swift 键盘与分享扩展、Kotlin 输入法
deploy/                 同步服务的自托管模板
```

## 参与贡献

欢迎 Issue 与 Pull Request——较大的改动请先开 Issue 讨论。
请阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md) 与
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)。
贡献以 [Developer Certificate of Origin](https://developercertificate.org/)
方式接受,提交时用 `git commit -s` 签名。

## 许可证

- **MPL-2.0**([`LICENSE`](LICENSE))—— 客户端与全部共享代码。
- **AGPL-3.0**([`apps/sync-server/LICENSE`](apps/sync-server/LICENSE))——
  仅覆盖同步服务端。

桌面发行版捆绑 [Espanso](https://espanso.org)(GPL-3.0)作为展开引擎:
未经修改的官方二进制,以独立程序形式分发,仅通过其命令行与配置文件驱动。
Typvia 不链接它的任何部分,也不复制它的源码。
详情与对应源码的获取途径见 [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)。
