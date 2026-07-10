---
title: 文件系统、输出与 Metadata 实施需求
aliases:
  - Filesystem Output Metadata
  - 文件与输出服务
tags:
  - neo-rimage
  - backend
  - filesystem
  - output
  - metadata
status: planned
area: backend
parent: "[[00-backend-overview]]"
related:
  - "[[01-local-engine-module]]"
  - "[[02-job-manager-concurrency]]"
  - "[[03-tauri-command-event-adapter]]"
  - "[[05-backend-quality-plan]]"
updated: 2026-07-10
---

# 文件系统、输出与 Metadata 实施需求

> [!summary] 目标
> 将输入发现、路径规范化、输出规划、冲突处理、事务式落盘、嵌入 metadata 和外部处理报告收敛到 Rust 后端。前端只提交用户选择并展示预览，不自行递归、拼输出路径或判断文件是否已经成功写入。

## 1. 模块定位

```mermaid
flowchart LR
    Scan["Input Discovery"] --> Plan["Output Planner"]
    Plan --> Engine["Local Engine"]
    Engine --> Temp["Temporary Output"]
    Temp --> Meta["Metadata Finalize"]
    Meta --> Commit["Atomic Commit"]
    Commit --> Result["Task Result / Report Data"]
```

建议拆成四个职责明确的服务：

| 服务 | 责任 | 消费者 |
| --- | --- | --- |
| Input Discovery | 扫描、规范化、过滤、去重、输入 warning | Tauri scan command / Job preflight |
| Output Planner | 计算 input → output、结构保留、suffix、扩展名、冲突 | Job 创建与每次 retry preflight |
| Output Transaction | 临时文件、metadata finalize、提交、回滚、backup | [[01-local-engine-module]] |
| Metadata / Report | EXIF/ICC 策略、输入输出属性、Job 报告聚合数据 | Engine / JobManager |

这些服务不能拥有 Job 状态；JobManager 只保存它们的规划和结果。

## 2. 输入发现

### 2.1 支持输入

Input Discovery 接受文件和目录列表，并返回：

- accepted candidates。
- rejected entries 及稳定原因码。
- scan warnings。
- 规范化后的公共根信息，供保留目录结构使用。

文件支持能力来自 [[01-local-engine-module]] 的 capability，不能在前端和 `scan_dir` 中各维护一份扩展名白名单。

### 2.2 扫描规则

- 目录递归由请求显式指定；不因拖入目录而默认无限递归。
- V1 默认不跟随 symlink/junction，避免循环、越界和重复扫描。
- 常规文件才进入候选；目录、设备、socket 等特殊类型拒绝。
- 单个 entry 读取失败转为 rejected/warning，不 `unwrap` 终止整个扫描。
- 发现顺序应稳定，便于任务列表和 fixture；不要依赖文件系统未定义枚举顺序。
- 扫描设置数量/深度/耗时保护；达到上限时返回 truncated 标记，而不是静默丢文件。
- 扫描不 decode 全部图片；只做轻量路径和 capability 初筛，真正格式验证在 Job/Engine preflight。

### 2.3 格式识别

- 扩展名用于快速筛选和 UI 提示，不是最终格式事实。
- Engine 在处理时使用 decoder/content 检查。
- 扩展名不支持但内容可能支持的文件，可进入 rejected 或“需要深度探测”类别，策略保持稳定。
- 扩展名与内容冲突时由 preflight 返回明确 warning/error。

### 2.4 路径规范化与去重

- 内部始终使用原生路径类型，不通过 UTF-8 字符串往返计算。
- 已存在输入优先 canonicalize，以解析 `.`、`..`、symlink 与 Windows 大小写/前缀差异。
- 去重基于规范化路径；必要时可增加文件 identity 防止 hard link 重复。
- Windows 路径需覆盖盘符、UNC、verbatim prefix、长路径、大小写和保留名。
- 传给前端的 display path 与内部 path identity 分开；lossy display 不得用于回传后重新定位文件。
- 多个目录输入保留各自 scan root，避免计算相对路径时跨 root 逃逸。

## 3. 输出规划

### 3.1 规划输入

Output Planner 需要：

- 规范化输入及其 scan root。
- 输出目录或 same-directory 策略。
- 是否保留相对目录结构。
- suffix/name template。
- encoder 决定的真实扩展名。
- collision policy。
- source backup / existing-output backup 策略。

输出路径不得由前端自行拼接后直接信任。

### 3.2 路径规则

- encoder format 是最终扩展名的唯一来源，避免内容与扩展名不符。
- suffix 只作用于安全的文件 stem；不能注入路径分隔符或 `..`。
- 保留目录结构时，相对路径必须验证仍位于对应 scan root 内。
- 输出目录不存在可以规划，但创建发生在 execution/commit 阶段。
- 输出目标与输入相同时必须被识别为显式 in-place 场景，不能偶然覆盖。
- 一次 Job 的所有 input → output 映射在入队前整体检查，先发现 task-to-task collision。
- retry 和真正 commit 前重新检查，处理外部程序在期间创建/修改文件的竞态。

### 3.3 冲突策略

内部模型明确区分：

- **Fail**：默认安全策略；目标已存在或 Job 内多任务同目标即拒绝。
- **Replace**：只有用户明确允许时使用，通过 Output Transaction 安全替换。
- **Auto Rename**：如产品启用，规划阶段使用确定性名称并在 commit 时再次确认；不允许不同线程竞态选择同一名称。

避坑：不要用“文件存在就加 `(1)`”的无锁循环；并发任务可能同时选择相同名称。

### 3.4 Backup 语义

必须区分两个概念：

- **Source backup**：in-place 处理前备份原输入，对应历史 CLI `backup` 的主要语义。
- **Existing-output backup**：Replace 一个与输入不同的已有输出前，备份该目标。

不要用一个模糊 `backup: bool` 同时代表两者。迁移旧参数时应转换到明确的内部策略。

备份要求：

- 备份路径也参与冲突规划。
- 备份与最终提交的顺序可回滚，不能先移动原文件后因 encode 失败导致源文件消失。
- 备份名称保留原始扩展名，并能追溯对应 Task ID/时间或采用明确兼容命名。
- backup 失败属于 Output error，禁止继续覆盖。

## 4. Output Transaction

### 4.1 为什么必须事务式写入

直接让 encoder 写最终路径会产生：

- encode 失败留下半文件。
- cancel 后 UI 看到貌似存在的失败产物。
- metadata 写回失败时无法安全回滚。
- Windows Replace/rename 行为差异导致用户文件丢失。

### 4.2 标准提交阶段

1. 确认目标与目录仍满足 preflight。
2. 在目标同目录创建唯一临时文件，以提高 rename 原子性。
3. encoder 写入临时文件。
4. 完成必要 flush/close，并验证输出可读取和非空等基本不变量。
5. 在临时输出上完成 metadata/ICC/EXIF finalize。
6. 再次检查取消；尚未 commit 时可删除临时文件并返回 Canceled。
7. 按 collision/backup 策略提交到最终目标。
8. 读取真实输出属性并返回成功结果。

### 4.3 回滚与清理

- Pre-commit 任意失败删除临时文件。
- Commit 中涉及 backup/replace 的多步操作必须有明确回滚顺序。
- 清理失败作为 warning/notice 记录临时路径，不能覆盖主错误。
- 应用启动时扫描由本项目命名规则产生的过期临时文件；只删除可确认归属且超过安全期限的文件。
- 不执行通配符式广泛删除，不清理无法证明由 neo-rimage 创建的文件。

### 4.4 并发与 TOCTOU

- Output Planner 防止 Job 内碰撞，Output Transaction 防止运行期间外部竞态。
- 对同一目标建立后端级 reservation/lock，避免两个 Job 同时提交。
- 文件存在、权限和修改时间检查不能替代 commit 时验证。
- 同一输入被多个 Job 同时 in-place 处理应拒绝或串行化，不能并发覆盖。

## 5. Embedded Metadata 与颜色管理

### 5.1 概念分离

不要把以下三类数据都称为 metadata：

1. **Embedded metadata**：EXIF/XMP 等附着在图片文件中的信息。
2. **Color profile**：ICC 与 colorspace 转换信息。
3. **Processing report metadata**：输入/输出大小、耗时、尺寸、压缩率等 neo-rimage 结果。

三者拥有不同的策略、错误严重度和输出位置。

### 5.2 EXIF / orientation

- AutoOrient 必须在丢弃 orientation tag 前应用，否则输出方向错误。
- `strip` 时不写回可剥离 EXIF，但仍应正确应用方向。
- 保留模式下，仅在目标格式和 encoder 支持时写回。
- writeback 失败默认记录 warning 和实际 `metadata_preserved = false`；如未来提供 strict 模式，再升级为 fatal。
- 不支持的 metadata 类型不得宣称保留成功。

### 5.3 ICC / colorspace

- 保留 ICC 与转换至 sRGB 是不同操作，必须在 Engine pipeline 中明确。
- 如果目标 encoder 不支持 profile，而颜色正确性要求转换，则转换失败应视为致命 Operation error。
- strip 模式也不能简单删除 ICC 而不处理依赖 profile 的像素颜色。
- 输入/输出实际 colorspace 和 profile 处理结果进入 Engine Result。
- rimage 当前 default features 与 CLI `build-binary` 的 ICC 能力不同；Cargo features 必须显式固定并测试。

### 5.4 动画与多帧

- report 记录 frame count、animated 状态和实际输出策略。
- 目标 encoder 不支持多帧时，在 preflight 拒绝或使用明确的首帧策略；禁止静默丢帧。
- EXIF/ICC 对多帧格式的支持差异进入 capability。

## 6. Processing Report

### 6.1 单 Task 结果字段

至少记录：

- Job/Task/attempt ID。
- 输入/输出路径与格式。
- 输入/输出字节、压缩率、节省字节。
- 原始和输出尺寸、像素数、宽高比。
- bit depth、colorspace、alpha、animation/frame count。
- encoder 与后端配置版本摘要。
- metadata/ICC/EXIF 实际结果和 warnings。
- 各主要阶段耗时与总耗时。
- 成功时间；失败时由结构化错误代替输出结果。

### 6.2 Job 聚合

JobManager 聚合：

- 总任务、成功、失败、取消数量。
- 总输入/输出字节和节省空间。
- 总耗时与吞吐统计。
- encoder/operation 配置摘要。
- warning/error code 分布。

聚合必须基于 Task terminal results，不能由 UI 对 event 临时求和。

### 6.3 JSON 报告

- 报告带 schema version、应用版本、rimage revision/features。
- 使用 Output Transaction 原子写入。
- 路径字段采用平台可恢复的序列化策略；若仅提供 display string，要明确不可作为再次执行输入。
- 大 Job 报告可流式/分段生成，避免复制全部图片 metadata 多次。
- 报告写入失败不应改变已成功图片的 Task 终态，但 Job 产生 report failure warning/error。

## 7. 安全与权限

- 后端自行打开文件，前端不需要宽泛 `fs:read-all`/write 权限时应移除。
- 所有输出必须落在用户显式选择或由 same-directory 策略推导的范围。
- 路径 containment 使用规范化组件判断，不使用字符串前缀。
- 默认不 follow symlink/junction，防止输出逃逸和递归循环。
- 临时文件权限不应比最终目标更宽。
- 日志默认避免完整输出用户目录；调试日志可通过 correlation ID 和可控级别定位。
- 错误返回可展示路径，但不把文件内容或 embedded metadata blob 发给前端。
- 磁盘空间不足、只读目录、被占用文件、杀毒软件锁定等 Windows 场景进入测试矩阵。

## 8. 性能与资源策略

- 扫描阶段只读取目录项和必要 metadata，不 decode。
- Engine 一次只持有当前 Task Item 的图片和编码缓冲，不跨任务缓存大图。
- 优先支持 encoder 写入临时文件/流，避免在内存复制完整输出两次。
- report 聚合只保存必要字段；大型二进制 metadata 不进入 JobManager snapshot。
- 文件 hash 只在确有去重/完整性需求时计算，不作为默认扫描成本。
- 路径 reservation 与临时文件表由后端维护并有界清理。

## 9. 阶段任务与交付物

### F1：输入发现与规范化

任务：建立 scan request/result、稳定遍历、过滤、去重、symlink 和上限策略。

交付物：Input Discovery service、Windows/目录错误 fixture。

验收：坏 entry 不使扫描 panic，重复路径稳定去重，truncated 可见。

### F2：输出规划

任务：实现相对结构、suffix、format extension、collision、backup 和 reservation 语义。

交付物：可预览的 input → output plan、冲突诊断。

验收：Job 内冲突在入队前发现，路径不能逃出 output root。

### F3：事务式输出

任务：临时文件、验证、commit、replace/backup、rollback 和启动清理。

交付物：Output Transaction 与 fault-injection 测试。

验收：encode/cancel/metadata/commit 任一点失败都不留下假成功目标。

### F4：EXIF/ICC 与结果

任务：实现 preserve/strip/AutoOrient/profile 策略和实际结果字段。

交付物：方向、颜色、metadata fixture 与 capability 表。

验收：strip 后方向正确；保留失败不会谎报成功。

### F5：Job 报告

任务：定义 versioned report、聚合、原子输出和大批量内存策略。

交付物：报告 schema、golden fixture、兼容说明。

验收：报告数据来自后端 terminal result，压缩汇总计算一致。

## 10. 子 Agent 工作包

### BE-04A：Input Discovery

- 输入：capability、drag/drop 与目录需求。
- 输出：扫描、规范化、过滤、去重、warning。
- 验收：跨 Windows 路径和错误目录项行为稳定。

### BE-04B：Output Planner

- 输入：Create Task output options、encoder extension。
- 输出：安全且可预览的 output plan/collision diagnostics。
- 验收：无字符串路径拼接，无 task-to-task 碰撞漏检。

### BE-04C：Output Transaction

- 输入：output plan、Engine encoded output。
- 输出：临时写入、commit/rollback/backup/reservation。
- 验收：fault injection 下用户原文件与已有输出不丢失。

### BE-04D：Metadata / Report

- 输入：rimage/zune metadata 能力、Task result 需求。
- 输出：EXIF/ICC 策略、实际结果字段、versioned Job report。
- 验收：方向/颜色/strip/preserve 与报告聚合有 fixture。

## 11. 风险与避坑

> [!danger] 直接写最终目标
> 这是最容易造成用户文件损坏的实现捷径。所有 encoder 输出必须先进入同目录临时文件，再经过 metadata finalize 和 commit。

- **扩展名即格式**：扩展名只能初筛，decoder/encoder 才是事实。
- **字符串路径处理**：UNC、非 UTF-8、大小写和 `..` 会击穿简单拼接。
- **symlink 越界**：V1 不 follow，未来启用也要单独设计 containment。
- **backup 含义混乱**：source backup 和 existing-output backup 分开。
- **并发 auto rename**：需要 reservation，不能无锁找下一个可用名称。
- **取消后覆盖**：commit 是不可逆边界，commit 前检查取消，commit 后保持成功。
- **删除 ICC 导致偏色**：strip metadata 不等于无需颜色转换。
- **保留失败却显示成功**：结果记录实际 preserved 状态和 warning。
- **前端重复扫描**：Rust 后端是唯一输入发现和路径规划来源。
- **广泛清理 temp**：只删除能证明属于本应用且过期的临时文件。

## 12. 验收清单

- [ ] 输入扫描稳定、可取消演进、错误 entry 不 panic。
- [ ] 格式能力来自后端 capability，不依赖前端白名单。
- [ ] 原生路径计算不通过 display string 往返。
- [ ] 默认不 follow symlink/junction，containment 有测试。
- [ ] output plan 在 Job 创建前发现内部冲突。
- [ ] Replace、Auto Rename、Source Backup、Existing-output Backup 语义分离。
- [ ] encoder 不直接写最终目标，失败/取消可回滚。
- [ ] 同一输出目标有 reservation，跨 Job 不竞态。
- [ ] AutoOrient、EXIF、ICC、strip 顺序有 fixture。
- [ ] Task result 记录 metadata 实际结果，不只记录请求。
- [ ] JSON report 有 schema/rimage revision，且原子写入。
- [ ] 前端 fs capability 已按实际职责收窄。

