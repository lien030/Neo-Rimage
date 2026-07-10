---
title: 后端质量、回归与升级计划
aliases:
  - Backend Quality Plan
  - 后端测试计划
tags:
  - neo-rimage
  - backend
  - testing
  - quality
  - dependency-management
status: planned
area: backend
parent: "[[00-backend-overview]]"
related:
  - "[[01-local-engine-module]]"
  - "[[02-job-manager-concurrency]]"
  - "[[03-tauri-command-event-adapter]]"
  - "[[04-filesystem-output-metadata]]"
updated: 2026-07-10
---

# 后端质量、回归与升级计划

> [!summary] 目标
> 为本地 Engine、JobManager、Tauri adapter 和文件输出建立可重复的自动化验证与升级门禁。测试重点是行为、文件安全、状态一致性和可追溯性，而不是只证明项目能够编译。

## 1. 质量原则

- Local Engine 必须脱离 Tauri 可测。
- 并发与状态机优先使用 fake Engine 做确定性测试，真实 codec 用于端到端验证。
- 用户原文件安全高于吞吐；任何失败/取消测试都要检查残留和回滚。
- event 不是状态源；协议测试必须覆盖 snapshot 恢复。
- lossy codec 不使用完整输出字节相等作为唯一断言。
- rimage 固定 revision、features、来源记录和许可证是发布质量的一部分。
- 手工点击只做体验验证，不替代自动化门禁。
- 不在测试或产品运行路径依赖 `rimage.exe`/sidecar；所有核心测试直接调用 library 与本地 Engine。

## 2. 测试分层

| 层级 | 覆盖目标 | 主要依赖 | 失败定位 |
| --- | --- | --- | --- |
| Domain unit | 配置校验、状态转换、错误映射、路径规则 | 无真实 codec/Tauri | 最精确 |
| Component | Decoder/encoder adapter、output transaction、metadata | rimage library + fixture | 模块级 |
| Manager integration | 排队、并发、取消、重试、revision | fake Engine/可控时钟 | 调度级 |
| Engine E2E | 单图片完整 pipeline | 真实 codec + 临时目录 | 处理链 |
| Tauri contract | DTO、commands、events、capabilities | 测试 app/adapter | IPC 边界 |
| Packaging smoke | debug/release bundle 启动和核心任务 | 实际 Tauri bundle | 发布环境 |

## 3. Fixture 基线

### 3.1 图片 fixture

至少准备：

- 每个支持 decoder/encoder 的最小有效图片。
- JPEG orientation 各典型方向。
- 带 ICC、EXIF、alpha、不同 bit depth/colorspace 的图片。
- 透明 PNG、WebP/AVIF、TIFF 与多帧/动画样本。
- 极宽、极高、大像素数和小文件头宣称大尺寸的资源压力样本。
- 截断、错误 magic、扩展名与内容不一致、unsupported format。
- resize/quantize 可观察结果的固定图片。

### 3.2 文件系统 fixture

- Unicode、空格、长路径、大小写差异、UNC 可用场景。
- 重复路径、相对路径、`.`/`..`、多个 scan root。
- symlink/junction、循环、无权限目录、扫描中删除的文件。
- 已存在输出、同 Job 冲突、跨 Job 同目标、只读目标和磁盘写入失败。
- 临时文件、backup、commit 中断与启动清理。

### 3.3 断言策略

- Lossless：解码后的尺寸、colorspace、像素内容、alpha 和 metadata 语义一致。
- Lossy：输出可解码、尺寸/方向/颜色属性正确，使用容差型图像质量断言和合理大小区间。
- Metadata：断言“实际保留/剥离结果”，不只看 request。
- 输出：断言最终文件、原文件、backup、temp 和报告的完整状态。
- 性能：使用独立 benchmark，不把易波动耗时写进普通单元测试。

## 4. 模块测试要求

### 4.1 Local Engine

对应 [[01-local-engine-module]]：

- 每个 stage 的成功、失败、warning 和取消检查点。
- decoder fallback、格式冲突和禁用 feature。
- operation 顺序及隐式 colorspace/depth 转换。
- encoder option 边界、unsupported option 和 capability 一致性。
- AutoOrient、EXIF/ICC preserve/strip 与多帧策略。
- reporter 不阻塞，取消在 commit 前后拥有正确终态。
- panic/依赖错误转换为结构化 Internal error。

### 4.2 JobManager

对应 [[02-job-manager-concurrency]]：

- 所有合法和非法 Job/Task 状态转换。
- desired/effective concurrency、slot busy/retiring 和上限。
- 多 Job 公平性、暂停/恢复、排队取消、运行中 CancelRequested。
- retry attempt、不可重试错误和输出重新 preflight。
- revision 单调性、progress 合并和 terminal 不丢失。
- fake Engine 卡住、panic、返回晚到 event、shutdown 超时。
- 大任务量下 active 数、队列内存和 snapshot 成本受控。

测试不得依赖真实 sleep 来碰运气，应使用 barrier、可控 fake 和有界超时。

### 4.3 Tauri Adapter

对应 [[03-tauri-command-event-adapter]]：

- DTO golden serialization、schema version、字符串 enum 和旧数字兼容映射。
- validation field errors、correlation ID、错误 code。
- command 快速 accepted，不等待 Engine 完成。
- state-delta revision、event emit 失败、乱序/gap 后 snapshot 校正。
- webview reload/subscription 重建。
- capabilities 与实际 Cargo features 一致。
- capability 权限中不存在未使用的 shell/sidecar/宽泛 fs 权限。

### 4.4 Filesystem / Output / Metadata

对应 [[04-filesystem-output-metadata]]：

- 扫描过滤、稳定排序、去重、truncated 和 entry error。
- output containment、suffix 清洗、扩展名、collision 和 reservation。
- encode、metadata、cancel、backup、rename 每一故障点的回滚。
- Windows Replace/文件占用/长路径行为。
- temp 清理只删除本应用可确认归属文件。
- report schema、聚合统计和原子写入。

## 5. 并发与故障注入

必须能注入以下故障，而不是依赖真实磁盘偶发出现：

- 打开/读取/写入/flush/rename/remove/metadata 失败。
- Engine 在指定 stage 阻塞、失败或 panic。
- progress 高频、重复、晚到或 reporter consumer 断开。
- cancel 发生在 decode 前、operation 间、encode 中、commit 前和 commit 后。
- 调整 worker count 时所有 slot busy。
- shutdown 时 task 不响应取消。

并发测试共同不变量：

- active Task Item 不超过 effective concurrency。
- 同一 Task 不会被两个 slot 同时运行。
- terminal state 只写一次且不倒退。
- 同一目标不会并发 commit。
- manager 锁不覆盖真实 Engine/I/O 执行。
- 应用代码未初始化 global Rayon pool。

## 6. 构建与发布门禁

### 6.1 每次后端改动

至少执行：

- Rust 格式检查。
- Clippy 覆盖全部后端 targets，并将本项目新增 warning 作为失败。
- 后端单元/组件/manager 测试。
- 前端 production build，验证 DTO 类型消费未破坏。
- Tauri debug build。

### 6.2 Engine、依赖或输出逻辑改动

额外执行：

- 全 codec/operation fixture 矩阵。
- metadata/ICC/EXIF 与 output fault-injection。
- concurrency/cancel/shutdown 压力测试。
- 依赖许可证和安全公告检查。
- Windows bundle smoke test；声明支持其他平台时补对应 runner。

### 6.3 发布候选

- release profile 构建和安装包验证。
- 新安装、覆盖安装、卸载后的配置/临时文件检查。
- 实际 GUI 创建 Job、GO/STOP、worker count、取消、错误展示和报告导出。
- 断网环境启动，证明不依赖外部 executable 或运行时下载。
- 进程目录中不存在需要随包分发的 `rimage.exe`。

## 7. 性能与资源基线

基线维度：

- 单文件延迟：小图、中图、大图。
- 批量吞吐：1、默认、最大 logical worker slots。
- 峰值内存：大图和多任务并发。
- 取消响应：各 stage 到安全取消完成时间。
- UI/IPC 压力：高频 progress 合并前后事件量。
- 启动与关闭：JobManager 初始化、graceful shutdown。

规则：

- 首个稳定实现记录机器、构建 profile、rimage revision 和 fixture 作为基线。
- 性能优化不得降低输出安全或状态正确性。
- codec 内部线程行为变化要重新评估 effective concurrency。
- 普通 CI 只做宽松回归检测；稳定机器运行正式 benchmark，避免环境噪声误判。

## 8. rimage 升级流程

每次升级必须作为独立工作包：

1. 记录旧/新 crate version、Git revision、Cargo features 和 zune 版本。
2. 阅读 rimage changelog 和 public codec/operation API 差异。
3. 对比曾提取的 CLI pipeline/main 来源片段，检查 decoder、operation order、metadata、output 语义变化。
4. 更新 provenance、第三方声明和许可证文件。
5. 只升级目标依赖，审查 lockfile 中 native codec/线程相关变化。
6. 跑完整 fixture、fault-injection、并发和性能基线。
7. 更新 capability 和已知限制后再合并。

避坑：

- 不用浮动 branch/path 的当前状态作为发布依赖。
- 不因编译通过就认为行为兼容。
- 不开启 `build-binary` 来补齐缺失功能。
- 不把上游新增 CLI option 自动暴露给前端；先进入 capability/产品评审。
- 不覆盖来源 commit 记录，保留每次本地差异演进。

## 9. 许可证与供应链门禁

- rimage dependency 和提取的 orchestration 代码分别登记。
- 记录选择的 `MIT OR Apache-2.0` 合规路径并保留必要版权文本。
- 每个带来源标注的本地文件可追溯到上游 commit 和参考路径。
- 新依赖检查许可证兼容、安全公告、维护状态和 native 构建来源。
- release bundle 的第三方 notices 与实际 lockfile/feature 集一致。
- 禁止提交来源不明的图片 fixture；测试资源也要记录许可或自生成方式。

## 10. 可观察性与问题诊断

- 日志使用 Job ID、Task ID、attempt、stage 和 correlation ID。
- 默认日志不输出图片内容、embedded metadata blob 或无必要的完整用户路径。
- error envelope 给用户安全信息，完整 error chain 仅进入受控日志。
- 记录 rimage revision/features、effective concurrency 和 capability 摘要，便于复现。
- progress 日志采样，避免高频事件淹没真正错误。
- debug diagnostics 可导出时要明确用户可审查内容，不自动上传。

## 11. 已知限制必须公开

V1 至少明确：

- JobManager 仅进程内持久，应用重启不恢复运行中的 Job。
- native codec 采用协作取消，不能承诺即时中断。
- 无可信内部进度的 codec 只显示 stage activity，不伪造百分比。
- 不支持的动画/metadata/encoder option 明确拒绝或 warning，不静默处理。
- worker 是 logical slot，不代表系统只有相同数量线程。
- local Engine 与固定 rimage revision 对齐，升级需要专门回归。

## 12. 阶段任务与交付物

### Q1：测试基础设施

任务：临时目录、fixture catalog、fake Engine、可控时钟、fault-injection 和 golden DTO。

交付物：共享测试工具与 fixture 许可证清单。

验收：各模块不重复造不可兼容的测试替身。

### Q2：模块门禁

任务：补齐 Engine、JobManager、adapter、output 的核心矩阵。

交付物：自动测试、覆盖清单和未覆盖风险。

验收：所有 P0 文件安全/状态一致性路径自动化。

### Q3：集成与打包

任务：Tauri contract、Windows bundle smoke、关闭/权限/断网验证。

交付物：发布候选检查报告。

验收：安装包无需 rimage executable，可完成真实图片任务。

### Q4：升级与性能基线

任务：建立 rimage upgrade checklist、provenance、dependency audit 和 benchmark。

交付物：可重复升级手册与首个基线报告。

验收：升级 Agent 无需猜测来源文件、feature 或验收范围。

## 13. 子 Agent 工作包

### BE-05A：Fixture / Test Harness

- 输入：四个后端模块契约。
- 输出：fixture catalog、fake Engine、fault-injection、golden helpers。
- 验收：资源来源合规，测试可并行且不污染用户目录。

### BE-05B：State / Concurrency QA

- 输入：JobManager 状态机。
- 输出：确定性调度、取消、revision、shutdown 压力测试。
- 验收：无 sleep 猜测，无永远 Running task。

### BE-05C：Engine / Output QA

- 输入：codec capability、pipeline、output transaction。
- 输出：图片/metadata/path/rollback 矩阵。
- 验收：每个支持 codec 至少一条 E2E，所有 commit 故障点可回滚。

### BE-05D：Release / Upgrade QA

- 输入：Cargo dependency、Tauri bundle、provenance。
- 输出：CI 门禁、bundle smoke、dependency/license/performance 基线。
- 验收：发布不携带 rimage.exe，升级有完整审计记录。

## 14. 风险与避坑

> [!danger] 只测 happy path
> 图片工具最严重的问题通常出现在覆盖原文件、取消、磁盘错误和并发竞态。没有 fault-injection 与回滚断言，就不能宣称后端可靠。

- **比较压缩字节**：codec 版本/线程可能改变字节；按 lossless/lossy 选择正确断言。
- **真实 sleep 并发测试**：会产生偶发失败；使用可控同步点。
- **只检查最终文件存在**：还要验证原文件、backup、temp、metadata 和 Job terminal state。
- **CI 只跑默认 features**：实际 bundle feature 集必须单独验证。
- **升级只看 Cargo 编译**：必须对比来源 pipeline 行为和 fixture。
- **手工测试代替协议测试**：DTO/event revision 需要 golden 和自动恢复测试。
- **忽略测试资源许可**：fixture 也属于发布仓库资产。
- **性能驱动无界并行**：任何优化都要同时测峰值内存、取消响应和 UI event 压力。

## 15. 发布验收清单

- [ ] fmt、clippy、unit/component/integration 测试通过。
- [ ] 每个启用 codec/operation 有 capability 与 fixture 对应。
- [ ] cancel、panic、I/O、metadata、commit 故障可回滚。
- [ ] JobManager 并发/状态/revision 不变量通过压力测试。
- [ ] Tauri DTO/event schema 与 snapshot 恢复通过 contract test。
- [ ] Windows debug/release bundle 可完成真实任务。
- [ ] bundle 和运行路径不包含/调用 `rimage.exe`。
- [ ] rimage/zune revision/features 与 lockfile 已记录。
- [ ] provenance、第三方 notices、fixture 许可完整。
- [ ] 性能/内存基线无未解释的严重回退。
- [ ] 已知限制已同步到用户文档和发布说明。

