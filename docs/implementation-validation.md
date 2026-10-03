# 内部重构、依赖升级与验证记录

## 范围

本次实施保留 React/Valtio/Tauri/JobManager/LocalEngine 的分层和后端状态权威，不重写 UI，也不引入 actor、事件回放、线程池或另一套状态库。按历史 `type(scope): description` 格式分批验证、提交；未创建新分支，未修改或提交用户已有的 `README.md` 和 `.obsidian/`。

Node `24.19.0`、Rust `1.96.1`、pnpm `10.30.3`、Vite `7.3.6`、TypeScript `5.8.3`、Vitest `4.1.10`、React Vite 插件 `4.7.0`、Tailwind 和其 Vite 插件 `4.3.2`、Tauri CLI `2.11.4` 保持原版本。仅配套更新 Tauri 的框架构建辅助 crate，并修补 PostCSS/nanoid 等传递依赖的安全版本；不实施此前研究中的编译工具链主版本迁移。

## 已实施的重构

- Rust domain 成为 IPC DTO 的类型来源，使用仅测试环境启用的 `ts-rs` 生成 `src/lib/ipc/contracts.ts`。schema/config version 及分页常量也来自 Rust。正常 Rust 测试和 `pnpm contracts:check` 会检测未同步的合约；`pnpm contracts:generate` 更新生成文件。
- 保持 `u64` 的 JSON number 表示。新增透明 ID、revision、timestamp 的 serde/TypeScript 原始类型一致性测试；生成不会引入 `bigint`。
- revision bridge 只读取 revision/timestamp，不再先构造一份完整 backend snapshot。容量为 1 的 dirty-hint 通知与前端完整快照恢复机制保持不变。
- 新增 10 项真实 runtime 模块测试：请求合并、最高目标 revision、拒绝旧快照、失败恢复、schema 校验、异步监听注册与卸载、丢 revision 后恢复等。
- 新增版本化 `scan_inputs` 命令，将文件/目录识别、递归、排序、扩展名白名单、canonical 去重和限制复用到后端预扫描；重工作仍在 `spawn_blocking`。前端一次扫描取代逐文件 `stat`/`basename` IPC 和 MIME 猜测。
- 预扫描返回文件名、规范化路径及拒绝原因。Windows 展示路径移除 verbatim 前缀但保留大小写。创建任务仍重新验证输入，测试覆盖预览后文件被删除的情况；不会把预扫描当作安全授权。
- 移除已无调用方的旧 `scan_dir`/`get_cpus` 命令、前端 MIME/fs 依赖，以及应用对 fs 插件的直接注册和宽泛 fs capability。新版 dialog 仍间接依赖 Rust fs 插件，不宣称原生依赖图中已完全消除该 crate。
- job detail 查询暴露 `offset`/`limit`，默认 200、上限 1,000；`total` 保持完整任务总数，调用者通过分页继续读取。测试覆盖末页、越界、零长度和最大限制。
- UI 创建任务显式发送 `includeItems: false`，响应保留 job 摘要但不传输全部 item。旧请求省略该字段仍默认返回 items，保留兼容性；IPC/config version 均仍为 1。
- 单 item 重试的归属检查不再生成全量详情，直接检查任务记录。末页 item 的归属不受分页限制影响；未引入新的可独立修改的索引权威。

## 依赖结果

| 模块 | 实施后的版本 |
| --- | --- |
| React / React DOM / 两者类型 | 19.3.0 |
| Tauri Rust / JS API | 2.12.1 |
| tauri-build / 关联框架宏 | 2.7.1 |
| dialog Rust / JS | 2.8.1 |
| Radix UI | 1.6.7 |
| i18next / react-i18next | 26.4.2 / 17.0.15 |
| Sonner / tailwind-merge / shadcn | 2.0.8 / 3.7.0 / 4.21.1 |
| serde / serde_json | 1.0.229 / 1.0.151 |
| vendored rimage | 0.14.0 |
| fast_image_resize / oxipng / lcms2 | 6.1.0 / 10.2.1 / 6.2.0 |
| jpeg-encoder | 0.7.1 |
| zune-core | 固定为 =0.5.1，与 rimage 上游一致 |

rimage 对应官方 `v0.14.0` tag，revision `978a87075ad60fbbd033ba851036fbe84e9e0f47`；原始选定文件逐一核对。唯一源码定制仍是 Windows CLI VERSION 资源的 `build-binary` guard，并增加对应环境变量的重构建监听。来源、补丁与许可记录在 `src-tauri/vendor/rimage/NEO_RIMAGE_PATCHES.md`。

保留显式 library feature 白名单，不启用 rimage CLI、上游并行编码、SVG/TIFF/AVIF 解码等额外功能。现有 AVIF 输出仍通过应用直接使用的 ravif 提供。补充两份同 revision 的 WebP/ICC 原始小夹具，使启用功能的上游库测试可以在本仓库复现。

字体 Geist `5.2.9`、lucide 图标 `1.24.0`、动画 CSS `1.4.0`、TanStack Table `8.21.3` 保持原版本。应用直接调用的 window-vibrancy `0.7.1` 和 windows-version `0.1.7` 暂不做破坏性版本迁移；Tauri 自身传递使用的材质辅助库随框架更新。

## 验证结果

| 验证 | 结果 |
| --- | --- |
| pnpm install --frozen-lockfile | 通过 |
| pnpm test | 7 文件、44 项通过，原有文件合并测试保留 |
| pnpm build | TypeScript 检查和 Vite production 构建通过 |
| pnpm contracts:check | Rust/TS 合约一致 |
| cargo test --locked --lib | 应用库 71 项通过，含所有 10 个输出编码器的真实格式验证 |
| cargo test --locked -p rimage --lib | 启用功能的上游库 111 项通过，含 8/16-bit/f32、颜色空间、WebP、ICC、resize overflow 和资源限制测试 |
| cargo fmt --check | 通过 |
| cargo clippy --locked --lib --tests -- -D warnings | 通过 |
| pnpm tauri build --debug --bundles nsis | x64 Windows debug 应用和 NSIS 安装包构建通过 |
| 真实 Tauri IPC | 预扫描、创建摘要、并发设为 2、恢复调度、分页上限和真实 PPM/WebP → PNG 转换通过；两个 item 均 succeeded，输出 PNG signature 正确 |
| pnpm audit --prod --json | 0 项匹配公告 |
| pnpm audit --json | 无 high/critical；仍有 2 项 moderate，来自保留版本的 Vitest/@vitest/mocker |

ts-rs 会打印未解析 `serde(transparent)` 属性的提示。原始类型一致性测试、生成合约测试和真实 IPC 均通过；未通过隐藏警告绕过校验。Vite 的 >500 kB chunk 提示和 Tauri `STATIC_VCRUNTIME` 弃用提示仍在；没有为了消除提示改动构建策略。

### UI 基线

比较的是实际 native WebView 和实际 Rust 后端，而不是 mock 页面。实施前记录 15 张基线，最终应用使用内嵌 production 资源再次执行相同场景：800×600 的英/中/日主页及全部 10 个编码器面板，1440×900 的对话框和主页。

- 4 张主页逐像素完全一致。
- 10 张普通对话框各有 10 个像素不同，宽对话框有 19 个；每通道最大仅 1/255，无更大的差异。未发现这些场景中的布局、字号、图标或文案变化。
- 页面未记录 JavaScript error；语言测试后恢复原来的 localStorage 值。
- 鼠标打开对话框、切换编码器、取消和显示输出选项菜单已执行。Tab/Shift+Tab 的控件焦点观察已执行，但完整原生键盘退出验收没有完成：本环境原生输入返回 `coordinate input geometry is unavailable`，CDP 的 Esc/目标生命周期测试也未形成可靠门禁，不把它标记为通过。
- 截图、逐像素结果与烟测夹具保存在本聊天的 `neo-rimage-validation` 本地验证目录，未将机器特定截图或测试输出加入源码仓库。

构建出的安装包是 `src-tauri/target/debug/bundle/nsis/neo-rimage_0.1.0_x64-setup.exe`。仅构建并运行了其应用二进制，没有安装到用户系统；安装/卸载、release 优化构建、签名、跨平台、大图峰值内存、UNC 实际网络盘及完整原生拖放/键盘回归均不在已通过范围内。

## 刻意保留的后续工作

- 按用户要求，编译工具链迁移暂缓；其中保留的 Vitest 中危公告也要在下一工具链批次处理。
- Table v9、字体/图标升级，以及原生材质辅助库的破坏性迁移需要独立视觉和交互验收。
- 大批量 item 索引、投影缓存、调度计数、全局内存预算和固定线程池先建立真实任务规模基准，不在本次引入未经测量的额外可变状态。rimage 的资源限制单元测试通过，不代表应用已经接入全局内存调度预算。
- 继续补充可靠的原生键盘/拖放自动化或手工验收，并建立 CI；本次不声称所有 UI 交互已经完整覆盖。

## 已验证的提交

- `8493dc5 refactor(ipc): generate frontend contracts from Rust`
- `3b919e9 refactor(ipc): streamline revision notifications and verify runtime sync`
- `ca0a973 refactor(inputs): share backend discovery with file previews`
- `d4c7af2 refactor(ipc): bound job detail pages and request creation summaries`
- `0a1bf92 chore(deps): update compatible runtime and Tauri packages`
- `661d825 chore(deps): resolve transitive security advisories`
- `b2da605 chore(deps): patch development supply chain advisories`
- `6d5d618 chore(engine): upgrade vendored rimage and compatible codecs`

复现应用、合约和图像库验证的命令从仓库根目录执行：

```powershell
pnpm install --frozen-lockfile
pnpm test
pnpm build
pnpm contracts:check
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
cargo test --locked --manifest-path src-tauri/Cargo.toml -p rimage --lib
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --lib --tests -- -D warnings
pnpm tauri build --debug --bundles nsis
pnpm audit --prod
pnpm audit
```

## 2026-10-03：前端复杂度清理

本节记录后续清理；前文的升级基线、提交列表和验证结果保持为历史记录。本轮不升级编译工具链，不修改后端、生成合约、vendor 或用户已有的 `.gitignore` 改动，只提交一个 `refactor(frontend)` commit。

### 清理与保留

- 四列任务表直接使用已有 Table 组件渲染，删除未提供实际排序、筛选或拖拽缩放功能的泛型表格适配层，并移除 `@tanstack/react-table` 及其专属传递依赖 `@tanstack/table-core`。本项目不再需要后续 Table v9 迁移；真正引入复杂表格交互时再评估。
- 应用没有 ThemeProvider，通知本来就回退到 system 主题；改用 Sonner 自带的 `theme="system"`，仍允许 Toaster props 覆盖，移除 `next-themes`。
- 删除没有消费者的 `activeSection`，以及请求转换完全忽略的 resize 草稿 `mode`、`percent`、`factor`；当前 UI 仍提交精确宽高。后端 ResizeMode 的所有已实现模式和相应测试保持不变。
- 保留 `isDirty` 和草稿变更入口：产品文档已明确规划修改后取消提示，维护成本很低；本轮不新增提示或改变关闭行为。metadata/report 草稿也保留。
- 保留能力描述、IPC v1 命令/事件与调度契约、Clock、Engine、Executor、IpcTransport 等扩展或测试替换边界，不以暂时调用少为由缩减协议。
- 保留标准 Radix/shadcn UI 原语，包括尚未使用的便捷导出。这些是可复用的基础组件而非业务框架；没有新增依赖，当前 production 构建会移除未使用导出，不值得为减少源码行数裁剪可访问性和组合接口。

### 本轮验证

| 验证 | 结果 |
| --- | --- |
| pnpm install --frozen-lockfile --ignore-scripts --offline | 通过；锁文件仅删除上述三个包，没有额外升级 |
| pnpm test | 8 文件、57 项通过，含新增任务表 13 项回归测试 |
| pnpm build | TypeScript 检查与 production 构建通过 |
| pnpm contracts:check | 通过，未修改生成合约 |
| cargo test --locked --manifest-path src-tauri/Cargo.toml --lib | 71 项通过，含 Rust/TS 合约一致性检查 |
| 重构前后 DOM 对照 | 三种语言键、空状态、同步/错误状态、全部任务状态、进度边界及 Toaster 默认/覆盖主题共 23 组静态渲染输出完全一致 |
| production CSS 对照 | 逐字节一致，SHA-256 为 `b8724030091af540fe67618d155f4a9277cb8f62d0889480815e4129b489ecd6` |
| git diff --check | 通过 |

相同工具链下，production JS 从 622.58 kB 降至 571.84 kB，gzip 从 193.24 kB 降至 178.69 kB；这是包体实测，不代表图像转换性能提升。表头固定、列宽、滚动容器、tooltip、进度/错误文案保持不变。独立 DOM 对照脚本和基线保存在本聊天的本地验证目录，不加入源码仓库。

本轮没有重新执行原生 WebView 截图、交互烟测或安装包构建，DOM/CSS 对照不替代这些检查。既有 Vite 大 chunk 提示与 ts-rs `serde(transparent)` 提示仍在；未通过调整工具链或隐藏提示绕过验证。
