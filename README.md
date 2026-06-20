# Swell Video Downloader Desktop

桌面版视频下载器主线项目。

当前目标：

- 输入视频页面 URL
- 一键下载推荐的最佳版本
- 支持展开查看更多清晰度 / 格式
- 第一阶段优先支持 `x.com` 与 `pornhub.com`
- 基于 `yt-dlp + ffmpeg`
- 以 Windows 为主，兼顾 macOS

## 文档入口

- 设计说明：`docs/superpowers/specs/2026-06-20-swell-desktop-downloader-design.md`
- 实施计划：`docs/superpowers/plans/2026-06-20-swell-desktop-downloader-v1.md`

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
