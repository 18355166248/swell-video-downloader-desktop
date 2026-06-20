// Populates resources/bin/<platform>/ffmpeg from the `ffmpeg-static` package so
// the desktop app has a working ffmpeg without committing the (large) binary to
// git. Runs automatically on `postinstall`; also runnable via `npm run setup:ffmpeg`.
// Safe to re-run and degrades gracefully when the binary isn't available.
import { execFileSync } from 'node:child_process';
import { copyFileSync, chmodSync, existsSync, mkdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { platform } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), '..');

const TARGETS = {
  win32: { dir: 'win', file: 'ffmpeg.exe' },
  darwin: { dir: 'mac', file: 'ffmpeg' },
  linux: { dir: 'linux', file: 'ffmpeg' },
};

function log(message) {
  console.log(`[setup-ffmpeg] ${message}`);
}

const target = TARGETS[platform()];
if (!target) {
  log(`不支持的平台 ${platform()}，跳过。`);
  process.exit(0);
}

const destDir = join(projectRoot, 'resources', 'bin', target.dir);
const dest = join(destDir, target.file);

if (existsSync(dest)) {
  log(`已存在，跳过：${dest}`);
  process.exit(0);
}

// `ffmpeg-static` exports the absolute path to its downloaded binary.
const require = createRequire(import.meta.url);
let sourcePath;
try {
  sourcePath = require('ffmpeg-static');
} catch {
  log('未安装 ffmpeg-static，跳过。可手动放置 ffmpeg 或设置环境变量 SWELL_FFMPEG_PATH。');
  process.exit(0);
}

// npmmirror mirrors the exact same release assets and is ~4x faster than GitHub
// from CN networks (measured: ~5MB/s vs ~1.2MB/s). Used by default; overridable
// via the FFMPEG_BINARIES_URL env var; falls back to GitHub if the mirror fails.
const NPM_MIRROR_URL = 'https://registry.npmmirror.com/-/binary/ffmpeg-static';

function runInstaller(installer, binariesUrl) {
  // ffmpeg-static's downloader (@derhuerst/http-basic) crashes on an http proxy
  // with an https target (ERR_INVALID_PROTOCOL); the mirror/GitHub are reachable
  // directly, so strip proxy vars for this child to avoid the bug.
  const env = { ...process.env };
  for (const key of ['HTTP_PROXY', 'HTTPS_PROXY', 'http_proxy', 'https_proxy']) {
    delete env[key];
  }
  if (binariesUrl) {
    env.FFMPEG_BINARIES_URL = binariesUrl;
  } else {
    delete env.FFMPEG_BINARIES_URL;
  }
  execFileSync(process.execPath, [installer], { stdio: 'inherit', env });
}

// pnpm/yarn may skip a dependency's own install script, so the binary often isn't
// downloaded yet. Run ffmpeg-static's downloader ourselves (this script is our own
// postinstall, so it always runs) before copying.
if (!sourcePath || !existsSync(sourcePath)) {
  const installer = require.resolve('ffmpeg-static/install.js');
  const userUrl = process.env.FFMPEG_BINARIES_URL;
  const primaryUrl = userUrl || NPM_MIRROR_URL;
  try {
    log(`正在下载 ffmpeg（源：${primaryUrl}）…`);
    runInstaller(installer, primaryUrl);
  } catch (error) {
    // A user-supplied URL is respected as-is; only the default mirror auto-falls
    // back to the GitHub origin.
    if (userUrl) {
      log(`下载失败：${error?.message ?? error}。可设置 SWELL_FFMPEG_PATH 指向本地 ffmpeg。`);
      process.exit(0);
    }
    log('镜像下载失败，回退 GitHub 源重试…');
    try {
      runInstaller(installer, undefined);
    } catch (fallbackError) {
      log(`下载失败：${fallbackError?.message ?? fallbackError}。可设置 SWELL_FFMPEG_PATH 指向本地 ffmpeg。`);
      process.exit(0);
    }
  }
}

if (!sourcePath || !existsSync(sourcePath)) {
  log('ffmpeg-static 未下载到二进制（可能网络受限），跳过。可设置 SWELL_FFMPEG_PATH 指向本地 ffmpeg。');
  process.exit(0);
}

mkdirSync(destDir, { recursive: true });
copyFileSync(sourcePath, dest);
if (platform() !== 'win32') {
  chmodSync(dest, 0o755);
}
log(`已内置 ffmpeg → ${dest}`);
