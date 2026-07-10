---
title: neo-rimage 术语与协作约定
status: active
area: governance
tags:
  - neo-rimage
  - glossary
  - agents
depends_on:
  - "[[docs/implementation/01-overview/00-program-overview]]"
---

# neo-rimage 术语与协作约定

## 1. 术语表

| 术语 | 统一含义 |
| --- | --- |
| rimage library | `D:\Projects\Rust\rimage` 的 `src/lib.rs` 所构建 Rust library，不含 CLI exe 调用 |
| Local Engine | neo-rimage 自己维护的高层图片处理 orchestration，底层调用 rimage library |
| Tauri Adapter | commands/events/DTO 转换层，不包含图片处理和调度规则 |
| JobManager | 后端 queue、task、progress、concurrency、cancellation 的唯一状态源 |
| Task | 一个输入文件与冻结配置形成的独立执行单元 |
| Batch/Create Request | 用户一次 Create 操作，可能产生多个 Task |
| Form Values | 前端编辑中的可不完整草稿 |
| JobSpec | Engine 可直接执行的 Rust 强类型任务配置 |
| TaskSnapshot | 后端提供给前端的只读任务状态 |
| Worker Slot | 并发执行槽位的展示，不等同于永久 OS 线程 |
| Operation | resize、quantization、premultiply 等有序预处理步骤 |
| Codec Adapter | Domain encoder config 到具体 rimage/zune encoder options 的映射 |
| Fixture | 用于 CLI/GUI engine 对照测试的固定输入和预期属性 |
| Vertical Slice | 从 Create Task 到真实输出、状态和结果的完整小范围功能 |

## 2. 命名规则

- 输入目录递归：`scanRecursively`。
- 输出保留目录结构：`preserveStructure`。
- 并发数量：`concurrency`，不要再用可无限增减的 `workerCount` 表达资源控制。
- Encoder kind 使用稳定小写字符串：`mozjpeg`、`jpeg_xl`、`oxipng`。
- 状态名使用完整语义：`queued`、`processing`、`cancelling`、`cancelled`。
- 前端 DTO 与后端 domain 类型不能仅靠数字 enum 顺序对应。

## 3. 文档状态

| Status | 含义 |
| --- | --- |
| draft | 内容可用于讨论，不可作为无条件实现依据 |
| active | 当前认可的实现依据，细节可补充 |
| blocked | 存在明确前置决策或外部依赖 |
| complete | 文档对应范围已经实现并验收 |
| superseded | 已由另一文档替代，应保留跳转说明 |

## 4. Agent 工作规则

### 开始前

1. 阅读工作包的 depends_on 文档。
2. 明确允许修改的路径和禁止事项。
3. 检查是否存在其他 Agent 的重叠写入。
4. 对未决 contract 不做隐式假设。

### 工作中

- 只修改工作包授权范围。
- 不在前端复制后端业务规则。
- 不在 Tauri command 内实现 engine pipeline。
- 不引入 exe/sidecar 调用。
- 不改变用户已有 `.obsidian/`、README 或无关工作树修改。
- 若需要扩展 contract，先向集成负责人提出变更影响。

### 交付时

报告必须包含：

- 完成的能力；
- 未完成/阻塞项；
- 修改路径；
- 验证证据；
- 对其他工作包的接口影响；
- 新增风险或后续建议。

## 5. 模块文档标准结构

每个功能模块至少包含：

1. 目标；
2. 范围内/范围外；
3. 上游依赖；
4. 模块职责与边界；
5. 分阶段任务；
6. 交付物；
7. 验收标准；
8. 避坑点；
9. 子 Agent 工作包建议。

模块计划不应包含大段具体实现代码。代码接口示例只用于澄清边界，不能替代 contract 文档。

## 6. 决策升级规则

以下情况必须暂停并交由架构/集成负责人决定：

- 需要改变核心状态所有权；
- 需要新增或破坏 IPC 字段；
- 需要改变 output/backup/cancellation 语义；
- 需要新增 native dependency 或改变 rimage feature matrix；
- 需要在多个模块复制同一业务规则；
- 发现 rimage 上游行为与当前文档不一致；
- 工作包无法在声明路径范围内完成。

## 7. Obsidian 约定

- 使用仓库根目录作为 vault root。
- 内部链接使用完整 vault-relative wikilink，例如 `[[docs/implementation/00-index]]`。
- 架构图优先使用 Mermaid。
- 重要约束使用 Obsidian callout：`[!warning]`、`[!danger]`、`[!tip]`。
- 文件名稳定后不要随意重命名，以免破坏 Agent prompt 与 wikilink。

