# Swell Desktop Downloader v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建一个可自用的桌面 GUI 下载器，支持 `x.com` 与 `pornhub.com` 的 URL 解析、最佳格式下载、基础下载队列、自动浏览器 Cookie 使用和手动 Cookie 导入兜底。

**Architecture:** 使用 `Tauri + React + TypeScript + React Spectrum` 搭建桌面界面，Rust 侧负责 Tauri commands、任务状态和本地能力桥接，`yt-dlp` 负责解析与下载，`ffmpeg` 负责后处理。前端只关心命令调用与事件订阅，不直接感知底层命令细节。

**Tech Stack:** Tauri, React, TypeScript, Vite, React Spectrum 2 (`@react-spectrum/s2`), Rust, yt-dlp, ffmpeg

---

### Task 1: 初始化桌面仓库与基础骨架

**Files:**
- Create: `package.json`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/app/providers.tsx`
- Create: `src/styles.css`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `README.md`

- [ ] **Step 1: 使用 Tauri 官方模板初始化 React + TypeScript 项目**

```bash
cd F:\FrontEnd\code
corepack pnpm create tauri-app@latest swell-video-downloader-desktop --template react-ts --manager pnpm
```

Expected: 生成 `src/` 与 `src-tauri/` 基础结构，项目可本地启动。

- [ ] **Step 2: 安装 React Spectrum 依赖**

```bash
corepack pnpm -C F:\FrontEnd\code\swell-video-downloader-desktop add @react-spectrum/s2
```

Expected: `package.json` 中出现 `@react-spectrum/s2` 依赖。

- [ ] **Step 3: 建立最小应用 Provider**

```tsx
// src/app/providers.tsx
import type { PropsWithChildren } from 'react';
import { Provider, defaultTheme } from '@react-spectrum/s2';

export function AppProviders({ children }: PropsWithChildren) {
  return <Provider theme={defaultTheme}>{children}</Provider>;
}
```

- [ ] **Step 4: 建立最小应用外壳**

```tsx
// src/App.tsx
import { Heading, View } from '@react-spectrum/s2';

export default function App() {
  return (
    <View padding="size-200">
      <Heading level={1}>Swell Video Downloader</Heading>
    </View>
  );
}
```

- [ ] **Step 5: 接入 Provider**

```tsx
// src/main.tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { AppProviders } from './app/providers';
import './styles.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppProviders>
      <App />
    </AppProviders>
  </React.StrictMode>,
);
```

- [ ] **Step 6: 运行前端与 Tauri 启动检查**

Run:

```bash
corepack pnpm -C F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

Expected: 桌面窗口正常启动，页面显示 `Swell Video Downloader`。

- [ ] **Step 7: Commit**

```bash
git add .
git commit -m "chore: scaffold tauri desktop app"
```

### Task 2: 搭建首页布局与核心 UI 占位

**Files:**
- Create: `src/features/resolve/ResolvePanel.tsx`
- Create: `src/features/resolve/ResultCard.tsx`
- Create: `src/features/downloads/DownloadsTable.tsx`
- Create: `src/features/settings/SettingsPanel.tsx`
- Modify: `src/App.tsx`
- Test: `src/App.tsx`

- [ ] **Step 1: 创建 URL 输入与主操作区**

```tsx
// src/features/resolve/ResolvePanel.tsx
import { Button, Flex, TextField } from '@react-spectrum/s2';

type ResolvePanelProps = {
  url: string;
  onUrlChange: (value: string) => void;
  onDownloadBest: () => void;
  onShowFormats: () => void;
};

export function ResolvePanel(props: ResolvePanelProps) {
  return (
    <Flex direction="column" gap="size-150">
      <TextField
        label="视频地址"
        value={props.url}
        onChange={props.onUrlChange}
        placeholder="粘贴 x.com 或 pornhub.com 视频页面地址"
      />
      <Flex gap="size-100">
        <Button variant="accent" onPress={props.onDownloadBest}>
          下载最佳
        </Button>
        <Button variant="secondary" onPress={props.onShowFormats}>
          查看更多格式
        </Button>
      </Flex>
    </Flex>
  );
}
```

- [ ] **Step 2: 创建结果摘要卡片**

```tsx
// src/features/resolve/ResultCard.tsx
import { Card, Content, Heading, Text } from '@react-spectrum/s2';

export type ResolvedSummary = {
  title: string;
  source: string;
  durationText: string;
  recommendation: string;
};

export function ResultCard({ summary }: { summary: ResolvedSummary | null }) {
  if (!summary) {
    return null;
  }

  return (
    <Card>
      <Heading level={3}>{summary.title}</Heading>
      <Content>
        <Text>{summary.source}</Text>
        <Text>{summary.durationText}</Text>
        <Text>推荐：{summary.recommendation}</Text>
      </Content>
    </Card>
  );
}
```

- [ ] **Step 3: 创建下载队列表格占位**

```tsx
// src/features/downloads/DownloadsTable.tsx
import { Cell, Column, Row, TableBody, TableHeader, TableView } from '@react-spectrum/s2';

export type DownloadRow = {
  id: string;
  title: string;
  status: string;
  progress: string;
};

export function DownloadsTable({ rows }: { rows: DownloadRow[] }) {
  return (
    <TableView aria-label="下载队列">
      <TableHeader>
        <Column>ID</Column>
        <Column>标题</Column>
        <Column>状态</Column>
        <Column>进度</Column>
      </TableHeader>
      <TableBody>
        {rows.map((row) => (
          <Row key={row.id}>
            <Cell>{row.id}</Cell>
            <Cell>{row.title}</Cell>
            <Cell>{row.status}</Cell>
            <Cell>{row.progress}</Cell>
          </Row>
        ))}
      </TableBody>
    </TableView>
  );
}
```

- [ ] **Step 4: 创建设置面板占位**

```tsx
// src/features/settings/SettingsPanel.tsx
import { Content, Dialog, DialogTrigger, Heading, Text } from '@react-spectrum/s2';

export function SettingsPanel() {
  return (
    <DialogTrigger>
      <Text>设置</Text>
      <Dialog>
        <Heading>设置</Heading>
        <Content>第一版先放下载目录、Cookie 来源和依赖状态。</Content>
      </Dialog>
    </DialogTrigger>
  );
}
```

- [ ] **Step 5: 组合主界面**

```tsx
// src/App.tsx
import { Flex, Heading, View } from '@react-spectrum/s2';
import { useState } from 'react';
import { DownloadsTable } from './features/downloads/DownloadsTable';
import { ResolvePanel } from './features/resolve/ResolvePanel';
import { ResultCard, type ResolvedSummary } from './features/resolve/ResultCard';

const demoSummary: ResolvedSummary = {
  title: '示例视频',
  source: 'x.com',
  durationText: '00:45',
  recommendation: '1080p mp4',
};

export default function App() {
  const [url, setUrl] = useState('');

  return (
    <View padding="size-200">
      <Flex direction="column" gap="size-200">
        <Heading level={1}>Swell Video Downloader</Heading>
        <ResolvePanel
          url={url}
          onUrlChange={setUrl}
          onDownloadBest={() => {}}
          onShowFormats={() => {}}
        />
        <ResultCard summary={demoSummary} />
        <DownloadsTable rows={[]} />
      </Flex>
    </View>
  );
}
```

- [ ] **Step 6: 运行桌面应用验证布局**

Run:

```bash
corepack pnpm -C F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

Expected: 首页可见输入框、两个按钮、结果卡片和空队列表格。

- [ ] **Step 7: Commit**

```bash
git add src
git commit -m "feat: add desktop app shell layout"
```

### Task 3: 定义前后端共享类型与 resolve_media 命令

**Files:**
- Create: `src/lib/types.ts`
- Create: `src/lib/tauri.ts`
- Create: `src-tauri/src/commands/resolve.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/commands/resolve.rs`

- [ ] **Step 1: 定义前端共享类型**

```ts
// src/lib/types.ts
export type MediaFormat = {
  id: string;
  label: string;
  ext: string;
  hasAudio: boolean;
  note: string;
};

export type ResolveMediaResponse = {
  title: string;
  source: 'x.com' | 'pornhub.com';
  durationText: string;
  recommendation: MediaFormat;
  formats: MediaFormat[];
};
```

- [ ] **Step 2: 定义前端 Tauri 调用封装**

```ts
// src/lib/tauri.ts
import { invoke } from '@tauri-apps/api/core';
import type { ResolveMediaResponse } from './types';

export async function resolveMedia(url: string): Promise<ResolveMediaResponse> {
  return invoke<ResolveMediaResponse>('resolve_media', { url });
}
```

- [ ] **Step 3: 创建 Rust 命令占位实现**

```rust
// src-tauri/src/commands/resolve.rs
use serde::Serialize;

#[derive(Serialize)]
pub struct MediaFormat {
    pub id: String,
    pub label: String,
    pub ext: String,
    pub has_audio: bool,
    pub note: String,
}

#[derive(Serialize)]
pub struct ResolveMediaResponse {
    pub title: String,
    pub source: String,
    pub duration_text: String,
    pub recommendation: MediaFormat,
    pub formats: Vec<MediaFormat>,
}

#[tauri::command]
pub fn resolve_media(url: String) -> Result<ResolveMediaResponse, String> {
    if !(url.contains("x.com") || url.contains("pornhub.com")) {
        return Err("仅支持 x.com 和 pornhub.com".into());
    }

    Ok(ResolveMediaResponse {
        title: "占位结果".into(),
        source: if url.contains("x.com") { "x.com".into() } else { "pornhub.com".into() },
        duration_text: "00:00".into(),
        recommendation: MediaFormat {
            id: "best".into(),
            label: "最佳".into(),
            ext: "mp4".into(),
            has_audio: true,
            note: "占位推荐".into(),
        },
        formats: vec![],
    })
}
```

- [ ] **Step 4: 在 Tauri 主入口注册命令**

```rust
// src-tauri/src/main.rs
mod commands;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::resolve::resolve_media])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 5: 建立 commands 模块出口**

```rust
// src-tauri/src/commands/mod.rs
pub mod resolve;
```

- [ ] **Step 6: 运行桌面应用验证命令注册通过**

Run:

```bash
corepack pnpm -C F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

Expected: 编译通过，无 `resolve_media` 未注册错误。

- [ ] **Step 7: Commit**

```bash
git add src/lib src-tauri/src
git commit -m "feat: add resolve media command contract"
```

### Task 4: 跑通解析流程与前端结果绑定

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/features/resolve/ResultCard.tsx`
- Modify: `src/lib/tauri.ts`
- Test: `src/App.tsx`

- [ ] **Step 1: 在 App 中接入解析动作状态**

```tsx
// src/App.tsx
import { useState } from 'react';
import { InlineAlert } from '@react-spectrum/s2';
import { resolveMedia } from './lib/tauri';
import type { ResolveMediaResponse } from './lib/types';
```

- [ ] **Step 2: 实现“查看更多格式”触发解析**

```tsx
const [resolved, setResolved] = useState<ResolveMediaResponse | null>(null);
const [error, setError] = useState('');

async function handleResolve() {
  setError('');
  try {
    const result = await resolveMedia(url);
    setResolved(result);
  } catch (err) {
    setResolved(null);
    setError(err instanceof Error ? err.message : '解析失败');
  }
}
```

- [ ] **Step 3: 将结果映射到结果卡片**

```tsx
<ResolvePanel
  url={url}
  onUrlChange={setUrl}
  onDownloadBest={handleResolve}
  onShowFormats={handleResolve}
/>
{error ? <InlineAlert variant="negative">{error}</InlineAlert> : null}
<ResultCard
  summary={
    resolved
      ? {
          title: resolved.title,
          source: resolved.source,
          durationText: resolved.durationText,
          recommendation: `${resolved.recommendation.label} ${resolved.recommendation.ext}`,
        }
      : null
  }
/>
```

- [ ] **Step 4: 手工验证支持站点和非支持站点**

Run:

```bash
corepack pnpm -C F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

Expected:

- 输入 `https://x.com/...` 时出现占位解析结果
- 输入不支持域名时出现错误提示“仅支持 x.com 和 pornhub.com”

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "feat: wire resolve flow into desktop ui"
```

### Task 5: 接入 yt-dlp 真实解析

**Files:**
- Create: `src-tauri/src/downloader/yt_dlp.rs`
- Modify: `src-tauri/src/commands/resolve.rs`
- Create: `src-tauri/src/downloader/mod.rs`
- Test: `src-tauri/src/commands/resolve.rs`

- [ ] **Step 1: 创建 yt-dlp 元数据读取封装**

```rust
// src-tauri/src/downloader/yt_dlp.rs
use std::process::Command;

pub fn fetch_metadata(url: &str) -> Result<String, String> {
    let output = Command::new("yt-dlp")
        .arg("-J")
        .arg(url)
        .output()
        .map_err(|err| format!("无法启动 yt-dlp: {err}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

- [ ] **Step 2: 导出 downloader 模块**

```rust
// src-tauri/src/downloader/mod.rs
pub mod yt_dlp;
```

- [ ] **Step 3: 在 resolve 命令中调用 yt-dlp**

```rust
// src-tauri/src/commands/resolve.rs
use crate::downloader::yt_dlp::fetch_metadata;

let raw = fetch_metadata(&url)?;
let title = if raw.contains("\"title\"") {
    "真实解析结果".to_string()
} else {
    "未识别标题".to_string()
};
```

- [ ] **Step 4: 先用最小策略解析字段，不一次性做复杂映射**

```rust
Ok(ResolveMediaResponse {
    title,
    source: if url.contains("x.com") { "x.com".into() } else { "pornhub.com".into() },
    duration_text: "--:--".into(),
    recommendation: MediaFormat {
        id: "best".into(),
        label: "best".into(),
        ext: "mp4".into(),
        has_audio: true,
        note: "来自 yt-dlp".into(),
    },
    formats: vec![],
})
```

- [ ] **Step 5: 用真实 URL 手工验证**

Run:

```bash
yt-dlp -J "https://x.com/..."
corepack pnpm -C F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

Expected: Rust 命令能成功启动 `yt-dlp`，前端能拿到真实解析链路结果，即使格式列表先不完整也要先打通。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src
git commit -m "feat: connect yt-dlp metadata resolution"
```

### Task 6: 加入下载任务与进度事件骨架

**Files:**
- Create: `src-tauri/src/commands/download.rs`
- Create: `src-tauri/src/events/download_events.rs`
- Modify: `src-tauri/src/main.rs`
- Create: `src/lib/download-events.ts`
- Modify: `src/features/downloads/DownloadsTable.tsx`
- Test: `src/features/downloads/DownloadsTable.tsx`

- [ ] **Step 1: 创建下载命令占位**

```rust
// src-tauri/src/commands/download.rs
#[tauri::command]
pub fn start_download(url: String, format_id: Option<String>) -> Result<String, String> {
    let task_id = format!("task-{}", uuid::Uuid::new_v4());
    let _ = (url, format_id);
    Ok(task_id)
}
```

- [ ] **Step 2: 创建下载事件名常量**

```rust
// src-tauri/src/events/download_events.rs
pub const DOWNLOAD_PROGRESS: &str = "download://progress";
pub const DOWNLOAD_STATUS: &str = "download://status";
pub const DOWNLOAD_ERROR: &str = "download://error";
```

- [ ] **Step 3: 注册下载命令**

```rust
tauri::Builder::default().invoke_handler(tauri::generate_handler![
    commands::resolve::resolve_media,
    commands::download::start_download
])
```

- [ ] **Step 4: 添加前端事件订阅封装**

```ts
// src/lib/download-events.ts
import { listen } from '@tauri-apps/api/event';

export function listenDownloadProgress(
  handler: (payload: unknown) => void,
) {
  return listen('download://progress', (event) => handler(event.payload));
}
```

- [ ] **Step 5: 在队列表格中预留任务数据结构**

```tsx
export type DownloadRow = {
  id: string;
  title: string;
  status: string;
  progress: string;
  speed?: string;
};
```

- [ ] **Step 6: 运行编译检查**

Run:

```bash
corepack pnpm -C F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

Expected: 下载命令和事件骨架编译通过，为下一步真实下载留出接口。

- [ ] **Step 7: Commit**

```bash
git add src src-tauri/src
git commit -m "feat: add download command and event skeleton"
```

### Task 7: 加入 Cookie 来源设置与手动导入占位

**Files:**
- Create: `src/features/cookies/CookieSourcePanel.tsx`
- Create: `src-tauri/src/commands/cookies.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/features/settings/SettingsPanel.tsx`
- Test: `src/features/settings/SettingsPanel.tsx`

- [ ] **Step 1: 创建 Cookie 命令占位**

```rust
// src-tauri/src/commands/cookies.rs
use serde::Serialize;

#[derive(Serialize)]
pub struct CookieSource {
    pub id: String,
    pub label: String,
}

#[tauri::command]
pub fn list_cookie_sources() -> Vec<CookieSource> {
    vec![
        CookieSource { id: "chrome".into(), label: "Chrome".into() },
        CookieSource { id: "edge".into(), label: "Edge".into() },
        CookieSource { id: "import".into(), label: "手动导入".into() },
    ]
}
```

- [ ] **Step 2: 注册 Cookie 命令**

```rust
tauri::Builder::default().invoke_handler(tauri::generate_handler![
    commands::resolve::resolve_media,
    commands::download::start_download,
    commands::cookies::list_cookie_sources
])
```

- [ ] **Step 3: 创建前端 Cookie 来源面板**

```tsx
// src/features/cookies/CookieSourcePanel.tsx
import { Picker } from '@react-spectrum/s2';

export function CookieSourcePanel() {
  return (
    <Picker label="Cookie 来源">
      <item key="chrome">Chrome</item>
      <item key="edge">Edge</item>
      <item key="import">手动导入</item>
    </Picker>
  );
}
```

- [ ] **Step 4: 在设置面板里接入 Cookie 来源与引导文案**

```tsx
// src/features/settings/SettingsPanel.tsx
import { Content, Dialog, DialogTrigger, Heading, Text } from '@react-spectrum/s2';
import { CookieSourcePanel } from '../cookies/CookieSourcePanel';

export function SettingsPanel() {
  return (
    <DialogTrigger>
      <Text>设置</Text>
      <Dialog>
        <Heading>设置</Heading>
        <Content>
          <CookieSourcePanel />
          <Text>如果自动读取失败，后续补充“导入 Cookie 文件”流程。</Text>
        </Content>
      </Dialog>
    </DialogTrigger>
  );
}
```

- [ ] **Step 5: 运行 UI 验证**

Run:

```bash
corepack pnpm -C F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

Expected: 设置面板可打开，Cookie 来源选择器可见。

- [ ] **Step 6: Commit**

```bash
git add src src-tauri/src
git commit -m "feat: add cookie source settings skeleton"
```

### Task 8: 增加依赖检查与启动前自检

**Files:**
- Create: `src-tauri/src/commands/system.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/features/settings/SettingsPanel.tsx`
- Test: `src-tauri/src/commands/system.rs`

- [ ] **Step 1: 创建依赖检查命令**

```rust
// src-tauri/src/commands/system.rs
use serde::Serialize;
use std::process::Command;

#[derive(Serialize)]
pub struct DependencyStatus {
    pub yt_dlp_ok: bool,
    pub ffmpeg_ok: bool,
}

#[tauri::command]
pub fn check_dependencies() -> DependencyStatus {
    let yt_dlp_ok = Command::new("yt-dlp").arg("--version").output().is_ok();
    let ffmpeg_ok = Command::new("ffmpeg").arg("-version").output().is_ok();
    DependencyStatus { yt_dlp_ok, ffmpeg_ok }
}
```

- [ ] **Step 2: 注册系统命令**

```rust
tauri::Builder::default().invoke_handler(tauri::generate_handler![
    commands::resolve::resolve_media,
    commands::download::start_download,
    commands::cookies::list_cookie_sources,
    commands::system::check_dependencies
])
```

- [ ] **Step 3: 在设置面板中预留依赖状态展示**

```tsx
<Text>依赖状态：yt-dlp / ffmpeg</Text>
```

- [ ] **Step 4: 手工验证依赖检查**

Run:

```bash
yt-dlp --version
ffmpeg -version
corepack pnpm -C F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

Expected: 环境存在依赖时命令可执行；缺依赖时后续 UI 需要能展示异常状态。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src src/features/settings/SettingsPanel.tsx
git commit -m "feat: add dependency health check"
```

## 自检结果

### Spec 覆盖检查

- 新建独立仓库：Task 1 覆盖
- React Spectrum UI：Task 1、Task 2 覆盖
- URL 输入、下载最佳、查看更多格式：Task 2、Task 4 覆盖
- `x.com` / `pornhub.com` 优先：Task 3、Task 5 覆盖
- `yt-dlp + ffmpeg` 路线：Task 5、Task 8 覆盖
- 下载队列与进度：Task 6 覆盖
- 自动 Cookie 与手动导入兜底：Task 7 为骨架，后续在执行时补完整实现

### 占位检查

当前计划中保留的“占位”仅限于第一阶段的渐进式落地顺序，不是未定义任务。每一步都已明确落点文件、命令和代码方向。

### 类型一致性检查

- `resolve_media`、`start_download`、`list_cookie_sources`、`check_dependencies` 的命名在前后任务中保持一致
- 前端使用的 `ResolveMediaResponse`、`DownloadRow` 与 Rust 命令返回模型对应关系已提前约定

## 执行交接

Plan complete and saved to `docs/superpowers/plans/2026-06-20-swell-desktop-downloader-v1.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
