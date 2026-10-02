# neo-rimage 架构与升级研究

研究日期：2026-10-02（Asia/Tokyo）

本报告保留实施前的研究和原始基线；文中的版本、测试数量与“本轮”均指研究阶段。后续代码改动、已验证结果和延期事项见 [实施与验证记录](implementation-validation.md)。

## 1. 结论与范围

**值得做有边界的内部重构，不值得在 UI 不变的前提下整体重写。大部分依赖可以迁移到最新稳定版本，但不能把“全部最新”当作正确性指标。**

本轮工作为架构阅读、上游源码和版本查询、当前版本基线验证，以及独立 TypeScript 7 编译探测；不是升级实施。未修改 UI、业务源码、依赖声明、锁文件或用户已有的 README 修改。唯一交付文件是本研究报告。

- 当前主干：React + Valtio + Tauri 2 + Rust 进程内调度器 + 本地图像处理引擎。
- 保留现有分层和状态所有权；优先降低批量处理的数据搬运、内存峰值与维护重复。
- 当前 29 个直接前端依赖中，23 个有较新版本，6 个已是 npm `latest`。版本以实际安装、锁文件及官方 registry 查询为准，而不是仅看 manifest 的范围。
- Rust 图像栈存在明确例外：最新 rimage 0.14.0 自己将 zune-core 固定为 0.5.1，并注明 0.5.3 会导致构建错误。不能对所有 crate 无差别执行更新。
- 未建立真实业务规模的性能基准，也未运行完整安装包、跨平台或 UI 视觉回归；下文的性能优先级是源码分析，不是已测得的加速比例。

## 2. 实际架构

```text
React WebView
  ├─ App / TitleBar / CreateTaskDialog / TaskTable / WorkerList
  ├─ components/ui：仓库内 shadcn/Radix 包装组件
  ├─ create-task feature：表单草稿、输入选择、请求构造
  └─ backend feature：只读后端观察缓存、命令 pending 状态、revision 同步
                  │ invoke / listen
                  ▼
Tauri adapter：原生命令、错误信封、revision 通知桥
                  │
BackendService：连接请求规范化器与调度器
  ├─ RequestNormalizer：输入发现、去重、路径规范化、输出/备份规划
  └─ JobManager：唯一队列与执行状态权威
       ├─ jobs / items / worker slots / pause / retry / cancel / shutdown
       ├─ Executor trait → LocalEngineExecutor
       └─ bounded revision subscription → adapter → frontend refresh
                  │
LocalEngine：单图处理、阶段进度、取消边界、错误/警告与输出事务
  ├─ validation / pipeline / capability / output / runtime
  ├─ vendored rimage：编解码和 resize / quantize / ICC 处理
  └─ zune-image / mozjpeg / ravif / jpeg-encoder 等直接适配
                  │
              本地文件系统
```

Rust 调度器、引擎与编解码器在同一原生应用进程内协作，没有调用 rimage CLI、sidecar 或外部 shell 来处理图片。React 运行在 WebView 中，经 Tauri IPC 连接原生端；这不是 Next.js 或 Electron 项目，`next-themes` 只是主题工具。

### 2.1 模块职责与现状

| 模块 | 当前职责 | 判断 |
| --- | --- | --- |
| React 页面和组件 | 窗口壳、拖放、任务表、并发控制、建任务对话框 | 规模适中，不需要引入路由、SSR 或第二套 UI 框架 |
| create-task feature | 将可编辑字符串表单转换为版本化 CreateJobRequest | 草稿、输入、UI 状态已分离，应保留 |
| backend feature | 观察 snapshot/capabilities，合并刷新并等待命令确认的 revision | 权威边界正确，实际同步流程需要更直接的测试 |
| lib/ipc + Rust domain | 命令、错误、能力、配置、状态与 revision 合约 | 类型双份维护，是扩展时的重要成本 |
| Tauri adapter | 注册命令、重负载 create_job 通过 spawn_blocking 执行 | 大体轻量，但仍保留兼容 scan_dir/get_cpus 路径 |
| BackendService/Normalizer | 文件发现、校验、路径身份与输出冲突规划 | 独立于 Tauri，可单测；批量发现逻辑应进一步统一入口 |
| JobManager | 同步 Mutex 保护状态，执行并发预算、取消、重试与关停 | 状态所有权清晰；热点有全量扫描和逐项创建线程 |
| LocalEngine | preflight → inspect → decode → normalize/operations → encode → sync/commit | 编排不侵入 vendor，边界合理；需要全局内存预算 |
| vendor/rimage | 固定来源、版本和 revision 的编解码/处理库 | 与产品逻辑隔离良好；Windows 资源补丁必须随升级保留 |
| 工程保障 | Vitest、Rust tests、Vite 构建及分阶段设计文档 | 没有发现仓库 CI workflow；现有测试不等于 UI/安装包验收 |

### 2.2 状态与数据流

1. 前端编辑草稿，`buildCreateJobRequest` 完成前端校验和表单转换；后端再次校验，不能依赖前端约束保证文件安全。
2. `create_job` 在阻塞任务中发现文件、规范化路径、规划输出，再提交到 JobManager。
3. JobManager 维护 jobs/items/slots，执行器调用单图 LocalEngine；编码内部不拥有 GUI 调度状态。
4. 原生端 revision 通知通道容量为 1，会合并突发更新；它是“状态变脏”的提示，不是可靠事件日志。
5. 前端收到提示后重新拉取权威 snapshot；刷新请求合并，旧 revision 不覆盖新状态。
6. 取消是阶段边界上的协作式取消；暂停是停止新派发并让已启动工作排空，不应改成前端伪造立即停止。

能力边界也比较诚实：有 10 个输出编码器，但 AVIF 输入解码不可用；EXIF 保留、自动方向修正、processing report 尚未宣称已支持。升级库不会自动使这些应用能力完成。

## 3. 是否值得重构

### 3.1 应保留的设计

- 保留 React、Valtio、Tauri 与 Rust；目前没有证据说明换框架会提高这类桌面图像工具的收益。
- 保留 BackendService → JobManager → Executor → LocalEngine 的边界及后端唯一状态权威。
- 保留同步 CPU-bound 编码与现有锁模型；切成全 Tokio/actor/event-sourcing 并不自动使编码更快。
- 保留手写的编码器 UI 面板和 Rust 枚举匹配；不用为有限格式集合建立插件加载平台或完全动态表单引擎。
- 保留文件提交/回滚、取消/重试、revision 单调性等已有正确性测试。
- 不按文件行数机械拆分：LocalEngine 等文件包含大量测试；拆文件本身不会解决资源或数据流问题。

### 3.2 值得优先做的局部重构

| 优先级 | 具体问题和证据 | 建议 | 收益与代价 |
| --- | --- | --- | --- |
| P0 | TS 合约和 Rust domain 手工维护；invoke 的泛型不是运行时结构验证 | 从 Rust 合约生成前端类型，保留 schema/config version，并验证 serde 标签与字段一致性 | 提高新增编码器/命令的可靠性；不要同时另建完整 schema 框架 |
| P0 | 当前主要测试为 Node 环境纯逻辑/Rust 测试，没有视觉基线 | 先补 UI 截图与关键交互、实际 runtime 同步、安装包烟测的最小门禁 | 这是“UI 不变”升级的前置条件，而不是重写理由 |
| P1 | 前端通过 stat/scan_dir/basename/MIME 选文件；后端又有正式发现与扩展名白名单 | 建立统一后端预扫描/预览入口，基于实际解码能力返回文件和拒绝原因；重工作放阻塞任务 | 消除重复策略与逐文件 IPC；保留当前 UI 及前端即时反馈 |
| P1 | revision bridge 为构造小通知先生成一次完整 snapshot，前端随后又读取完整 snapshot | 提供轻量 revision/timestamp 读取；保留 dirty hint 和全量权威恢复机制 | 明确减少重复投影；先不引入复杂 delta/event replay |
| P1 | active_items/queued_items/item lookup/snapshot slot 会扫描全部 item，且投影持有状态锁；单任务上限为 100,000 个文件 | 先基准测量，再增加 item 定位索引、调度计数及可验证的投影缓存 | 对大批量更有意义；派生索引不是第二份可独立修改的权威状态 |
| P1 | manager 支持分页，但 adapter 获取 job detail、创建 job response 使用全量 items（u32::MAX） | UI 不需要的数据不全量传输；详情暴露 limit/cursor，创建响应优先返回摘要 | 避免大批量创建的序列化/IPC 峰值；需要双方合约同步迁移 |
| P1 | 并发上限按物理 CPU 核数；图像 clone/flatten/RGB 转换会产生多个大缓冲区 | 解码前按头部尺寸和位深估算内存；根据任务峰值与全局预算限制并发 | 比换前端状态库更贴近真实风险；不要只按输入压缩文件大小估计 |
| P2 | 多处重复描述编码器标签、默认值、参数范围、能力和扩展名 | 生成共享合约/能力元数据，前端建立轻量 descriptor 映射；后端校验继续权威 | 改善扩展效率；保留不同编码器的专属 UI 和实现 |
| P2 | 每个 item 启动一个新 OS 线程，而不是长驻 worker pool | 仅在小图批处理基准显示线程开销明显时换固定线程池 | 大图主要成本是编解码；没有测量不必马上重写调度器 |
| P2 | 前端有 623.74 kB 单 JS chunk 的构建警告 | 测启动/打开对话框耗时后再考虑 lazy loading；不机械拆 chunk | 本地桌面 app 不等同网络网站；警告不是卡顿证据 |

### 3.3 “拓展性与高效”的平衡

- 扩展性主要用合约单一来源、清晰 feature 边界、可替换 Executor、能力描述实现，不靠额外层数。
- 高效同时包含开发/维护效率与运行效率：减少重复定义有助前者；减少扫描、IPC 和大缓冲峰值有助后者。
- CPU 与内存预算分开处理。继续由外层 JobManager 控制批量并发，不能升级时顺手启用 rimage 的 `threads` 默认特性，让内外两层并行叠加。
- 对 100/1,000/10,000 个文件，分别测入队延迟、snapshot 延迟/大小、锁等待、吞吐、RSS 峰值与取消延迟；先看瓶颈再加缓存/线程池/虚拟列表。
- 当前任务表按 job 展示，不是按所有 image item 展示；不要把大量图片直接等同于大量表格行。只有实际 jobs 很多且渲染成为瓶颈时才加虚拟化。
- 发布优化配置由应用根 Cargo manifest 控制，vendor 的 release profile 不会自动成为应用 profile。可以单独评估 thin LTO/strip，但不要随意改为 panic=abort，否则现有工作线程 panic 捕获不再提供同等隔离。

## 4. 升级研究的判断口径

“最新”指本次查询时 npm 的 `latest` 稳定版本、crates.io 的 `max_stable_version`，不包括 alpha/beta/RC。版本存在、依赖约束允许、当前代码可迁移、升级后实际验证通过，是四种不同证据。

表中的“建议升级”表示根据上游接口/约束可进入升级批次，不代表已经用新依赖跑过完整构建、UI 和安装包。

- A：同线更新，通常可以优先升级，但仍要回归。
- B：主版本、0.x 的不兼容线更新或构建链变化，需要独立迁移。
- C：明确兼容性例外，暂不追最新。
- V：已经是本次查询的最新版本，无需更改。

### 4.1 全部直接前端依赖

| 依赖 | 当前实际版本 | 最新稳定版本 | 建议 |
| --- | --- | --- | --- |
| React | 19.2.7 | 19.3.0 | A：与 react-dom 和类型配套 |
| react-dom | 19.2.7 | 19.3.0 | A：与 React 同版 |
| @types/react | 19.2.17 | 19.3.0 | A：检查 Valtio snapshot 的复杂类型 |
| @types/react-dom | 19.2.3 | 19.3.0 | A：随 React 类型升级 |
| vite | 7.3.6 | 8.3.2 | B：构建器切换到 Rolldown |
| @vitejs/plugin-react | 4.7.0 | 6.1.1 | B：新版要求 Vite 8，不能单独更新 |
| typescript | 5.8.3 | 7.0.2 | B：已实测现有 baseUrl 配置失败 |
| vitest | 4.1.10 | 5.0.3 | B：Node 版本要求收紧，迁移测试配置 |
| tailwindcss | 4.3.2 | 4.3.3 | A：与 @tailwindcss/vite 同版 |
| @tailwindcss/vite | 4.3.2 | 4.3.3 | A：最新 peer 支持 Vite 8 |
| @tanstack/react-table | 8.21.3 | 9.2.4 | B：迁移 hook/feature/类型 API，保持现有 DOM/CSS |
| @tauri-apps/api | 2.11.1 | 2.12.1 | A：与 Rust 端及 CLI 配套验收 |
| @tauri-apps/cli | 2.11.4 | 2.12.1 | A：同一批次升级原生框架 |
| @tauri-apps/plugin-dialog | 2.7.1 | 2.8.1 | A：JS/Rust 插件配套，检查权限和文件选择 |
| @tauri-apps/plugin-fs | 2.5.1 | 2.6.0 | A：JS/Rust 插件配套，检查路径和 capability |
| radix-ui | 1.6.2 | 1.6.7 | A：焦点、键盘、dialog/select/tooltip 回归 |
| shadcn | 4.13.0 | 4.21.1 | A：更新 CLI/构建 CSS，不自动覆盖本地组件 |
| sonner | 2.0.7 | 2.0.8 | A：toast 布局与交互回归 |
| valtio | 2.3.2 | 2.3.2 | V：没有替换状态库的必要 |
| i18next | 26.3.6 | 26.4.2 | A：语言存储和 fallback 回归 |
| react-i18next | 17.0.9 | 17.0.15 | A：三语文本与 hooks 回归 |
| lucide-react | 1.24.0 | 1.50.0 | A：图标可能有视觉变化，严格 UI 不变时可先冻结 |
| @fontsource-variable/geist | 5.2.9 | 5.3.0 | A：字体/度量可能变化，严格 UI 不变时可先冻结 |
| tailwind-merge | 3.6.0 | 3.7.0 | A：检查冲突 utility 合并结果 |
| class-variance-authority | 0.7.1 | 0.7.1 | V |
| clsx | 2.1.1 | 2.1.1 | V |
| mime | 4.1.0 | 4.1.0 | V：长期是否保留取决于输入发现入口统一 |
| next-themes | 0.4.6 | 0.4.6 | V |
| tw-animate-css | 1.4.0 | 1.4.0 | V |

#### 前端主版本的具体注意点

1. **Vite 8 / plugin-react 6**：现有配置没有复杂 Rollup 自定义插件，迁移范围相对受控。最新 plugin-react 的 peer 是 Vite `^8.0.0`，应作为一个构建链批次处理。Vite 8 改为 Rolldown，CSS minifier 也有变化，不能仅凭成功编译断言 UI 输出等价。不默认开启 React Compiler 或额外 Babel 插件。
2. **TypeScript 7**：独立执行最新编译器，直接得到 TS5102：`Option 'baseUrl' has been removed`，位置为 tsconfig.json:16。第一步是移除 baseUrl，保留相对于配置文件的 `@/*` paths 映射；随后检查项目引用、Vite 配置类型及应用类型。此轮未修改配置，未证明修正后其余检查一定通过。
3. **Vitest 5**：当前测试使用 node 环境，未使用 browser pool、coverage 或自定义 runner，迁移面较小；但其 Node engine 为 `^22.12.0 || ^24.0.0 || >=26.0.0`。项目现有 Node 范围仍允许 20，升级时需同步收紧，建议固定受支持的 24.x LTS。
4. **TanStack Table 9**：当前只有 TaskTable 使用该库，范围相对局部，但 v9 是 API/类型迁移，不是只改 package 版本。保留 Table 组件、列的实际布局和渲染逻辑，改内部 adapter；没有新表格能力需求时，排在构建链与原生引擎之后。
5. **shadcn**：本地 UI 组件是复制到仓库的代码，升级 CLI 不会自动升级它们。与此同时，本项目确实导入 `shadcn/tailwind.css`，所以不能简单删除这个依赖。它可以作为构建/开发依赖归类，但移动到 devDependencies 并不能修复工具链漏洞，也不能代替重新审计。

### 4.2 原生端直接依赖与当前启用的主要图像依赖

| 依赖 | 当前锁定/来源版本 | 最新稳定版本 | 建议 |
| --- | --- | --- | --- |
| tauri | 2.11.5 | 2.12.1 | A：与 JS API/CLI 联动；新版本 MSRV 1.90 |
| tauri-build | 2.6.3 | 2.7.1 | A：随 Tauri 批次，不强行与 tauri 使用同一字面版本 |
| tauri-plugin-dialog | 2.7.1 | 2.8.1 | A：与 JS 插件配套 |
| tauri-plugin-fs | 2.5.1 | 2.6.0 | A：与 JS 插件配套 |
| serde | 1.0.228 | 1.0.229 | A：回归合约序列化 |
| serde_json | 1.0.150 | 1.0.151 | A：回归合约序列化 |
| window-vibrancy（应用直接依赖） | 0.7.1 | 0.8.1 | B：Windows mica/acrylic 和 macOS 材质手动验收 |
| windows-version | 0.1.7 | 0.100.0 | B：0.x 跨线；当前调用的 OsVersion::current().build 仍存在，MSRV 1.95 |
| sysinfo | 0.39.6 | 0.39.6 | V：本版本已要求 Rust 1.95 |
| walkdir | 2.5.0 | 2.5.0 | V |
| rimage | vendor 0.12.4 | 0.14.0 | B：有实际修复价值，但保留资源补丁/特性选择并重新跑图像矩阵 |
| zune-core | 0.5.1 | 0.5.3 | C：上游 rimage 0.14.0 明确锁定 =0.5.1，不建议单独追新 |
| zune-image | 0.5.0 | 0.5.0 | V：等待与新 zune-core 兼容的发布，不替换 git main |
| mozjpeg | 0.10.13 | 0.10.13 | V |
| ravif | 0.13.0 | 0.13.0 | V |
| jpeg-encoder | 0.7.0 | 0.7.1 | A：保留 progressive 直接适配并回归 |
| fast_image_resize（vendor） | 6.0.0 | 6.1.0 | A：跟随 rimage/resize 回归 |
| imagequant（vendor） | 4.4.1 | 4.4.1 | V |
| rgb（vendor） | 0.8.53 | 0.8.53 | V |
| oxipng（vendor） | 10.1.1 | 10.2.1 | A：跟随 rimage，检查输出属性和大小 |
| webp（vendor） | 0.3.1 | 0.3.1 | V |
| lcms2（vendor） | 6.1.1 | 6.2.0 | A：与 ICC/8-bit/16-bit 图像一起回归 |

Cargo.lock 同时含有 window-vibrancy 0.6.0 和 0.7.1；表中的 0.7.1 是应用直接依赖，不代表所有传递依赖都能通过一个版本改动合并。tiff/libavif 等旧 vendor 可选依赖当前未启用、未进入锁定图像链；不为了“全新”启用它们或 CLI-only 特性。传递依赖仍需后续完整漏洞和特性审计。

#### rimage 0.14.0 为什么值得升级，但不能直接替换

- 比对当前固定 revision 到 v0.14.0 的源码，发现 WebP 输入/构造错误、尺寸与分配溢出、ICC 16-bit 处理、编码错误等相关修复；它不是仅有 CLI 外观变化的版本。
- 最低 Rust 1.95，当前本机 1.96.1 满足。新版的 CLI exit code 改动不影响本项目，因为本项目不调用 CLI。
- 新 build.rs 改用 winresource，但仍为 Windows 库消费者编译资源；本项目原有 BUILD_BINARY 条件补丁不能丢，否则会重新引入与 Tauri 的 Windows VERSION 资源冲突。
- 新上游新增 SVG 和系统资源 limits；但应用的 decode/输入白名单/能力描述是自己适配的，不能把“库有功能”当作“GUI 已支持”。接入这些能力是单独的产品任务。
- 新版 AVIF 解码特性改用 dav1d 并要求系统库/pkg-config。当前采用独立 ravif 编码，rimage 的 avif 特性关闭；保留现状升级不等于必须安装 dav1d。只有决定补 AVIF 输入支持时才需要处理该链。
- 当前 `default-features=false` 与显式 feature 列表应保留；不要带入默认 threads/SVG/AVIF/TIFF，从而改变并发、打包或输入能力。
- 新上游 Cargo.toml 明确引用 zune-image issue #439，并将 zune-core 精确固定到 0.5.1。当前项目的宽松 0.5.1 范围可能在锁文件更新时漂移；建议迁移时统一固定兼容版本，不做“所有 patch 都无风险”的假设。
- 保留 JPEG progressive 的直接 jpeg-encoder 适配，以及现有 AVIF 直接 ravif 适配，直到确认上游行为已经满足当前产品合约。
- 更新 vendor 时同步更新版本/revision 来源记录、NEO_RIMAGE_PATCHES、能力来源常量和许可文件；产品 JobManager/GUI 逻辑继续留在 vendor 外部。

### 4.3 工具链

| 工具 | 本机/项目 | 最新查询结果 | 建议 |
| --- | --- | --- | --- |
| Node.js | 24.19.0 | Current 26.10.0；LTS 24.21.0 | 优先同 LTS 更新到 24.21.0，不为版本号切非 LTS |
| pnpm | 项目固定 10.30.3 | latest 12.8.1；latest-10 10.34.6 | 先同线维护更新；12 单独验证锁文件、脚本与干净安装 |
| rustc / cargo | 1.96.1 | stable 1.99.0（2026-10-01） | 可以更新，但不是上述主要依赖升级的阻塞；先固定团队/CI 工具链 |

Rust 应用 edition=2021，vendor edition=2024，可以共存。升级编译器不要求把应用 edition 同时改为 2024；这是另一类迁移，不应混进依赖批次。

## 5. 推荐实施顺序

1. **建立冻结基线**：确认当前锁文件与干净安装可复现；固定工具链；记录三语、最小/最大窗口、drag-drop、dialog/select、任务表、toast、原生材质的截图与交互。
2. **同线维护与工具链审计**：先处理兼容范围内的库更新、shadcn 工具依赖及安全公告；如要求像素级 UI 不变，暂时冻结字体/图标/CSS 变化。每个批次单独评审锁文件。
3. **构建链批次**：Vite 8 + plugin-react 6 + Tailwind Vite 插件；再处理 TypeScript 7 的配置与类型迁移及 Vitest 5/Node engine。按子步骤验证，不混入调度器改造。
4. **Tauri 批次**：Rust tauri/tauri-build、JS API/CLI、dialog/fs 两端配套；单独升级原生材质和 Windows 版本辅助库，检查 capability、原生窗口与安装包。
5. **图像栈批次**：保留 feature 白名单，升级 rimage vendor 与 resize/oxipng/lcms2/jpeg-encoder；保持 zune-core 兼容版本；恢复本地补丁并验证全部编码器、ICC 和资源边界。
6. **高收益内部优化**：统一输入发现、消除通知桥重复 snapshot、完善分页、测量大批量后增加索引/内存预算。合约生成与上述触及合约的改动配套处理。
7. **可选低优先级迁移**：TanStack Table 9；只有性能证据充分时再做线程池、lazy loading、投影缓存或虚拟列表。

各批次的依赖更新、逻辑重构和 UI/能力扩展分开，不做一次性的全量替换。这既降低回滚成本，也能判断变化来自编译器、原生插件还是编码器。

## 6. 本轮验证与局限

### 6.1 已执行

| 检查 | 结果 |
| --- | --- |
| pnpm test | 6 个测试文件、28 个测试通过；当前 Vitest 4.1.10 |
| pnpm build | TypeScript 5.8.3 + Vite 7.3.6 构建通过，1990 modules，Vite 阶段 5.45 s |
| 构建输出 | JS 623.74 kB / gzip 193.26 kB；CSS 64.66 kB / gzip 11.35 kB；有 >500 kB chunk 警告 |
| cargo test --locked --lib | 61 个测试通过，包括调度/取消/重试/输出事务及小图真实编码 |
| TypeScript 7.0.2 独立编译探测 | 当前 tsconfig 在 baseUrl 处报 TS5102；没有改项目配置或锁文件 |
| npm/crates.io 官方版本与源码查询 | 完整直接前端清单、Rust 直接和主要启用图像依赖、peer/MSRV、rimage 差异已核对 |
| pnpm audit --prod --json | 55 条公告匹配：22 high、29 moderate、4 low；所有报告路径均经过 shadcn |
| cargo audit --version | 本机未安装 cargo-audit；未开展 Rust 漏洞数据库审计 |

安全审计的 55 条是公告匹配数，不等于 55 个可被桌面应用远程利用的漏洞。当前代码只从 shadcn 导入构建 CSS，没有使用其 Node CLI/服务端依赖作为 WebView 运行时。仍应更新开发/构建供应链并重跑完整审计，不能通过移动 dependency 类别掩盖问题，也不能声称当前应用已确认不受影响。

### 6.2 升级后仍必须验证

- 最新依赖尚未安装到本项目，除独立 TS 编译器探测外，没有新版本整栈构建成功的证据。
- Rust 本轮测试针对应用 lib；不等于运行了所有 vendor 上游测试、全部真实输入格式/图像大小或恶意文件测试。
- 当前没有做安装包构建/安装、真实 UI 操作、视觉对比、跨 OS 运行或大图峰值内存基准。
- 图像回归至少覆盖所有输出格式、透明/灰度/8-bit/16-bit/ICC、小图和大图、覆盖/备份/取消/Unicode 与 UNC 路径。输出码流可随编码器改变，应该验证语义、可解码性和质量/大小，而不是要求所有字节一致。
- UI 不变的验收应检查 DOM/CSS/字体/图标、焦点与键盘、三语文字和窗口材质；不能用单元测试通过来替代。
- `csp: null` 和 `fs:read-all` 当前较宽，后续按真实功能收窄并做专项安全验证；不在本轮未经验证地直接修改权限。

## 7. 可复核证据

### 7.1 代码定位

- 架构入口：`D:/Projects/HTML/neo-rimage/src-tauri/src/lib.rs:26`。
- 前端控制台：`D:/Projects/HTML/neo-rimage/src/App.tsx:44`。
- 权威 snapshot 同步：`D:/Projects/HTML/neo-rimage/src/features/backend/runtime.ts:98`。
- TS 合约：`D:/Projects/HTML/neo-rimage/src/lib/ipc/contracts.ts:177`；Rust 合约：`D:/Projects/HTML/neo-rimage/src-tauri/src/domain/config.rs:1`。
- 输入重复路径：`D:/Projects/HTML/neo-rimage/src/features/create-task/input-files.ts:11`；正式发现：`D:/Projects/HTML/neo-rimage/src-tauri/src/backend/normalize.rs:56`。
- 通知桥完整投影：`D:/Projects/HTML/neo-rimage/src-tauri/src/tauri_adapter.rs:17`。
- adapter 全量详情：`D:/Projects/HTML/neo-rimage/src-tauri/src/tauri_adapter.rs:48`；创建响应：`D:/Projects/HTML/neo-rimage/src-tauri/src/backend/service.rs:71`。
- snapshot 扫描：`D:/Projects/HTML/neo-rimage/src-tauri/src/jobs/manager/projection.rs:21`；item 查找：`D:/Projects/HTML/neo-rimage/src-tauri/src/jobs/manager.rs:783`。
- 活跃 item 全量计数：`D:/Projects/HTML/neo-rimage/src-tauri/src/jobs/manager.rs:850`；逐项线程派发：`D:/Projects/HTML/neo-rimage/src-tauri/src/jobs/manager.rs:927`。
- JPEG/AVIF 缓冲准备：`D:/Projects/HTML/neo-rimage/src-tauri/src/engine/pipeline.rs:190`；`D:/Projects/HTML/neo-rimage/src-tauri/src/engine/pipeline.rs:241`。
- 当前能力边界：`D:/Projects/HTML/neo-rimage/src-tauri/src/engine/capability.rs:18`。
- Table v8 接口：`D:/Projects/HTML/neo-rimage/src/components/TaskTable.tsx:91`。
- shadcn CSS 导入：`D:/Projects/HTML/neo-rimage/src/index.css:3`。
- TS 7 配置阻塞：`D:/Projects/HTML/neo-rimage/tsconfig.json:16`。
- vendor 补丁记录：`D:/Projects/HTML/neo-rimage/src-tauri/vendor/rimage/NEO_RIMAGE_PATCHES.md:1`；补丁：`D:/Projects/HTML/neo-rimage/src-tauri/vendor/rimage/build.rs:6`。

### 7.2 查询来源与复核命令

官方来源：npm Registry 的 latest/peer/engine 元数据；crates.io 的 max_stable_version/MSRV；Vite 8 官方迁移指南；TypeScript v7.0.2 官方发布；TanStack Table v9 官方迁移指南；Tauri 2.12.1 官方发布；rimage v0.14.0 官方发布、Cargo.toml/build.rs 和与当前 revision 的源码比较；zune-image issue #439；Node 官方发行列表；Rust stable channel manifest。对应可点击的公开引用见本聊天的研究摘要。

```powershell
pnpm list --depth 0
pnpm outdated --format json
pnpm test
pnpm build
pnpm audit --prod --json
pnpm --package=typescript@7.0.2 dlx tsc --project tsconfig.json --noEmit
```

原生验证在 `D:/Projects/HTML/neo-rimage/src-tauri` 执行：

```powershell
cargo test --locked --lib
```

最后建议：先保住 UI/结果语义与清晰状态权威，再通过分批升级和针对性的内部优化提高维护与运行效率；“合理的最新组合”优于“所有模块的最大版本号”。
