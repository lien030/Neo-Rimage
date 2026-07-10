---
title: 端到端测试与发布门禁
aliases:
  - E2E Test and Release
tags:
  - neo-rimage
  - integration
  - testing
  - release
status: planned
owner: integration
---

# 端到端测试与发布门禁

> [!summary]
> 测试以“真实 Tauri 应用同进程调用 local engine 与 rimage library”为核心，不用 mock sidecar 证明错误的架构。发布门禁覆盖契约、图片结果、状态恢复、取消、平台构建与依赖升级。

## 目标

- 证明 React、IPC、JobManager、local engine 和 rimage library 能按冻结契约协同工作。
- 以少量高价值 fixture 覆盖成功、失败、取消和输出策略，而非只验证页面能点击。
- 建立可重复的发布门禁，防止参数映射、状态机或 rimage 升级产生静默回归。
- 区分每次提交的快速检查与发布前的完整平台验证。

## 边界

### 本文覆盖

- 契约测试、后端集成测试、真实应用 E2E 和安装包冒烟测试。
- 图片 fixture、结果断言、错误和取消场景。
- CI 分层、发布阻断条件、人工验收和 rimage 升级门禁。
- Windows 主平台以及其他声明支持平台的构建责任。

### 本文不覆盖

- codec 算法本身的全面正确性证明。
- 视觉像素级回归平台的具体选型。
- 商店审核、证书采购和发布渠道运营。
- 大规模云压测或分布式执行。

## 依赖

- [[00-integration-overview|集成层实施总览]]：提供正式调用链和工作包边界。
- [[01-domain-ipc-contracts|领域模型与 IPC 契约]]：提供请求、快照、事件和 capability 断言。
- [[02-progress-error-cancellation|进度、错误与取消]]：提供状态机、错误码与取消验收依据。
- 前端实施文档：提供 Create Task、任务列表和恢复行为。
- 后端实施文档：提供 JobManager、engine、输出事务和依赖 feature 组合。

## 测试分层

### 1. 契约测试

- 验证前后端共享枚举、默认值、可选字段和协议版本。
- 对固定请求、快照、事件、错误和 capability 样例做序列化 round-trip。
- 验证未知字段、未知枚举和缺失字段的兼容行为。
- 验证每个 encoder 的参数只出现在对应命名空间中。

### 2. Engine 集成测试

- 不启动 Tauri，直接使用 local engine 处理 fixture。
- 覆盖输入识别、operation 顺序、encoder 选择、元数据和输出策略。
- 断言结构化结果、输出格式、尺寸、可解码性和文件完整性。
- 不把压缩文件字节完全一致作为普遍标准；仅在上游保证确定性时使用哈希断言。

### 3. JobManager 集成测试

- 使用真实 local engine 或受控测试实现验证队列和状态机。
- 覆盖受控并发、批量失败隔离、取消竞态、revision 和终态归约。
- 验证前端断开不影响任务执行，重新获取 snapshot 可恢复状态。
- 验证清理和重试不会篡改历史任务结论。

### 4. Tauri IPC 集成测试

- 通过实际 command 路径提交、查询、取消和清理任务。
- 验证 adapter 校验、错误序列化和 capability 与构建 feature 一致。
- 确认 command handler 不承担长时执行。
- 确认正式二进制中没有启动 rimage CLI/sidecar 的行为与配置。

### 5. UI E2E

- 从 Create Task 选择输入、配置参数并提交。
- 观察任务从排队到终态，核对进度和结果摘要。
- 覆盖窗口重载、语言切换、错误详情、取消和再次查询。
- UI 断言以用户可见结果和后端 snapshot 为准，不依赖内部组件结构。

### 6. 安装包冒烟

- 在干净或接近干净的目标系统安装并启动应用。
- 处理至少一张 fixture，确认输出可读取。
- 验证无外置 `rimage.exe` 依赖、无缺失动态库、无权限异常。
- 验证卸载和升级不破坏用户已有图片；任务历史是否保留按产品策略检查。

## Fixture 设计

### 最小核心集合

- 小尺寸 RGB JPEG，带和不带 EXIF orientation。
- 透明 PNG，用于 alpha 与不支持透明输出的策略。
- 包含 ICC/EXIF 的输入，用于保留与移除策略。
- 极小图、奇数尺寸图和较大图，用于 resize 边界。
- 损坏文件、伪装扩展名和不支持格式。
- Unicode、空格和较长路径；Windows 还需覆盖盘符与受限目录。

### Fixture 原则

- 文件小、许可证清晰、来源记录完整。
- 每个 fixture 对应明确风险，避免无目的堆积。
- 原始输入只读；测试输出写入独立临时目录。
- 预期结果优先断言语义：格式、尺寸、alpha、元数据、可解码性、大小趋势和结构化状态。

## 关键端到端场景

### 成功链路

- 单输入使用默认参数成功输出。
- 多输入按固定并发完成，任务计数与 item 终态一致。
- 各首版 encoder 至少有一个能力驱动的成功用例。
- Resize、metadata、overwrite、suffix 和输出目录策略组合正确。

### 参数与能力

- 不支持的 encoder feature 不出现在 UI，构造请求时后端仍能拒绝。
- encoder-specific 非法范围、互斥参数和缺失必填值被稳定拒绝。
- 前端显示的默认值与后端规范化结果一致。

### 失败隔离

- 批量任务中单个损坏文件失败，其余输入继续处理。
- 输出目录无权限时返回 Output 分类错误并保留诊断上下文。
- 同名输出按覆盖策略得到成功、跳过或冲突错误，不静默覆盖。

### 取消与恢复

- 排队任务取消后不进入 engine。
- 运行任务进入 Cancelling，安全检查点后结束。
- 取消与自然成功竞态只产生一个最终结果。
- 窗口刷新后任务继续，UI 通过 snapshot 恢复。
- 事件缺失或乱序时，revision 触发对账而非状态倒退。

### 输出完整性

- 编码或写入失败不留下伪成功文件。
- 临时文件按策略清理，应用意外退出后可识别残留。
- backup 与 overwrite 在成功、失败、取消路径中行为一致。

## CI 与发布门禁

### 每次提交

- 前端类型检查与生产构建。
- Rust 格式、静态检查、单元测试和核心集成测试。
- 契约 fixture round-trip。
- 禁止新增 sidecar、shell 启动和外部 `rimage.exe` 打包配置的架构检查。

### 合并到主分支

- JobManager 状态机、取消竞态和 engine fixture 测试。
- Tauri command 集成与 capability 一致性测试。
- Windows 主平台 debug/release 构建。
- 依赖许可证与已知安全问题检查。

### 发布候选

- 所有声明支持平台的 release 构建。
- 安装包冒烟和真实 UI E2E。
- 性能基线、内存峰值与大批量任务稳定性对比。
- rimage revision、features、第三方 notices 和变更记录确认。
- 人工验证关键系统行为：窗口控制、拖放、语言持久化、取消、关闭应用。

### 阻断条件

- 任一平台无法静态集成所声明的 rimage features。
- 任务状态不一致、终态重复、取消产生损坏输出。
- capability 与实际构建能力不一致。
- 安装后仍依赖未声明的外置 exe 或运行库。
- 固定 fixture 出现无法解释的质量、尺寸、元数据或错误分类变化。

## rimage 升级流程

1. 单独提交依赖 revision 与 feature 变化，禁止混入无关功能。
2. 审阅上游 codecs、operations、公共类型和行为变更。
3. 对照 neo-rimage local engine 的适配点，记录需要同步的高层语义。
4. 运行完整 engine fixture、性能基线和平台构建矩阵。
5. 对任何预期差异更新设计说明和测试依据，不直接刷新黄金结果掩盖回归。
6. 更新第三方许可证、来源 commit 和 release notes。

## 分阶段任务

### 阶段 1：测试基线

- 建立最小 fixture 集、来源说明和临时输出约定。
- 建立契约样例与 engine 单图成功测试。
- 在 CI 中运行前后端基础构建与 Rust 测试。

### 阶段 2：任务系统覆盖

- 添加批量、并发、失败隔离、revision 和 snapshot 恢复测试。
- 添加排队取消、运行取消和终态竞态测试。
- 建立输出事务与残留临时文件测试。

### 阶段 3：真实应用 E2E

- 覆盖 Create Task、任务列表、错误详情和取消。
- 覆盖窗口刷新、事件对账和 capability 驱动 UI。
- 在 Windows 安装包上完成首次冒烟。

### 阶段 4：发布矩阵

- 引入 release 构建、平台矩阵、性能基线和依赖审计。
- 固化 rimage 升级清单和回归报告模板。
- 定义候选版签署人与阻断问题处理流程。

## 交付物

- 许可证清晰的图片 fixture 集及风险映射表。
- 契约、engine、JobManager、IPC 和 UI E2E 测试套件。
- CI 快速检查、主分支检查和候选版矩阵。
- 安装包冒烟清单与执行记录模板。
- 性能基线和 rimage 升级回归模板。
- 发布门禁、阻断条件和签署责任表。

## 验收标准

- 首版每个受支持 encoder 都至少通过一个真实 engine 与 UI E2E 场景。
- 批量任务可证明单项失败隔离、计数一致和终态唯一。
- 排队取消、运行取消、窗口刷新和事件丢失均有自动化覆盖。
- fixture 输出能够被独立 decoder 重新读取，且符合预期尺寸、格式和元数据策略。
- release 安装包在目标 Windows 环境无需 `rimage.exe` 即可完成任务。
- rimage revision 升级无法绕过完整回归和差异记录。

## 避坑点

- 不只测试 command 返回成功；图片处理发生在异步任务中，必须等待并验证终态与输出。
- 不用大量二进制黄金文件掩盖语义；不同 codec 或平台可能产生合法的字节差异。
- 不让 E2E 依赖开发机的固定绝对路径、现有图片或用户设置。
- 不用人为延时假设任务已完成；应等待状态或可观测条件。
- 不在单元测试中 mock 掉所有 engine 行为，否则无法发现参数映射问题。
- 不把 debug 构建成功等同于安装包可运行。
- 不在更新依赖时无审查地重录 snapshot 或黄金结果。
- 不忽略取消和失败路径的临时文件、backup 与覆盖策略。

## 可分派工作包

| 工作包 | 任务 | 依赖 | 交付 |
| --- | --- | --- | --- |
| QA-01 Fixture 基线 | 准备合法、小型、可复现的输入集 | 无 | fixture 与来源说明 |
| QA-02 契约回归 | 固定请求、事件、快照和错误样例 | [[01-domain-ipc-contracts]] | round-trip 测试 |
| QA-03 Engine 集成 | 覆盖格式、operation、encoder 和输出 | QA-01 | engine fixture 测试 |
| QA-04 JobManager 场景 | 覆盖并发、失败、revision、取消 | [[02-progress-error-cancellation]] | 状态机集成测试 |
| QA-05 Tauri/UI E2E | 覆盖真实提交、观察、恢复和取消 | QA-02/03/04 | 应用 E2E 套件 |
| QA-06 安装包冒烟 | 在目标环境验证静态集成和输出 | QA-05 | 冒烟记录 |
| QA-07 发布矩阵 | 建立平台、性能、许可证和升级门禁 | QA-03/06 | 发布检查单 |

## 导航

- 架构入口：[[00-integration-overview|集成层实施总览]]
- 协议依据：[[01-domain-ipc-contracts|领域模型与 IPC 契约]]
- 运行时依据：[[02-progress-error-cancellation|进度、错误与取消]]

