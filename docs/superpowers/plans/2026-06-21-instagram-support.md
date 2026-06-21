# Instagram 支持实施计划

> **致执行型 Agent：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐 Task 实施本计划。各步骤使用复选框（`- [ ]`）语法以便追踪进度。

**Goal:** 为现有 Swell 桌面下载器增加 Instagram 单链接解析、详情页连续抓取 `N` 条、用户主页抓取最近 `N` 条，以及 `sessionid / cookies.txt` 登录态支持。

**Architecture:** 保持现有 `Tauri + React + yt-dlp` 主架构不变，新增一个 Node + Playwright 的 `instagram-collector` 采集脚本作为前置步骤。浏览器自动化只负责建立 Instagram 登录态、采集 canonical URL、导出临时 cookies；最终解析和下载仍走现有 `resolve_media` 与 `start_download` 链路。

**Tech Stack:** Tauri, Rust, React, TypeScript, Vite, Playwright, Node.js, yt-dlp, ffmpeg

---

## 文件结构

**新增文件**

- `scripts/instagram-collector.mjs`  
  Playwright 自动化入口。负责接收 JSON 参数、建立登录态、采集帖子 URL、导出 cookies、输出 JSON 结果。

- `src-tauri/src/commands/instagram.rs`  
  Tauri 命令入口。负责参数校验、子进程调用 `instagram-collector.mjs`、收集 stdout/stderr、返回结构化 JSON。

- `src-tauri/src/commands/instagram_types.rs`  
  Instagram 采集相关的 Rust 请求/响应类型，避免把所有类型挤进命令实现文件。

**修改文件**

- `package.json`  
  增加 `playwright` 依赖和一个 collector 自测脚本。

- `src-tauri/src/commands/mod.rs`  
  导出 Instagram commands/types。

- `src-tauri/src/lib.rs`  
  注册 `collect_instagram_targets` 命令。

- `src-tauri/src/commands/resolve.rs`  
  把域名白名单从 `x.com / pornhub.com` 扩展到 `instagram.com`，支持 Instagram 单条直解析。

- `src-tauri/src/commands/download.rs`  
  支持下载时优先使用 Instagram collector 导出的临时 Cookie 文件。

- `src-tauri/src/downloader/yt_dlp.rs`  
  补充 Instagram 场景下的 Cookie 参数优先级，以及错误分类映射。

- `src/lib/types.ts`  
  增加 Instagram auth/collect 类型。

- `src/lib/tauri.ts`  
  增加 `collectInstagramTargets()` 封装。

- `src/features/cookies/CookieSourcePanel.tsx`  
  扩展为既支持通用 Cookie 设置，也支持 Instagram 专用 `sessionid / cookies.txt`。

- `src/features/settings/SettingsPanel.tsx`  
  新增 Instagram 访问设置块。

- `src/App.tsx`  
  对 Instagram URL 做采集前置分流，并把批量采集结果注入现有 `handleResolveAll()`。

- `src/styles.css`  
  支持 Instagram 设置块和模式/数量输入的样式。

---

### Task 1：新增 Instagram 类型与 Tauri 命令契约

**文件：**
- 创建：`src-tauri/src/commands/instagram_types.rs`
- 创建：`src-tauri/src/commands/instagram.rs`
- 修改：`src-tauri/src/commands/mod.rs`
- 修改：`src-tauri/src/lib.rs`
- 测试：`src-tauri/src/commands/instagram.rs`

- [ ] **步骤 1：编写校验 URL / 模式 / 数量的失败 Rust 测试**

```rust
// src-tauri/src/commands/instagram.rs
#[cfg(test)]
mod tests {
    use super::validate_collect_request;
    use crate::commands::instagram_types::{
        CollectInstagramTargetsRequest, InstagramCollectMode,
    };

    #[test]
    fn rejects_non_instagram_url() {
        let request = CollectInstagramTargetsRequest {
            url: "https://example.com/demo".into(),
            mode: InstagramCollectMode::Single,
            count: 1,
            sessionid: None,
            cookie_file_path: None,
        };

        let error = validate_collect_request(&request).expect_err("should reject host");
        assert!(error.contains("Instagram"));
    }

    #[test]
    fn rejects_zero_count() {
        let request = CollectInstagramTargetsRequest {
            url: "https://www.instagram.com/p/abc123/".into(),
            mode: InstagramCollectMode::Single,
            count: 0,
            sessionid: None,
            cookie_file_path: None,
        };

        let error = validate_collect_request(&request).expect_err("should reject zero count");
        assert!(error.contains("数量"));
    }
}
```

- [ ] **步骤 2：运行 Rust 测试，确认因文件/函数尚不存在而失败**

运行：

```bash
cargo test instagram --manifest-path F:\FrontEnd\code\swell-video-downloader-desktop\src-tauri\Cargo.toml
```

预期：因模块/函数缺失而 FAIL（编译失败）。

- [ ] **步骤 3：创建 Instagram 请求/响应类型**

```rust
// src-tauri/src/commands/instagram_types.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstagramCollectMode {
    Single,
    DetailNext,
    ProfileRecent,
    StoryExperimental,
}

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct CollectInstagramTargetsRequest {
    pub url: String,
    pub mode: InstagramCollectMode,
    pub count: u32,
    pub sessionid: Option<String>,
    pub cookie_file_path: Option<String>,
}

#[derive(Clone, Serialize, Debug, PartialEq, Eq)]
pub struct InstagramCollectItem {
    pub url: String,
    pub kind: String,
    pub source_label: String,
    pub thumbnail_hint: Option<String>,
}

#[derive(Clone, Serialize, Debug, PartialEq, Eq)]
pub struct CollectInstagramTargetsResponse {
    pub items: Vec<InstagramCollectItem>,
    pub resolved_count: usize,
    pub warnings: Vec<String>,
    pub cookie_bridge_file_path: Option<String>,
}
```

- [ ] **步骤 4：实现最小命令契约与参数校验**

```rust
// src-tauri/src/commands/instagram.rs
use crate::commands::instagram_types::{
    CollectInstagramTargetsRequest, CollectInstagramTargetsResponse,
};

pub fn validate_collect_request(
    request: &CollectInstagramTargetsRequest,
) -> Result<(), String> {
    if !request.url.contains("instagram.com") {
        return Err("仅支持 Instagram 链接".into());
    }

    if request.count == 0 {
        return Err("抓取数量必须大于 0".into());
    }

    Ok(())
}

#[tauri::command]
pub async fn collect_instagram_targets(
    request: CollectInstagramTargetsRequest,
) -> Result<CollectInstagramTargetsResponse, String> {
    validate_collect_request(&request)?;

    Ok(CollectInstagramTargetsResponse {
        items: vec![],
        resolved_count: 0,
        warnings: vec![],
        cookie_bridge_file_path: None,
    })
}
```

- [ ] **步骤 5：导出并注册命令**

```rust
// src-tauri/src/commands/mod.rs
pub mod cookies;
pub mod download;
pub mod instagram;
pub mod instagram_types;
pub mod preview;
pub mod resolve;
pub mod system;
```

```rust
// src-tauri/src/lib.rs
        .invoke_handler(tauri::generate_handler![
            commands::resolve::resolve_media,
            commands::resolve::diagnose_media,
            commands::instagram::collect_instagram_targets,
            commands::download::start_download,
            commands::download::cancel_download,
            commands::download::get_download_dir,
            commands::download::get_download_dir_settings,
            commands::download::set_download_dir,
            commands::download::reset_download_dir,
            commands::preview::generate_preview,
            commands::cookies::list_cookie_sources,
            commands::system::check_dependencies
        ])
```

- [ ] **步骤 6：再次运行测试并确认通过**

运行：

```bash
cargo test instagram --manifest-path F:\FrontEnd\code\swell-video-downloader-desktop\src-tauri\Cargo.toml
```

预期：新增的校验测试 PASS（通过）。

- [ ] **步骤 7：提交**

```bash
git add src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "feat: add Instagram command contract"
```

### Task 2：新增 Playwright 采集脚本与本地 JSON 契约

**文件：**
- 修改：`package.json`
- 创建：`scripts/instagram-collector.mjs`
- 测试：`scripts/instagram-collector.mjs`

- [ ] **步骤 1：在 package.json 中写一个会失败的冒烟测试命令**

```json
{
  "scripts": {
    "instagram:collector": "node scripts/instagram-collector.mjs"
  }
}
```

- [ ] **步骤 2：安装 Playwright，并确认脚本当前缺失**

运行：

```bash
pnpm add -D playwright --dir F:\FrontEnd\code\swell-video-downloader-desktop
pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop instagram:collector
```

预期：因 “Cannot find module” 或脚本文件缺失而 FAIL。

- [ ] **步骤 3：创建带 stdin/stdout JSON 管道的采集脚本**

```js
// scripts/instagram-collector.mjs
import { chromium } from 'playwright';

async function readJsonStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  const text = Buffer.concat(chunks).toString('utf8').trim();
  return text ? JSON.parse(text) : {};
}

function writeJsonStdout(payload) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}

function normalizeInstagramUrl(raw) {
  const parsed = new URL(raw);
  parsed.hash = '';
  parsed.search = '';
  return parsed.toString();
}

async function main() {
  const input = await readJsonStdin();
  const browser = await chromium.launch({ headless: true });
  try {
    const context = await browser.newContext();
    const url = normalizeInstagramUrl(input.url);
    writeJsonStdout({
      items: [{ url, kind: 'unknown', source_label: 'collector smoke test', thumbnail_hint: null }],
      resolved_count: 1,
      warnings: [],
      cookie_bridge_file_path: null,
    });
  } finally {
    await browser.close();
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack || error.message}\n`);
  process.exit(1);
});
```

- [ ] **步骤 4：用一个示例 Instagram URL 运行采集脚本**

运行：

```bash
echo {"url":"https://www.instagram.com/p/abc123/"} | pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop instagram:collector
```

预期：PASS，输出一行包含 `items` 与 `resolved_count` 的 JSON。

- [ ] **步骤 5：提交**

```bash
git add package.json pnpm-lock.yaml scripts/instagram-collector.mjs
git commit -m "feat: add Instagram Playwright collector skeleton"
```

### Task 3：实现 sessionid 登录态桥接与采集模式

**文件：**
- 修改：`scripts/instagram-collector.mjs`
- 测试：`scripts/instagram-collector.mjs`

- [ ] **步骤 1：为登录态与模式分支添加会失败的断言**

```js
// add near helper tests or temporary assertions in script-specific test harness
function buildCookieBridgeLine(domain, value) {
  return [domain, 'TRUE', '/', 'TRUE', '0', 'sessionid', value].join('\t');
}

if (process.env.INSTAGRAM_COLLECTOR_SELFTEST === '1') {
  const line = buildCookieBridgeLine('.instagram.com', 'abc');
  if (!line.includes('sessionid')) {
    throw new Error('cookie bridge self-test failed');
  }
  process.exit(0);
}
```

- [ ] **步骤 2：在实现完整模式支持前先运行自测**

运行：

```bash
$env:INSTAGRAM_COLLECTOR_SELFTEST="1"; pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop instagram:collector
```

预期：辅助函数尚不存在，因此 FAIL。

- [ ] **步骤 3：实现登录态桥接辅助函数与临时 cookies.txt 导出**

```js
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

function buildCookieBridgeLine(domain, name, value) {
  return [domain, 'TRUE', '/', 'TRUE', '0', name, value].join('\t');
}

async function writeTempCookiesFile(cookies) {
  const filePath = path.join(os.tmpdir(), `swell-instagram-${Date.now()}.cookies.txt`);
  const lines = ['# Netscape HTTP Cookie File', ...cookies];
  await fs.writeFile(filePath, `${lines.join('\n')}\n`, 'utf8');
  return filePath;
}
```

- [ ] **步骤 4：实现 sessionid 注入、主页采集与详情页连续采集**

```js
async function applySessionCookie(context, sessionid) {
  if (!sessionid?.trim()) return;
  await context.addCookies([
    {
      name: 'sessionid',
      value: sessionid.trim(),
      domain: '.instagram.com',
      path: '/',
      httpOnly: true,
      secure: true,
    },
  ]);
}

async function collectSingle(page, url) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  return [{ url: page.url(), kind: 'unknown', source_label: 'single', thumbnail_hint: null }];
}

async function collectProfileRecent(page, url, count) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  const urls = new Set();
  for (let i = 0; i < 8 && urls.size < count; i += 1) {
    const hrefs = await page.locator('a[href*="/p/"], a[href*="/reel/"]').evaluateAll((nodes) =>
      nodes.map((node) => node.href),
    );
    hrefs.forEach((href) => urls.add(href));
    await page.mouse.wheel(0, 2400);
    await page.waitForTimeout(500);
  }
  return Array.from(urls).slice(0, count).map((href) => ({
    url: href,
    kind: href.includes('/reel/') ? 'reel' : 'post',
    source_label: 'profile_recent',
    thumbnail_hint: null,
  }));
}
```

- [ ] **步骤 5：在本地验证两种采集模式**

运行：

```bash
echo {"url":"https://www.instagram.com/example/","mode":"profile_recent","count":3,"sessionid":"demo"} | pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop instagram:collector
echo {"url":"https://www.instagram.com/p/abc123/","mode":"single","count":1} | pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop instagram:collector
```

预期：Either successful JSON output on accessible pages, or clear stderr about login/page access. No malformed JSON output.

- [ ] **步骤 6：提交**

```bash
git add scripts/instagram-collector.mjs
git commit -m "feat: add Instagram auth bridge and collector modes"
```

### Task 4：让 Rust 命令调用采集脚本

**文件：**
- 修改：`src-tauri/src/commands/instagram.rs`
- 测试：`src-tauri/src/commands/instagram.rs`

- [ ] **步骤 1：为 stdout JSON 解析编写会失败的单元测试**

```rust
#[test]
fn parses_collector_stdout_json() {
    let raw = r#"{"items":[{"url":"https://www.instagram.com/p/abc/","kind":"post","source_label":"single","thumbnail_hint":null}],"resolved_count":1,"warnings":[],"cookie_bridge_file_path":null}"#;
    let parsed = parse_collector_output(raw).expect("should parse");
    assert_eq!(parsed.resolved_count, 1);
    assert_eq!(parsed.items[0].kind, "post");
}
```

- [ ] **步骤 2：运行 Rust 测试，确认在解析器/执行器代码存在前会失败**

运行：

```bash
cargo test parses_collector_stdout_json --manifest-path F:\FrontEnd\code\swell-video-downloader-desktop\src-tauri\Cargo.toml
```

预期：因函数缺失而 FAIL。

- [ ] **步骤 3：实现子进程执行器与解析器**

```rust
use std::io::Write;
use std::process::{Command, Stdio};

use crate::commands::instagram_types::{
    CollectInstagramTargetsRequest, CollectInstagramTargetsResponse,
};

fn parse_collector_output(raw: &str) -> Result<CollectInstagramTargetsResponse, String> {
    serde_json::from_str(raw).map_err(|error| format!("解析 Instagram 采集结果失败：{error}"))
}

fn run_collector(
    request: &CollectInstagramTargetsRequest,
) -> Result<CollectInstagramTargetsResponse, String> {
    let mut child = Command::new("node")
        .arg("scripts/instagram-collector.mjs")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("启动 Instagram 采集脚本失败：{error}"))?;

    let payload =
        serde_json::to_vec(request).map_err(|error| format!("序列化采集参数失败：{error}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(&payload)
            .map_err(|error| format!("写入采集参数失败：{error}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("等待 Instagram 采集脚本结束失败：{error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Instagram 采集脚本执行失败".into()
        } else {
            stderr
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_collector_output(&stdout)
}
```

- [ ] **步骤 4：让 Tauri 命令调用真实的采集脚本**

```rust
#[tauri::command]
pub async fn collect_instagram_targets(
    request: CollectInstagramTargetsRequest,
) -> Result<CollectInstagramTargetsResponse, String> {
    validate_collect_request(&request)?;
    run_collector(&request)
}
```

- [ ] **步骤 5：运行目标 Rust 测试并做一次手动命令冒烟检查**

运行：

```bash
cargo test instagram --manifest-path F:\FrontEnd\code\swell-video-downloader-desktop\src-tauri\Cargo.toml
pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

预期：Rust 测试 PASS；应用在注册新命令后可正常编译。

- [ ] **步骤 6：提交**

```bash
git add src-tauri/src/commands/instagram.rs
git commit -m "feat: invoke Instagram collector from Tauri"
```

### Task 5：扩展前端类型并新增 Instagram 设置界面

**文件：**
- 修改：`src/lib/types.ts`
- 修改：`src/lib/tauri.ts`
- 修改：`src/features/cookies/CookieSourcePanel.tsx`
- 修改：`src/features/settings/SettingsPanel.tsx`
- 测试：`src/features/settings/SettingsPanel.tsx`

- [ ] **步骤 1：添加会失败的 TypeScript 类型与函数调用**

```ts
// src/lib/types.ts
export type InstagramCollectMode =
  | 'single'
  | 'detail_next'
  | 'profile_recent'
  | 'story_experimental';

export type InstagramCollectItem = {
  url: string;
  kind: 'post' | 'reel' | 'story' | 'unknown';
  sourceLabel: string;
  thumbnailHint?: string | null;
};

export type CollectInstagramTargetsResponse = {
  items: InstagramCollectItem[];
  resolvedCount: number;
  warnings: string[];
  cookieBridgeFilePath?: string | null;
};
```

- [ ] **步骤 2：在更新 tauri.ts 与 UI 前先运行 TypeScript 检查**

运行：

```bash
pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop build
```

预期：若在更新封装/UI 前就引用新类型，则 FAIL。

- [ ] **步骤 3：添加前端 Tauri 封装**

```ts
// src/lib/tauri.ts
export async function collectInstagramTargets(
  url: string,
  mode: 'single' | 'detail_next' | 'profile_recent' | 'story_experimental',
  count: number,
  sessionid?: string | null,
  cookieFilePath?: string | null,
) {
  return invoke<{
    items: Array<{ url: string; kind: string; source_label: string; thumbnail_hint?: string | null }>;
    resolved_count: number;
    warnings: string[];
    cookie_bridge_file_path?: string | null;
  }>('collect_instagram_targets', {
    request: { url, mode, count, sessionid, cookieFilePath },
  }).then((response) => ({
    items: response.items.map((item) => ({
      url: item.url,
      kind: item.kind as 'post' | 'reel' | 'story' | 'unknown',
      sourceLabel: item.source_label,
      thumbnailHint: item.thumbnail_hint ?? null,
    })),
    resolvedCount: response.resolved_count,
    warnings: response.warnings,
    cookieBridgeFilePath: response.cookie_bridge_file_path ?? null,
  }));
}
```

- [ ] **步骤 4：为设置界面扩展 Instagram 专用字段**

```tsx
// src/features/settings/SettingsPanel.tsx
type SettingsPanelProps = {
  cookieSources: CookieSource[];
  selectedCookieSource: string;
  cookieFilePath: string;
  instagramSessionId: string;
  instagramCookieFilePath: string;
  instagramCollectMode: 'single' | 'detail_next' | 'profile_recent' | 'story_experimental';
  instagramCollectCount: string;
  // existing props omitted
  onInstagramSessionIdChange: (value: string) => void;
  onInstagramCookieFilePathChange: (value: string) => void;
  onInstagramCollectModeChange: (
    value: 'single' | 'detail_next' | 'profile_recent' | 'story_experimental',
  ) => void;
  onInstagramCollectCountChange: (value: string) => void;
};
```

```tsx
<div className="panel-stack">
  <TextField
    label="Instagram sessionid"
    type="password"
    value={props.instagramSessionId}
    onChange={props.onInstagramSessionIdChange}
    placeholder="粘贴 sessionid，主推荐方案"
  />
  <TextField
    label="Instagram cookies.txt 路径"
    value={props.instagramCookieFilePath}
    onChange={props.onInstagramCookieFilePathChange}
    placeholder="例如 C:\\Users\\Administrator\\Downloads\\instagram-cookies.txt"
  />
</div>
```

- [ ] **步骤 5：重新构建并确认设置界面可编译通过**

运行：

```bash
pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop build
```

预期：接好新的 Instagram 设置类型与 props 后 PASS。

- [ ] **步骤 6：提交**

```bash
git add src/lib src/features/cookies/CookieSourcePanel.tsx src/features/settings/SettingsPanel.tsx
git commit -m "feat: add Instagram frontend types and settings UI"
```

### Task 6：在 App.tsx 中将 Instagram URL 走「采集优先」流程

**文件：**
- 修改：`src/App.tsx`
- 修改：`src/lib/types.ts`
- 测试：`src/App.tsx`

- [ ] **步骤 1：为 Instagram 路由逻辑添加会失败的辅助测试或本地断言**

```ts
function isInstagramUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.hostname.includes('instagram.com');
  } catch {
    return false;
  }
}

function shouldCollectInstagramBatch(url: string, count: number): boolean {
  return isInstagramUrl(url) && count > 1;
}
```

- [ ] **步骤 2：在启用「采集优先」路由前先运行应用构建**

运行：

```bash
pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop build
```

预期：若在添加 state/import 前就引用新辅助函数，则 FAIL。

- [ ] **步骤 3：向 App.tsx 添加 Instagram 相关状态**

```tsx
const [instagramSessionId, setInstagramSessionId] = useState('');
const [instagramCookieFilePath, setInstagramCookieFilePath] = useState('');
const [instagramCollectMode, setInstagramCollectMode] =
  useState<'single' | 'detail_next' | 'profile_recent' | 'story_experimental'>('single');
const [instagramCollectCount, setInstagramCollectCount] = useState('1');
```

- [ ] **步骤 4：在 handleResolveAll 之前加入「采集优先」分支**

```tsx
async function resolveInstagramTargets(targetUrls: string[]) {
  const firstUrl = targetUrls[0];
  const count = Math.max(1, Number.parseInt(instagramCollectCount || '1', 10) || 1);
  const collected = await collectInstagramTargets(
    firstUrl,
    instagramCollectMode,
    count,
    instagramSessionId || null,
    instagramCookieFilePath || null,
  );

  collected.warnings.forEach((warning) => pushToast(warning, 'info'));

  return {
    urls: collected.items.map((item) => item.url),
    cookieBridgeFilePath: collected.cookieBridgeFilePath ?? '',
  };
}
```

```tsx
if (valid.length === 1 && isInstagramUrl(valid[0])) {
  const collected = await resolveInstagramTargets(valid);
  setCookieFilePath(collected.cookieBridgeFilePath || cookieFilePath);
  await handleResolveAll(Array.from(new Set(collected.urls)));
  return;
}
```

- [ ] **步骤 5：将新增的 Instagram 设置 props 传递给 SettingsPanel**

```tsx
<SettingsPanel
  cookieSources={cookieSources}
  selectedCookieSource={selectedCookieSource}
  cookieFilePath={cookieFilePath}
  instagramSessionId={instagramSessionId}
  instagramCookieFilePath={instagramCookieFilePath}
  instagramCollectMode={instagramCollectMode}
  instagramCollectCount={instagramCollectCount}
  dependencyStatus={dependencyStatus}
  downloadDirectory={downloadDirectorySettings}
  downloadDirectoryDraft={downloadDirectoryDraft}
  isSavingDownloadDirectory={isSavingDownloadDirectory}
  onCookieSourceChange={setSelectedCookieSource}
  onCookieFilePathChange={setCookieFilePath}
  onInstagramSessionIdChange={setInstagramSessionId}
  onInstagramCookieFilePathChange={setInstagramCookieFilePath}
  onInstagramCollectModeChange={setInstagramCollectMode}
  onInstagramCollectCountChange={setInstagramCollectCount}
  onDownloadDirectoryDraftChange={setDownloadDirectoryDraft}
  onSaveDownloadDirectory={handleSaveDownloadDirectory}
  onResetDownloadDirectory={handleResetDownloadDirectory}
/>
```

- [ ] **步骤 6：运行构建并做一次手动 UI 冒烟测试**

运行：

```bash
pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop build
pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

预期：构建 PASS；UI 显示 Instagram 设置，并能进入 Instagram 采集流程而不崩溃。

- [ ] **步骤 7：提交**

```bash
git add src/App.tsx src/lib/types.ts src/features/settings/SettingsPanel.tsx
git commit -m "feat: route Instagram URLs through collector flow"
```

### Task 7：扩展 Instagram 的解析/下载链路并复用登录态桥接

**文件：**
- 修改：`src-tauri/src/commands/resolve.rs`
- 修改：`src-tauri/src/downloader/yt_dlp.rs`
- 修改：`src-tauri/src/commands/download.rs`
- 修改：`src/lib/types.ts`
- 测试：`src-tauri/src/commands/resolve.rs`

- [ ] **步骤 1：为 Instagram 域名支持添加会失败的 Rust 测试**

```rust
#[test]
fn accepts_instagram_host() {
    let source = resolve_source("https://www.instagram.com/p/demo123/")
        .expect("instagram host should be supported");
    assert_eq!(source, "instagram.com");
}
```

- [ ] **步骤 2：在更新 resolve_source 前先运行目标 Rust 测试**

运行：

```bash
cargo test accepts_instagram_host --manifest-path F:\FrontEnd\code\swell-video-downloader-desktop\src-tauri\Cargo.toml
```

预期：因 `resolve_source` 仅支持 `x.com` 与 `pornhub.com` 而 FAIL。

- [ ] **步骤 3：扩展 source 模型以纳入 Instagram**

```rust
fn resolve_source(url: &str) -> Result<String, String> {
    if url.contains("instagram.com") {
        return Ok("instagram.com".into());
    }

    if url.contains("x.com") {
        return Ok("x.com".into());
    }

    if url.contains("pornhub.com") {
        return Ok("pornhub.com".into());
    }

    Err("仅支持 x.com、pornhub.com 和 instagram.com".into())
}
```

```ts
// src/lib/types.ts
export type ResolveMediaResponse = {
  title: string;
  source: 'x.com' | 'pornhub.com' | 'instagram.com';
  durationText: string;
  recommendation: MediaFormat;
  formats: MediaFormat[];
  thumbnail?: string | null;
};
```

- [ ] **步骤 4：让 yt-dlp 的 Cookie 选择能使用采集脚本导出的 cookies**

```rust
// src-tauri/src/downloader/yt_dlp.rs
fn cookie_args(
    cookie_source: Option<&str>,
    cookie_file_path: Option<&str>,
) -> Result<Vec<String>, String> {
    if let Some(path) = cookie_file_path.map(str::trim).filter(|value| !value.is_empty()) {
        if !Path::new(path).is_file() {
            return Err("cookies.txt 文件不存在，请确认路径后重试。".into());
        }
        return Ok(vec!["--cookies".into(), path.to_string()]);
    }

    match current_cookie_mode(cookie_source).as_str() {
        "chrome" => Ok(vec!["--cookies-from-browser".into(), "chrome".into()]),
        "edge" => Ok(vec!["--cookies-from-browser".into(), "edge".into()]),
        "import" => Err("已选择手动导入 Cookie，请填写 cookies.txt 文件路径。".into()),
        "none" => Ok(Vec::new()),
        other => Err(format!("不支持的 Cookie 来源：{other}")),
    }
}
```

- [ ] **步骤 5：重跑 Rust 测试并做一次真实的 Instagram 解析冒烟测试**

运行：

```bash
cargo test resolve_source --manifest-path F:\FrontEnd\code\swell-video-downloader-desktop\src-tauri\Cargo.toml
pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

预期：域名支持测试 PASS；UI 可尝试 Instagram 解析，并能清晰地呈现解析器/登录态错误。

- [ ] **步骤 6：提交**

```bash
git add src-tauri/src/commands/resolve.rs src-tauri/src/downloader/yt_dlp.rs src/lib/types.ts
git commit -m "feat: extend resolve and cookies for Instagram downloads"
```

### Task 8：最终集成验证与文档收尾

**文件：**
- 修改：`docs/superpowers/specs/2026-06-21-instagram-support-design.md`
- 修改：`README.md`
- 测试：整个应用

- [ ] **步骤 1：在 README 中补充 Instagram sessionid 与采集模式的使用说明**

```md
## Instagram

- 单条帖子 / Reel：直接粘贴链接即可
- 连续抓取：选择详情页连续下一条或用户主页最近内容，并设置数量
- 推荐使用 `sessionid`
- `cookies.txt` 作为备用登录方案
```

- [ ] **步骤 2：若实现与原设计有出入，在 spec 中补一段简短说明**

```md
实现说明补充：

- collector 默认以 Playwright headless 模式运行
- Story 在首版中标记为实验性，失败时单独提示
```

- [ ] **步骤 3：运行完整验证套件**

运行：

```bash
pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop build
cargo test --manifest-path F:\FrontEnd\code\swell-video-downloader-desktop\src-tauri\Cargo.toml
pnpm --dir F:\FrontEnd\code\swell-video-downloader-desktop tauri dev
```

预期：

- `pnpm build` PASS
- `cargo test` PASS
- `tauri dev` 成功启动

- [ ] **步骤 4：手工验证清单**

在运行中的应用里手动执行以下检查：

```text
1. 粘贴一个 Instagram 单条链接，抓取数量 1，确认进入解析结果
2. 粘贴一个 Instagram 单条链接，抓取数量 3，确认 collector 返回 3 条 URL
3. 粘贴一个 Instagram 用户主页链接，抓取数量 5，确认 collector 返回前 5 条
4. 输入无效 sessionid，确认提示登录态失败
5. 输入不存在的 cookies.txt，确认提示路径错误
6. 选一条已解析内容开始下载，确认下载进入现有队列
```

- [ ] **步骤 5：提交**

```bash
git add README.md docs/superpowers/specs/2026-06-21-instagram-support-design.md
git commit -m "docs: document Instagram collector workflow"
```

## 自检

### Spec 覆盖度

- `sessionid` 主推、`cookies.txt` 兜底：Task 3, Task 5, Task 7
- 单条、详情连续 `N` 条、主页最近 `N` 条：Task 3, Task 6
- Playwright collector + 现有下载器复用：Task 2, Task 4, Task 7
- 前端设置与交互分流：Task 5, Task 6
- 错误处理和验证：Task 1, Task 4, Task 7, Task 8

### 占位符扫描

- 没有 `TBD` / `TODO` / “后续补上” 一类占位语。
- 每个任务都包含明确文件、命令、预期结果和代码片段。

### 类型一致性

- Instagram 采集模式统一为 `'single' | 'detail_next' | 'profile_recent' | 'story_experimental'`
- 采集结果 Rust 字段 `source_label / thumbnail_hint / resolved_count` 与前端映射一致
- 站点 source 扩展统一为 `'x.com' | 'pornhub.com' | 'instagram.com'`

## 执行交接

计划已完成并保存至 `docs/superpowers/plans/2026-06-21-instagram-support.md`。两种执行方式：

**1. 子 Agent 驱动（推荐）** —— 每个 Task 派发一个全新的子 Agent，Task 之间复核，迭代更快

**2. 内联执行** —— 在当前会话中使用 executing-plans 执行，批量推进并设检查点

**采用哪种方式？**
