# Binary Layout

把项目内置依赖放在这里，应用运行时会优先从这些目录查找，再回退到系统 PATH。

目录约定：

- `resources/bin/win/yt-dlp.exe`
- `resources/bin/win/ffmpeg.exe`
- `resources/bin/mac/yt-dlp`
- `resources/bin/mac/ffmpeg`

说明：

- Windows 下优先放 `.exe`
- macOS 下使用无扩展名可执行文件
- 后续如需 Linux，可再增加 `resources/bin/linux/`
