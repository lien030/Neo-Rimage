---
title: JobManager 与并发控制实施需求
aliases:
  - Job Manager and Concurrency
  - 后端任务调度
tags:
  - neo-rimage
  - backend
  - job-manager
  - concurrency
  - cancellation
status: planned
area: backend
parent: "[[00-backend-overview]]"
related:
  - "[[01-local-engine-module]]"
  - "[[03-tauri-command-event-adapter]]"
  - "[[05-backend-quality-plan]]"
updated: 2026-07-10
---

# JobManager 与并发控制实施需求

> [!summary] 目标
> 建立进程内唯一的后端状态源，管理 Job、Task Item、逻辑 worker slot、有限并发、暂停、取消、重试、快照与退出。JobManager 只调度 [[01-local-engine-module]]，不理解具体 codec 参数细节。

## 1. 当前模型为什么必须替换

现有后端使用全局 `Lazy<Arc<Mutex<...>>>`、crossbeam channel 和“一 worker 一永久线程”模型，存在以下问题：

- 前端和后端都维护 worker/task 状态，刷新或异常后会漂移。
- worker status 和当前 task 没有可靠地随处理过程更新。
- 移除 worker 只发送 stop，无法表达处理中的协作取消和线程收尾。
- channel 中的 task 只包含文件基础信息，无法绑定一组稳定的处理配置。
- codec 内部并行与外部 worker 线程可能叠加，CPU/内存不可控。
- 静态全局状态让测试隔离、重建 manager 和优雅退出变困难。

新 JobManager 必须替换旧模型，而不是在其外面继续包一层。

## 2. 核心领域对象

### 2.1 Job / Batch

一次 Create Task 提交形成一个 Job。它包含：

- 稳定 Job ID、创建时间和配置版本。
- 一组 Task Item。
- 共享 Engine 配置和输出策略。
- Job 当前状态与汇总计数。
- 期望并发额度、暂停/取消标记。
- 配置快照；提交后不因前端表单变化而改变。

### 2.2 Task Item

一个输入文件对应一个 Task Item。它包含：

- 稳定 Task ID 和所属 Job ID。
- 输入/输出规划、当前 attempt。
- 当前状态、阶段、最近可信进度。
- 开始/结束时间、当前 logical worker slot。
- 成功结果、warning 或结构化错误。
- 取消请求时间与最终取消状态。

### 2.3 Logical Worker Slot

worker 是 UI 可视化的逻辑并发槽，不等于 OS 线程：

- slot 数量表示 JobManager 允许同时运行的外层 Task Item 数量。
- slot ID 由后端生成，在 manager 生命周期内稳定。
- slot 只在有 task 时显示 Busy；空闲时不持有图片或永久阻塞线程。
- 降低并发时，Busy slot 标记为 retiring，完成当前安全任务后退出，不杀线程。
- UI 的 “Add/Remove Worker” 最终语义应收敛为调整 desired concurrency。

> [!important] 兼容策略
> 迁移期可保留 `add_worker`/`remove_worker` 命令，但它们只能增减 JobManager 的 desired concurrency。不得再接收由前端生成的 worker ID，也不得创建一 worker 一永久线程。

### 2.4 Snapshot

Snapshot 是前端读取状态的唯一方式：

- 由 JobManager 在同一一致性边界内生成。
- 带单调递增 revision。
- 仅包含可序列化、面向 UI 的只读投影。
- 不暴露内部锁、channel、token、JoinHandle 或 Engine 实例。
- event 可以只携带 delta，但前端随时能请求完整 snapshot 校正。

## 3. 状态机

### 3.1 Job 状态

| 状态 | 含义 | 可进入 | 可执行动作 |
| --- | --- | --- | --- |
| Queued | 已创建，等待调度 | 新建 | start、cancel |
| Running | 正在派发/执行任务 | Queued、Paused | pause、cancel |
| Paused | 不再启动新 task，已运行 task 默认继续至安全终点 | Running | resume、cancel |
| CancelRequested | 已取消排队项，正在等待运行项响应 | Queued、Running、Paused | 查询 |
| Completed | 所有 task 成功 | Running | 查询、清理历史 |
| CompletedWithErrors | 所有 task 已终结，至少一个失败 | Running、CancelRequested | retry failed、查询 |
| Canceled | 所有未完成 task 均已终结为 canceled/已完成 | CancelRequested | retry canceled、清理历史 |
| Failed | Job 级错误导致无法调度，例如配置/输出规划整体失败 | Queued、Running | 查询、按修正后新建 Job |

Job 不提供“暂停正在执行的 native codec”语义。Pause 只停止派发新 Task Item；如产品需要“暂停后等待当前全部完成”，由 UI 展示 draining 状态。

### 3.2 Task Item 状态

| 状态 | 含义 | 合法终点 |
| --- | --- | --- |
| Queued | 已验证并等待 slot | 否 |
| Running | 已交给 Engine | 否 |
| CancelRequested | Engine 正在不可中断区间，等待安全检查点 | 否 |
| Succeeded | 输出已提交，结果可用 | 是 |
| Failed | 结构化失败，输出未提交或已安全回滚 | 是 |
| Canceled | 未开始即取消，或 Engine 在安全点完成取消 | 是 |

禁止状态倒退，例如 Succeeded 不得改为 Canceled；重试通过增加 attempt 并重新进入 Queued，而不是篡改历史结果。

### 3.3 状态转换规则

- 每个状态转换由 JobManager 单点执行，并验证来源状态。
- 每次可观察状态改变都递增 revision。
- terminal event 必须在结果/错误写入状态后发出。
- Engine 只返回事件和结果，不直接修改 JobManager 内部实体。
- 任意 panic/worker 丢失必须被转成 Internal failure，不允许 task 永远停在 Running。
- Job 汇总状态由 Task Item 终态派生，不由前端指定。

## 4. 并发模型

### 4.1 单一外层预算

JobManager 维护统一的外层并发预算：

- `desired concurrency` 是用户/配置希望的逻辑 slot 数。
- `effective concurrency` 受 CPU、内存压力、codec 特性和关闭状态约束。
- 默认从安全的单并发开始；UI 明确调整后再增加。
- 上限由后端能力返回，前端不能提交无限 worker。
- JobManager 只允许有限数量的 Task Item 同时进入 Engine。

### 4.2 禁止 global Rayon pool

应用代码不得调用 `rayon::ThreadPoolBuilder::build_global()`：

- GUI 是长生命周期进程，需要可重复创建/销毁测试实例和动态调整并发。
- global pool 一旦初始化无法按 Job 安全重配。
- 多个模块争用 global pool 会使 worker UI 与真实资源使用不一致。

允许的策略：

- JobManager 使用自有的有界执行器或异步 blocking pool 接入点。
- 确有 Rayon 并行需求时使用私有、局部 ThreadPool，并将预算纳入 effective concurrency。
- 对内部会自行并行的 codec 记录“资源权重”，必要时降低外层并发或串行执行重型 codec。

> [!warning] 依赖内部线程
> “本项目不 build global pool”不等于依赖一定不使用线程。每个 rimage feature 和 codec 都要审计内部并行行为；不能仅按逻辑 slot 数宣称总线程数完全相等。

### 4.3 避免嵌套并行

初期策略以外层 Task Item 并发为主：

- 同一 Task Item 的 orchestration 默认顺序执行。
- codec 自身需要线程时，将其成本计入调度权重。
- 不在 JobManager、Engine 和 codec 三层同时无界并行。
- 性能优化必须以 [[05-backend-quality-plan]] 的吞吐、峰值内存和交互响应基线为依据。

### 4.4 公平性与排序

- 同一 Job 内默认保持输入顺序派发，但完成顺序不保证。
- 多 Job 并存时使用简单、可解释的公平策略，避免大 Job 永久阻塞后续 Job。
- 用户优先级功能未明确前，不引入复杂优先级抢占。
- 已 Running 的任务不因新 Job 到来被强制抢占。

## 5. 队列与背压

### 5.1 队列内容

队列只保存轻量 Task Item/执行计划引用，不预先 decode 图片，也不缓存所有输出字节。

### 5.2 大批量输入

- 输入发现可以产生大量路径，但 JobManager 不应为每个文件创建永久线程或大对象。
- 快照应支持摘要与分页/按 Job 查询的演进空间，避免一次序列化数十万完整错误链。
- Job 级汇总计数与 Task Item 明细分离。
- 事件队列必须有界；允许合并中间 progress，绝不能丢失 terminal state。

### 5.3 事件节流

- StageStarted、terminal state、warning/error 立即提交。
- 连续 StageProgress 可按 Task Item 合并，只保留最新值。
- UI 推送频率设置统一上限，避免 IPC 洪泛。
- Snapshot revision 始终是最终校正机制，不能把 event bus 当数据库。

## 6. 暂停、取消、重试

### 6.1 Pause / Resume

- Pause：停止从 Queued 取新任务。
- 已 Running 的任务继续处理，除非用户另行 Cancel。
- Resume：恢复派发，不重建 Job 或改变配置。
- Paused 状态下可以调整 desired concurrency；恢复后生效。

### 6.2 Cancel

- Queued task 立即转为 Canceled，不进入 Engine。
- Running task 转为 CancelRequested，并触发 [[01-local-engine-module]] 的 cancellation probe。
- codec 不可中断区间允许延迟完成取消，UI 必须展示“正在取消”。
- 已完成 output commit 的 task 保持 Succeeded。
- 取消 Job 不等于删除 Job 记录；用户仍可查看结果和失败原因。

### 6.3 Retry

- 默认只重试 Failed；Canceled 是否重试由命令显式选择。
- retry 复用同一配置快照，但重新执行 preflight，防止路径/文件环境已经变化。
- attempt 递增并保留最近失败摘要，避免无限重试掩盖系统性错误。
- Validation/Unsupported 等非瞬态错误默认标记不可重试。
- retry 前重新执行输出冲突检查，不能覆盖上次成功任务的产物。

## 7. 锁、执行和故障隔离原则

- JobManager 状态锁只保护快速读写；decode/encode、文件 I/O 和 Tauri emit 必须在锁外执行。
- 任何外部 callback 不得在持有 manager 锁时调用，避免重入死锁。
- 每个 Task Item 的 Engine 执行有独立取消 token 和 panic 边界。
- 单文件失败不应中止整个 Job，除非属于 Job 级不变量或用户选择 fail-fast。
- 共享配置提交后只读，减少复制和竞态。
- channel/executor 关闭必须显式反映为 backend shutting down，不能无限等待。
- poison/内部不变量错误要记录并转为结构化 Internal error，不使用无上下文 `unwrap()`。

## 8. 后端唯一状态源

### 8.1 前端允许保存

- Create Task 尚未提交的表单草稿。
- 最近收到的 snapshot/delta，用于渲染。
- 展开行、tab、filter 等纯视图状态。

### 8.2 前端不得作为权威保存

- worker 数量和 worker ID。
- task 状态、当前阶段、百分比、错误终态。
- 队列长度、Job 是否暂停/取消。
- output 是否已经生成、压缩结果。

### 8.3 恢复范围

V1 JobManager 为进程内状态源：

- webview 刷新/重载后可以通过 snapshot 恢复当前任务视图。
- 应用进程完全退出后不保证继续运行或恢复 Job 历史。
- 退出期间通过 output transaction 清理临时状态；下次启动执行残留临时文件检查。
- 如未来需要跨重启恢复，应单独设计持久化日志/数据库，不能直接序列化内部锁和线程对象。

## 9. 应用退出与生命周期

退出流程：

1. JobManager 进入 ShuttingDown，拒绝新 Job 和 concurrency 调整。
2. 取消所有 Queued task。
3. 向 Running task 发取消请求。
4. 在统一的有界 grace period 内等待安全点。
5. 清理未提交临时输出并关闭执行器/event bridge。
6. 返回可供 Tauri 决定关闭的结果。

不能保证 native codec 立即中断，因此：

- grace period 超时必须记录仍在执行的 Task ID 和阶段。
- 不在库层调用进程退出。
- 强制关闭后的临时文件由下次启动清理策略兜底。

## 10. 与 Tauri Adapter 的契约

[[03-tauri-command-event-adapter]] 只能通过 JobManager public API 操作状态：

- Create/Validate/Start Job。
- Pause/Resume/Cancel/Retry。
- Set desired concurrency。
- Get summary/full snapshot。
- Subscribe to state deltas。
- Begin graceful shutdown。

Tauri adapter 不可：

- 直接发送 Task Item 到内部 channel。
- 读取或修改内部 mutex 数据结构。
- 直接调用 Engine 绕过状态机。
- 自行推导 Job 汇总终态。

## 11. 阶段任务与交付物

### J1：状态机与 fake Engine

任务：建立 Job/Task/slot/snapshot/revision 模型，用可控 fake Engine 验证状态转换。

交付物：状态图、manager API、确定性单元测试。

验收：每个终态和非法转换都有测试；无 Tauri 依赖。

### J2：有界调度

任务：接入自有执行器、desired/effective concurrency、slot retiring、公平队列。

交付物：并发调度实现、压力测试和资源上限说明。

验收：反复调整并发不会创建泄漏线程，不调用 global pool。

### J3：进度与事件快照

任务：消费 Engine reporter，更新状态、递增 revision、合并高频 progress。

交付物：snapshot/delta 模型、乱序/丢 event 恢复测试。

验收：前端可只靠完整 snapshot 恢复权威状态。

### J4：取消、重试与退出

任务：实现排队取消、运行中协作取消、attempt、graceful shutdown。

交付物：故障/取消/退出测试矩阵。

验收：没有永远 Running 的任务；失败和取消均不遗留被标记成功的输出。

### J5：替换旧 worker/task 全局状态

任务：兼容旧命令、切换消费者、删除 Lazy globals/crossbeam worker loop。

交付物：迁移说明和删除清单。

验收：后端只有一个 JobManager 实例作为 tauri managed state。

## 12. 子 Agent 工作包

### BE-02A：状态机与 Snapshot

- 输入：本文状态语义、Engine fake 接口。
- 输出：Job/Task/slot 状态机、revision、snapshot。
- 验收：非法转换被拒绝，terminal 不可倒退。

### BE-02B：Scheduler 与预算

- 输入：状态机、CPU/capability 信息。
- 输出：有界执行器、公平调度、desired/effective concurrency。
- 验收：压力测试下 active 数不超过预算，无 global Rayon 初始化。

### BE-02C：取消与退出

- 输入：Engine cancellation contract、Tauri lifecycle 需求。
- 输出：cancel/pause/resume/retry/shutdown 行为。
- 验收：不可中断 codec 用 CancelRequested 表达，不假装即时成功。

### BE-02D：旧模型迁移

- 输入：当前 add/remove worker、task commands。
- 输出：兼容 adapter 和旧全局状态删除方案。
- 验收：不存在第二队列、第二 worker 列表或前端生成 worker ID。

## 13. 风险与避坑

> [!danger] 锁内执行重任务
> 如果 manager 在持锁时调用 Engine、文件系统或 Tauri emitter，批处理很快会出现 UI 卡死、取消无响应或死锁。

- **把 slot 当线程**：slot 是资源令牌和展示投影，不能持有永久阻塞线程。
- **Pause 等同强暂停**：native codec 无安全冻结点；Pause 只停派发。
- **Cancel 覆盖成功**：commit 之后的任务不能再改成 canceled。
- **event 即状态**：event 会丢失或乱序，必须用 revision + snapshot 校正。
- **无界队列/事件**：大目录可能产生数万条任务，必须限制内存和 IPC 频率。
- **嵌套并行**：外层 slot 与 codec 内部线程要统一预算，不能按 CPU 核数各自放大。
- **重试覆盖文件**：每次 attempt 都重新 preflight 输出目标。
- **退出直接杀进程**：优先有界协作退出，并让 output transaction 能清理残留。
- **状态双写**：兼容旧命令也必须落到 JobManager，不能保留旧 static list。

## 14. 验收清单

- [ ] JobManager 是后端唯一任务/worker 状态源。
- [ ] Job、Task Item、Logical Worker Slot 概念分离。
- [ ] 状态转换有单点校验和单调 revision。
- [ ] 并发有界且可动态调整，无永久 worker 线程。
- [ ] 应用代码不初始化 global Rayon pool。
- [ ] Pause、CancelRequested、Canceled 语义与 codec 能力一致。
- [ ] progress 可合并，terminal event 不丢失。
- [ ] webview 重载可通过 snapshot 恢复。
- [ ] V1 跨进程恢复范围已明确，不误承诺后台续跑。
- [ ] 优雅退出、panic、channel/executor 故障均有测试。
- [ ] 旧全局队列和 worker handles 在迁移完成后删除。

