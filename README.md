# Swell Video Downloader Desktop

桌面版视频下载器主线项目。

当前目标：

- 输入视频页面 URL
- 一键下载推荐的最佳版本
- 支持展开查看更多清晰度 / 格式
- 第一阶段优先支持 `x.com` 与 `pornhub.com`
- 基于 `yt-dlp + ffmpeg`
- 以 Windows 为主，兼顾 macOS

## Instagram

- 单条帖子 / Reel：直接粘贴链接即可（采集模式选「当前链接」，数量 1）
- 连续抓取：选择「详情页连续下一条」或「用户主页最近内容」，并设置抓取数量 `N`
- 登录态优先粘贴 `sessionid`；`cookies.txt` 作为备用方案
- Story 为实验性能力，失败时会单独提示
- 采集依赖 Playwright（首次需执行 `node node_modules/playwright/cli.js install chromium` 下载浏览器）
- 访问 Instagram 通常需要本地代理，采集脚本会读取 `HTTPS_PROXY` / `HTTP_PROXY` 环境变量

## 配置持久化

- 基础配置(下载目录、Cookie 来源/路径、Instagram sessionid 与采集设置)保存在用户主目录下的 `~/.swell-video-downloader/config.json`,**卸载应用不会清除**。
- 可用环境变量 `SWELL_CONFIG_DIR` 覆盖该目录位置。
- `sessionid` 以 base64 编码存储(最小可见性保护),不在配置文件里以明文出现;采集过程导出的临时 Cookie 桥接文件不会被持久化。

## 文档入口

- 设计说明：`docs/superpowers/specs/2026-06-20-swell-desktop-downloader-design.md`
- 实施计划：`docs/superpowers/plans/2026-06-20-swell-desktop-downloader-v1.md`
- Instagram 设计说明：`docs/superpowers/specs/2026-06-21-instagram-support-design.md`
- Instagram 实施计划：`docs/superpowers/plans/2026-06-21-instagram-support.md`

## 开发运行

本仓库用 **pnpm**（`pnpm-lock.yaml`），且 `src-tauri/tauri.conf.json` 里的
`beforeDevCommand` / `beforeBuildCommand` 直接写死了 `pnpm dev` / `pnpm build`。
所以 pnpm 必须在 PATH 上，否则 `tauri dev` 会以
「'pnpm' 不是内部或外部命令」退出。

```bash
pnpm install
pnpm tauri:dev
```

没装 pnpm 时用 `npm i -g pnpm` 或 `corepack enable pnpm` 装一个。

临时环境里实在拿不到 pnpm，可以自己起前端、再让 Tauri 跳过那一步（仓库文件不用改）：

```bash
npx vite
```

```bash
npx tauri dev --config '{"build":{"beforeDevCommand":""}}'
```

## 二进制依赖

项目运行时默认优先查找项目内置二进制，其次才回退到系统 PATH。

推荐放置方式：

- `resources/bin/win/yt-dlp.exe`
- `resources/bin/win/ffmpeg.exe`
- `resources/bin/mac/yt-dlp`
- `resources/bin/mac/ffmpeg`

也支持通过环境变量覆盖：

- `SWELL_YTDLP_PATH`
- `SWELL_FFMPEG_PATH`

## 说明

- 当前仓库是桌面应用主线，不依赖旧的 Chrome 扩展项目运行。
- 浏览器扩展如需联动，后续作为附加能力接入。
