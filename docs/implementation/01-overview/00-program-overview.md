---
title: neo-rimage 项目实装总览
status: active
area: architecture
tags:
  - neo-rimage
  - architecture
  - roadmap
depends_on:
  - "[[docs/rimage-core-integration-review]]"
---

# neo-rimage 项目实装总览

## 1. 项目目标

把现有 neo-rimage 原型建设为 rimage 的原生桌面 GUI：用户通过图形界面选择输入、编码器、预处理和输出策略，Tauri 后端在同一进程内直接调用 rimage library，并以可靠的任务队列提供进度、取消、错误和结果。

最终产品必须做到：

- GUI 与 rimage codecs/operations 的能力一致；
- 不依赖外部 rimage 可执行文件；
- 同一任务配置可以稳定序列化、验证和回放；
- 长时间批处理不会因状态漂移、过度并发或单文件错误导致应用失控；
- Windows 安装包包含运行所需核心能力；
- 后续升级 rimage 时有明确的兼容验证流程。

## 2. 明确不做的事情

- 不把 CLI 的 Clap 参数、console 输出和进度条直接移入 GUI。
- 不通过 subprocess/sidecar 执行 rimage exe。
- 不复制 rimage 的 codec 实现。
- 不允许前端自行伪造后端 task/worker 运行状态。
- 不把 Tauri API、窗口对象或 emit 调用放进 engine。
- 不在第一阶段实现所有高级参数；先建立正确的任务闭环。
- 不以“能跑一张图片”替代错误、取消、输出冲突和恢复设计。

## 3. 产品能力域

| 能力域 | 产品职责 | 主要归属 |
| --- | --- | --- |
| Create Task | 输入、编码器、operations、输出设置与校验 | Frontend + IPC contract |
| Queue | 创建、排序、启动、暂停、取消、清理任务 | JobManager |
| Engine | decode、operations、encode、write | Backend local engine + rimage library |
| Progress | 阶段、百分比、当前文件、统计 | Engine reporter + JobManager + events |
| Output | 路径、suffix、backup、metadata、冲突策略 | Engine output module |
| Native UX | 拖放、窗口、文件选择、设置、多语言 | Frontend/Tauri adapter |
| Reliability | 错误隔离、取消、并发、恢复、日志 | Backend + integration |
| Release | feature matrix、依赖固定、安装包、许可 | Integration/release |

## 4. 不可变架构原则

### 4.1 单一状态源

后端 JobManager 保存运行时真相。前端 store 只保存后端快照和纯 UI 状态。

### 4.2 Engine 纯净边界

Engine 接收强类型 JobSpec，返回 Result 并报告 progress。它不认识 React、Tauri command、窗口或前端 DTO。

### 4.3 Adapter 薄层

Tauri adapter 只负责：权限边界、DTO 转换、调用 JobManager、发送事件。业务规则不应散落在 commands 中。

### 4.4 显式能力

任何显示在 Create Task 中的控件都必须满足：可编辑、可验证、可序列化、可被 engine 消费、可通过测试证明生效。做不到时应隐藏或明确 disabled。

### 4.5 有界并发

外层文件并发与 codec 内部并行共用资源预算。不得通过不断创建 OS 线程扩大处理能力。

### 4.6 可升级

本地 engine 复制的是 orchestration 思路和必要流程，不复制 codec。所有来源代码都记录 rimage commit，并用 fixtures 保障升级兼容。

## 5. 成功指标

### 功能成功

- 用户可以创建包含完整配置的任务并得到真实输出。
- MozJPEG 形成第一个完整纵向切片，随后其他 codecs 按同一模式接入。
- Queue 状态、进度、错误和取消与后端一致。
- 输出目录、suffix、backup、metadata 与设计一致。

### 工程成功

- 前后端契约有版本和验证规则。
- Engine、JobManager、Tauri adapter 可分别测试。
- 单文件失败不会终止整个队列或 Tauri 进程。
- 并发设置可以预测 CPU/RAM 行为。
- Release 构建不依赖开发机 sibling path。

### 维护成功

- rimage 升级有固定检查清单。
- 每个模块有明确 owner、输入、输出和 DoD。
- 新 codec 不需要修改无关模块或复制一套新队列逻辑。

## 6. 工作流划分

| Workstream | 责任 | 可并行时机 |
| --- | --- | --- |
| Architecture/Contracts | 冻结 domain、IPC、状态与错误语义 | 最先进行 |
| Backend Engine | 提取并重构 orchestration | 可与前端表单骨架并行 |
| Backend Queue | JobManager、并发、取消与事件 | contract 冻结后 |
| Frontend Form | Create Task 表单和校验 | contract 草案后 |
| Frontend Runtime | Queue/worker/progress views | snapshot/event contract 后 |
| Integration/QA | fixtures、E2E、打包、升级检查 | 从第一纵向切片开始持续进行 |

## 7. 关键参考

- [[docs/implementation/01-overview/01-target-architecture|目标架构]]
- [[docs/implementation/01-overview/02-delivery-roadmap|交付路线图]]
- [[docs/implementation/04-integration/01-domain-ipc-contracts|Domain 与 IPC 契约]]
- [[docs/implementation/05-execution/00-agent-work-packages|Agent 工作包]]

