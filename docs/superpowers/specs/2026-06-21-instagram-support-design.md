# Swell 桌面下载器 Instagram 支持设计说明

日期：2026-06-21  
状态：待评审  
范围：为现有桌面下载器新增 Instagram 单链接解析与登录态驱动的连续采集能力

## 一、摘要

当前项目已经具备以下基础能力：

- Tauri 命令层与 React 前端交互
- `yt-dlp` 负责主解析与下载
- `ffmpeg` 负责后处理
- 浏览器 Cookie / `cookies.txt` 配置链路
- 多链接解析与下载队列

本次扩展目标不是重写一套 Instagram 下载器，而是在现有结构上新增一条“浏览器自动化采集 + 现有下载器复用”的混合链路。

新增能力包括：

- 支持 Instagram 单条 `post / reel` 链接解析与下载
- 支持单图、视频、轮播内容
- 支持从详情页开始，按“下一条”连续抓取前 `N` 条内容
- 支持从用户主页抓取最近 `N` 条内容
- 主推用户直接粘贴 Instagram `sessionid`
- 同时保留 `cookies.txt` 作为兜底路径

第一版不把 Instagram 扩展成一个“通用账号爬虫平台”，而是围绕“给定入口 URL，抓取指定数量内容并复用现有下载流程”来做。

## 二、核心产品决策

### 1. 采用混合架构，而不是纯 `yt-dlp` 或纯浏览器自动化

本次 Instagram 支持采用两段式流程：

1. 浏览器自动化负责建立登录态、遍历页面、收集 canonical URL
2. 现有 `yt-dlp` 下载链负责解析格式、下载资源、后处理与进度展示

原因如下：

- 纯 `yt-dlp` 适合“给定单链接直接解析”，但不适合“在页面里点下一条继续抓”
- 纯浏览器自动化可以抓页面内容，但不适合替代现有下载链承担格式解析与下载管理
- 混合模式最容易复用当前项目已经稳定的解析、下载、进度和错误处理能力

### 2. `sessionid` 作为主登录入口，`cookies.txt` 作为兜底

第一版主推用户直接粘贴 Instagram `sessionid`，因为这最符合目标用户的实际使用方式，也比读取浏览器 Cookie 更可控。

同时保留 `cookies.txt`，原因如下：

- `sessionid` 可能失效、过期或不足以覆盖某些页面访问
- 某些用户已经有现成的 Instagram `cookies.txt`
- 出现登录态问题时，`cookies.txt` 是最直接的恢复路径

### 3. 首版同时支持两种采集入口

首版明确支持：

- 从某条 Instagram 内容详情页开始，连续抓取 `N` 条
- 从某个用户主页开始，抓取最近 `N` 条

默认行为：

- 如果输入是一条内容链接，默认抓 1 条
- 如果用户指定数量大于 1，则进入连续采集模式
- 如果输入是用户主页链接，则按最近内容抓取前 `N` 条

## 三、第一版支持范围

### 支持

- `instagram.com/p/...`
- `instagram.com/reel/...`
- `instagram.com/<username>/`
- 单图帖子
- 视频帖子
- 轮播帖子
- 已登录可见的普通内容

### 实验性支持

- Story

Story 的限制包括：

- 时效性强
- 登录态依赖更重
- 页面结构和切换节奏更容易变化

因此第一版可以接入，但必须在 UI 和错误提示中明确标注“实验性”。

### 第一版非目标

- 不做账号全量备份
- 不做无限滚动式大规模采集
- 不做定时抓取
- 不做评论、点赞、元数据分析
- 不做“输入一个用户名自动抓完整历史”的承诺

## 四、用户输入与采集模式

新增三种 Instagram 采集模式：

### 1. 当前链接

适用于：

- 单条 `post`
- 单条 `reel`
- 用户只想抓当前这一条

行为：

- 默认数量为 1
- 如果链接本身已是 canonical 内容链接，则优先直接标准化并交给现有解析链

### 2. 详情页连续下一条

适用于：

- 用户从某一条打开的内容详情页开始，希望继续抓后面的内容

行为：

- 从当前内容开始记录 canonical URL
- 点击“下一条”
- 等待页面内容或 URL 变化
- 直到达到指定数量 `N`

### 3. 用户主页最近内容

适用于：

- 用户输入主页链接，希望抓最近 `N` 条内容

行为：

- 打开主页
- 收集页面中可见的 `/p/` 与 `/reel/` 链接
- 数量不足时继续滚动加载
- 去重后截取前 `N` 条

## 五、总体架构

保持现有架构主体不变，在其上新增 Instagram Collector 支线。

整体分层如下：

1. 前端输入与配置层  
   负责 Instagram 入口识别、采集模式选择、数量输入、登录态配置和结果展示。

2. Tauri Instagram 命令层  
   负责参数校验、调用外部采集脚本、接收 JSON 结果、回传前端。

3. 浏览器自动化采集层  
   使用 Playwright 建立 Instagram 登录态，打开页面，遍历内容并输出 canonical URL 列表。

4. 现有解析与下载层  
   使用 `yt-dlp` 和 `ffmpeg` 对采集结果逐条解析、下载和后处理。

5. 登录态桥接层  
   负责将 `sessionid` 或 `cookies.txt` 转换成 Playwright 与 `yt-dlp` 都能复用的临时 Cookie 状态。

## 六、技术路线

### 1. 浏览器自动化使用 Playwright

理由：

- 支持稳定的浏览器上下文与 Cookie 注入
- 适合做“打开页面、采集链接、点击下一条、滚动主页”的交互流程
- 可以导出上下文状态，方便桥接到 `yt-dlp`
- Node 脚本形式接入 Tauri 成本较低

### 2. 不把 Playwright 直接嵌入 Rust 主逻辑

建议使用独立脚本：

- `scripts/instagram-collector.mjs`

由 Tauri 命令通过子进程调用。

原因：

- Playwright 在 Node 生态里更成熟，调试更顺手
- 与现有前端工具链兼容
- 避免在 Rust 层引入一整套浏览器自动化复杂度
- Instagram 页面变化时，只需迭代采集脚本，不必污染下载核心

### 3. 下载仍完全复用现有 `yt-dlp` 链

采集脚本不直接下载媒体文件。

它的输出是：

- 规范化后的 Instagram canonical URL 列表
- 可选标题提示
- 可选缩略图提示
- 登录态桥接生成的临时 Cookie 文件路径

然后由当前项目已有的：

- `resolve_media`
- `start_download`
- 下载队列
- 进度事件

继续承担下载职责。

## 七、登录态设计

### 1. `sessionid` 主路径

用户在设置区粘贴 `sessionid` 后，系统执行以下步骤：

1. Playwright 创建全新 `BrowserContext`
2. 向 `.instagram.com` 注入 `sessionid`
3. 打开 `https://www.instagram.com/`
4. 等待页面稳定并确认是否处于登录态
5. 读取 context 内的完整 cookies
6. 导出为临时 `cookies.txt`

这份临时 `cookies.txt` 随后作为本次采集与下载的统一登录态来源。

设计原因：

- 直接把 `sessionid` 传给 `yt-dlp` 不够稳
- 首次页面访问后，Instagram 可能补发其他辅助 cookies
- 导出完整 cookies 后，Playwright 与 `yt-dlp` 可以共用同一份登录态

### 2. `cookies.txt` 备用路径

如果用户提供的是 `cookies.txt`：

- Playwright 读取并转换为上下文 cookies
- `yt-dlp` 直接使用该文件

如果 Playwright 无法直接消费该文件格式，则允许脚本只在下载阶段复用 `yt-dlp --cookies`，而采集阶段改为提示用户优先使用 `sessionid`。

### 3. 安全要求

- `sessionid` 默认不完整回显
- 日志中不得打印明文 `sessionid`
- 临时 `cookies.txt` 应写入系统临时目录
- 使用完毕后删除，或在短期缓存策略下定时清理
- 本地持久化时必须至少做最小可见性保护，不在普通 UI 文本里明文展示

## 八、自动化行为设计

### 1. 单条内容采集

对于 `/p/` 或 `/reel/`：

- 优先标准化 URL
- 可选择是否打开页面核验登录态与可访问性
- 返回 1 条 `InstagramCollectItem`

如果用户要求数量大于 1，则进入“详情页连续下一条”模式。

### 2. 详情页连续采集

算法要求：

1. 打开起始详情页
2. 识别当前内容的 canonical URL
3. 记录并去重
4. 查找“下一条”控件并点击
5. 等待 URL 或主内容节点变化
6. 继续直到采集满 `N` 条，或无法继续

关键要求：

- 不依赖易变的视觉 class 作为唯一定位方式
- 以 URL 变化、语义属性和内容稳定信号为主
- 遇到重复 URL 必须跳过，避免死循环

### 3. 用户主页采集

算法要求：

1. 打开用户主页
2. 提取当前可见帖子链接
3. 过滤成 `/p/` 和 `/reel/`
4. 去重
5. 数量不足时滚动页面
6. 获取前 `N` 条

该模式优先于“打开第一个帖子后再点下一条”，因为它对页面结构依赖更小。

### 4. Story 采集

Story 只做实验性支持。

行为要求：

- 如果用户给的是 Story 入口，则尽力采集当前可见 Story
- 连续采集仅在脚本能可靠判断下一 Story 时启用
- 失败时必须给出“实验性能力失败”的单独提示，而不是误报为通用下载故障

## 九、数据模型与接口

### 1. 新增 Tauri 命令

新增：

- `collect_instagram_targets(payload)`

输入建议包含：

- `url`
- `mode`
- `count`
- `sessionid`
- `cookieFilePath`

输出建议包含：

- `items`
- `resolvedCount`
- `warnings`
- `cookieBridgeFilePath`

### 2. 采集结果模型

新增前端类型：

- `InstagramAuthMode = 'sessionid' | 'cookies_txt'`
- `InstagramCollectMode = 'single' | 'detail_next' | 'profile_recent' | 'story_experimental'`

新增采集项结构：

```ts
type InstagramCollectItem = {
  url: string;
  kind: 'post' | 'reel' | 'story' | 'unknown';
  sourceLabel: string;
  thumbnailHint?: string | null;
};
```

### 3. 与现有解析链的衔接

前端行为：

- 如果是 Instagram 且仅需单条，可直接进入现有 `resolveMedia`
- 如果是 Instagram 且需要批量采集，则先调用 `collectInstagramTargets`
- 再把返回的 `items[].url` 交给现有 `handleResolveAll`

这样现有 `ResolveBoard`、下载队列与任务状态都可以保留。

## 十、仓库改动边界

建议新增或修改以下文件。

### 新增

- `src-tauri/src/commands/instagram.rs`
- `scripts/instagram-collector.mjs`
- `scripts/instagram-cookie-bridge.mjs`（如需要拆分登录态桥接逻辑）

### 修改

- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/resolve.rs`
- `src-tauri/src/commands/download.rs`
- `src/lib/tauri.ts`
- `src/lib/types.ts`
- `src/App.tsx`
- `src/features/settings/SettingsPanel.tsx`
- `src/features/cookies/CookieSourcePanel.tsx`
- `src/styles.css`

### 文件职责

- `instagram.rs`：命令入口与脚本调用
- `instagram-collector.mjs`：Playwright 自动化采集
- `resolve.rs`：扩展域名白名单与 Instagram 响应映射
- `download.rs`：支持传入 Instagram 采集后复用的 Cookie 文件
- `types.ts`：新增 Instagram 类型
- `App.tsx`：采集前置分流与批量结果注入
- 设置相关组件：Instagram 登录态配置与引导

## 十一、前端交互设计

### 1. 设置区新增 Instagram 配置

新增独立的 `Instagram 访问` 设置块，而不是把所有逻辑都塞进通用 Cookie 设置。

字段包括：

- `sessionid`
- `cookies.txt 路径`
- 采集模式
- 抓取数量

交互要求：

- `sessionid` 是主推荐路径
- `cookies.txt` 明确标为备用方案
- `sessionid` 输入框默认遮罩显示
- UI 提供简短获取说明

### 2. 主页面输入行为

当前输入区保持不变，但针对 Instagram 增加分流：

- 单条且数量 1：走普通解析
- 单条且数量 > 1：先采集，再批量解析
- 用户主页：先采集，再批量解析

### 3. 结果展示

批量采集后：

- 结果继续进入当前 `ResolveBoard`
- 每条内容仍可单独选择格式下载
- 顶部增加本次采集摘要，例如“已采集 5 条 Instagram 内容”

## 十二、错误处理

Instagram 相关错误必须单独分类，不能全部落入现有泛化错误里。

建议新增错误分类：

- `instagram_auth_invalid`
- `instagram_cookie_bridge_failed`
- `instagram_automation_failed`
- `instagram_collect_incomplete`
- `instagram_story_experimental_failed`

规则要求：

- 登录失败时明确提示 `sessionid` 失效或登录态不可用
- 自动化失败时提示“页面结构或访问流程变化”
- 采集部分成功时允许返回已采集结果与 warning，而不是整批失败
- Story 失败时单独提示实验性能力不稳定

## 十三、测试策略

### 1. Rust 单元测试

覆盖：

- Instagram URL 识别
- 采集命令参数校验
- `sessionid / cookies.txt` 优先级
- 采集结果映射到现有解析队列

### 2. Node 脚本测试

覆盖：

- URL 标准化
- 主页链接提取与去重
- 详情页下一条迭代控制
- Cookie bridge 导出格式

### 3. 手工集成测试

至少覆盖：

- 单条公开视频
- 单条登录后可见内容
- 详情页连续抓 3 条
- 用户主页抓最近 5 条
- 轮播帖子
- Story
- `sessionid` 无效
- `cookies.txt` 路径错误

## 十四、主要风险

1. Instagram 页面结构变化快，自动化选择器可能失效。
2. `sessionid` 不一定在所有情况下都足以稳定恢复完整登录态。
3. Playwright 首次安装浏览器体积较大，会增加初始化成本。
4. Story 的稳定性会显著低于普通帖子与 Reels。
5. Windows 上新增 Playwright 运行时后，需要确认打包与本地路径管理策略。

## 十五、首版交付范围

第一阶段完成标准：

- 能识别 Instagram 单条链接与用户主页链接
- 能用 `sessionid` 建立可复用登录态
- 能采集单条、详情连续 `N` 条、主页最近 `N` 条
- 能把采集结果接入现有解析与下载队列
- 对失败场景给出清晰可恢复提示

首版允许存在的限制：

- Story 标为实验性
- 页面结构变化导致的局部失效需后续迭代修复
- 不承诺支持所有私密或特殊权限内容

## 十六、成功标准

如果满足以下条件，则视为本次 Instagram 扩展设计目标达成：

- 用户可在应用内配置 `sessionid` 或 `cookies.txt`
- 用户可从单条链接抓当前内容
- 用户可从详情页连续抓取前 `N` 条
- 用户可从用户主页抓取最近 `N` 条
- 采集结果能无缝进入现有解析与下载流程
- 失败时用户能判断问题出在登录态、自动化还是下载解析阶段
