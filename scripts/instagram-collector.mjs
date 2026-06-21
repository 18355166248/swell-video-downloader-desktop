import { chromium } from 'playwright';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

// ---------------------------------------------------------------------------
// stdin/stdout JSON plumbing
// ---------------------------------------------------------------------------

async function readJsonStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  const text = Buffer.concat(chunks).toString('utf8').trim();
  return text ? JSON.parse(text) : {};
}

function writeJsonStdout(payload) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

function normalizeInstagramUrl(raw) {
  const parsed = new URL(raw);
  parsed.hash = '';
  parsed.search = '';
  return parsed.toString();
}

function classifyContentUrl(href) {
  if (href.includes('/reel/') || href.includes('/reels/')) return 'reel';
  if (href.includes('/p/')) return 'post';
  if (href.includes('/stories/')) return 'story';
  return 'unknown';
}

// ---------------------------------------------------------------------------
// Cookie bridge helpers
// ---------------------------------------------------------------------------

function buildCookieBridgeLine(domain, name, value) {
  const includeSub = domain.startsWith('.') ? 'TRUE' : 'FALSE';
  return [domain, includeSub, '/', 'TRUE', '0', name, value].join('\t');
}

async function writeTempCookiesFile(cookies) {
  const filePath = path.join(os.tmpdir(), `swell-instagram-${Date.now()}.cookies.txt`);
  const lines = ['# Netscape HTTP Cookie File', ...cookies];
  await fs.writeFile(filePath, `${lines.join('\n')}\n`, 'utf8');
  return filePath;
}

// Export the live BrowserContext cookies into a temporary cookies.txt that
// yt-dlp can reuse, so collector and downloader share one auth state.
async function exportContextCookies(context) {
  const cookies = await context.cookies();
  if (!cookies.length) return null;
  const lines = cookies.map((cookie) =>
    buildCookieBridgeLine(cookie.domain, cookie.name, cookie.value),
  );
  return writeTempCookiesFile(lines);
}

// ---------------------------------------------------------------------------
// Auth: sessionid injection and cookies.txt import
// ---------------------------------------------------------------------------

async function applySessionCookie(context, sessionid) {
  if (!sessionid?.trim()) return false;
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
  return true;
}

function parseNetscapeCookies(text) {
  const cookies = [];
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;
    const parts = line.split('\t');
    if (parts.length < 7) continue;
    const [domain, , cookiePath, secure, expires, name, value] = parts;
    if (!domain.includes('instagram.com')) continue;
    const cookie = {
      name,
      value,
      domain,
      path: cookiePath || '/',
      secure: secure?.toUpperCase() === 'TRUE',
    };
    const expiresNum = Number.parseInt(expires, 10);
    if (Number.isFinite(expiresNum) && expiresNum > 0) cookie.expires = expiresNum;
    cookies.push(cookie);
  }
  return cookies;
}

async function applyCookiesFile(context, cookieFilePath) {
  if (!cookieFilePath?.trim()) return false;
  const text = await fs.readFile(cookieFilePath.trim(), 'utf8');
  const cookies = parseNetscapeCookies(text);
  if (!cookies.length) return false;
  await context.addCookies(cookies);
  return true;
}

// ---------------------------------------------------------------------------
// Collection modes
// ---------------------------------------------------------------------------

async function collectSingle(page, url) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  const finalUrl = normalizeInstagramUrl(page.url());
  return [
    {
      url: finalUrl,
      kind: classifyContentUrl(finalUrl),
      source_label: 'single',
      thumbnail_hint: null,
    },
  ];
}

async function collectProfileRecent(page, url, count) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  const urls = new Set();
  for (let i = 0; i < 12 && urls.size < count; i += 1) {
    const hrefs = await page
      .locator('a[href*="/p/"], a[href*="/reel/"]')
      .evaluateAll((nodes) => nodes.map((node) => node.href));
    hrefs.forEach((href) => urls.add(normalizeInstagramUrl(href)));
    if (urls.size >= count) break;
    await page.mouse.wheel(0, 2400);
    await page.waitForTimeout(600);
  }
  return Array.from(urls)
    .slice(0, count)
    .map((href) => ({
      url: href,
      kind: classifyContentUrl(href),
      source_label: 'profile_recent',
      thumbnail_hint: null,
    }));
}

async function collectDetailNext(page, url, count) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  const ordered = [];
  const seen = new Set();
  for (let i = 0; i < count; i += 1) {
    const current = normalizeInstagramUrl(page.url());
    if (seen.has(current)) break;
    seen.add(current);
    ordered.push(current);
    if (ordered.length >= count) break;

    const nextButton = page
      .locator('a[aria-label="Next"], button[aria-label="Next"], svg[aria-label="Next"]')
      .first();
    if ((await nextButton.count()) === 0) break;
    await nextButton.click({ timeout: 5000 }).catch(() => {});
    // Wait for the canonical URL or main content node to change.
    await page
      .waitForFunction((prev) => window.location.href !== prev, current, { timeout: 5000 })
      .catch(() => {});
    await page.waitForTimeout(400);
  }
  return ordered.map((href) => ({
    url: href,
    kind: classifyContentUrl(href),
    source_label: 'detail_next',
    thumbnail_hint: null,
  }));
}

async function collectStory(page, url, count) {
  // Experimental: best-effort capture of the current visible story entry only.
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  const current = normalizeInstagramUrl(page.url());
  return [
    {
      url: current,
      kind: 'story',
      source_label: 'story_experimental',
      thumbnail_hint: null,
    },
  ].slice(0, Math.max(1, count));
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  const input = await readJsonStdin();
  const url = normalizeInstagramUrl(input.url);
  const count = Math.max(1, Number.parseInt(input.count ?? 1, 10) || 1);
  const warnings = [];

  // A single-link entry with count > 1 means "keep grabbing the next ones",
  // which is the detail-next behavior.
  let mode = input.mode || 'single';
  if (mode === 'single' && count > 1) {
    mode = 'detail_next';
    warnings.push(`数量为 ${count}，已自动切换为「详情页连续下一条」模式。`);
  }

  // Instagram is typically reached through a local proxy on dev machines.
  // Honor an explicit proxy from the caller, then common proxy env vars.
  const proxyServer =
    input.proxy ||
    process.env.HTTPS_PROXY ||
    process.env.HTTP_PROXY ||
    process.env.ALL_PROXY ||
    null;
  const launchOptions = { headless: true };
  if (proxyServer) launchOptions.proxy = { server: proxyServer };

  const browser = await chromium.launch(launchOptions);
  try {
    const context = await browser.newContext();

    let authed = false;
    try {
      authed = await applySessionCookie(context, input.sessionid);
      if (!authed) authed = await applyCookiesFile(context, input.cookie_file_path);
    } catch (error) {
      warnings.push(`登录态注入失败：${error.message}`);
    }

    const page = await context.newPage();

    // When authed via sessionid, warm up the session so Instagram can issue
    // any supplementary cookies, then export a shared cookies.txt bridge.
    let cookieBridgeFilePath = null;
    if (authed && input.sessionid?.trim()) {
      try {
        await page.goto('https://www.instagram.com/', { waitUntil: 'domcontentloaded' });
        await page.waitForTimeout(800);
      } catch (error) {
        warnings.push(`登录态预热失败：${error.message}`);
      }
    }

    let items = [];
    try {
      switch (mode) {
        case 'profile_recent':
          items = await collectProfileRecent(page, url, count);
          break;
        case 'detail_next':
          items = await collectDetailNext(page, url, count);
          break;
        case 'story_experimental':
          items = await collectStory(page, url, count);
          break;
        case 'single':
        default:
          items = await collectSingle(page, url);
          break;
      }
    } catch (error) {
      if (mode === 'story_experimental') {
        warnings.push(`实验性 Story 采集失败：${error.message}`);
      } else {
        throw error;
      }
    }

    if (count > 1 && items.length < count) {
      warnings.push(`仅采集到 ${items.length} 条，少于请求的 ${count} 条`);
    }

    // Only export a cookies.txt bridge when we actually injected an auth state.
    // An anonymous export would otherwise override the user's logged-in browser
    // cookies downstream and turn a working resolve into an auth failure.
    if (authed) {
      try {
        cookieBridgeFilePath = await exportContextCookies(context);
      } catch (error) {
        warnings.push(`导出 Cookie 桥接文件失败：${error.message}`);
      }
    }

    writeJsonStdout({
      items,
      resolved_count: items.length,
      warnings,
      cookie_bridge_file_path: cookieBridgeFilePath,
    });
  } finally {
    await browser.close();
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack || error.message}\n`);
  process.exit(1);
});
