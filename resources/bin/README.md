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

---

## 安装 ffmpeg

ffmpeg 超过 100MB，不纳管在 git 中。克隆仓库后请按以下方式安装：

### 方式一：放到 resources/bin/（推荐，开箱即用）

**Windows：**

1. 从 [ffmpeg 官网](https://ffmpeg.org/download.html) 下载 Windows 版本（推荐 [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) 的 `ffmpeg-release-essentials.zip`）
2. 解压后找到 `bin/ffmpeg.exe`
3. 拷贝到 `resources/bin/win/ffmpeg.exe`

**macOS：**

1. 从 [ffmpeg 官网](https://ffmpeg.org/download.html) 下载 macOS 版本，或用 Homebrew：
   ```
   brew install ffmpeg
   ```
2. 找到 ffmpeg 可执行文件并拷贝到 `resources/bin/mac/ffmpeg`：
   ```
   cp $(which ffmpeg) resources/bin/mac/ffmpeg
   ```

### 方式二：加入系统 PATH

安装 ffmpeg 到任意目录，将其所在文件夹加入系统 PATH 环境变量。

**Windows：**
1. 下载解压后，把 `bin/` 目录路径（含 ffmpeg.exe）加入 `Path` 环境变量
2. 重新打开终端，验证：`ffmpeg -version`

**macOS：**
```
brew install ffmpeg
```

### 方式三：通过环境变量指定路径

设置 `SWELL_FFMPEG_PATH` 环境变量指定 ffmpeg 位置：

**Windows (PowerShell)：**
```powershell
$env:SWELL_FFMPEG_PATH = "D:\tools\ffmpeg\bin\ffmpeg.exe"
```

**macOS / Linux：**
```bash
export SWELL_FFMPEG_PATH=/usr/local/bin/ffmpeg
```

---

## 安装 yt-dlp

yt-dlp 体积较小（~17MB）已纳入 git。如需手动更新：

- [yt-dlp 发布页](https://github.com/yt-dlp/yt-dlp/releases)
- Windows 下载 `yt-dlp.exe`，替换 `resources/bin/win/yt-dlp.exe`
- macOS 下载 `yt-dlp` 或 `yt-dlp_macos`，替换 `resources/bin/mac/yt-dlp`

### 环境变量

如果不想放在 `resources/bin/` 里，也可设置环境变量：

```bash
# Windows PowerShell
$env:SWELL_YTDLP_PATH = "D:\tools\yt-dlp.exe"

# macOS / Linux
export SWELL_YTDLP_PATH=/usr/local/bin/yt-dlp
```
