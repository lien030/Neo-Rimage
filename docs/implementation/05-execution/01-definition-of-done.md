---
title: neo-rimage Definition of Done
status: active
area: execution
tags:
  - neo-rimage
  - quality-gate
  - definition-of-done
depends_on:
  - "[[docs/implementation/00-index]]"
---

# neo-rimage Definition of Done

## 1. 全局 DoD

任何工作包只有同时满足以下条件才算完成：

- 范围内需求全部实现；范围外内容没有被隐式加入。
- 没有改变已冻结架构决策或 IPC contract；如有变更，存在批准记录。
- 模块职责清晰，没有把规则放入错误层。
- 新增行为有对应自动测试或可重复验证证据。
- 错误路径、空状态、取消/关闭场景已考虑。
- 不存在仅展示但无效的 UI 控件。
- 不存在前端伪造后端运行状态。
- 不存在 rimage exe/sidecar/CLI 输出解析。
- 不存在无界线程、global rayon pool 或未说明的并发扩大。
- 文档、风险登记和依赖关系已同步。
- 工作树未覆盖用户或其他 Agent 的无关修改。
- 交付报告包含修改范围、测试结果、限制与后续影响。

## 2. Architecture/Contract DoD

- 字段语义、默认值、可选性、范围和冲突条件明确。
- request、snapshot、event、error 不混用同一类型承担不同职责。
- 状态转换有明确 owner。
- 版本/兼容策略明确。
- 前端和后端都能根据文档实现而无需猜测。
- 至少有正向、边界、无效和兼容 fixture。

## 3. Frontend DoD

- Form、UI、backend snapshot state 分离。
- invoke/listen 通过统一 IPC 层使用。
- 组件卸载后 listener 正确释放。
- Create Task 不因 render/HMR 重置草稿。
- 可见参数均进入 request，disabled 参数有原因说明。
- loading、empty、error、cancelled、completed 状态均有表现。
- 键盘操作、焦点、tooltip/label 和中英日布局经过检查。
- Production build 与 Tauri dev 模式都验证过，避免仅一种 bundler 模式有效。

## 4. Backend Engine DoD

- Engine 无 Tauri/React/CLI 依赖。
- 底层直接调用 rimage library。
- 每个 pipeline 阶段有结构化错误上下文。
- 单文件失败不影响其他 task。
- cancellation checkpoint 和临时输出清理规则明确。
- 输出写入、backup 和 metadata 有失败测试。
- 来源于 rimage 的 orchestration 代码记录 commit 和许可证。
- 与选定 rimage fixture 的结果完成对照。

## 5. JobManager/Adapter DoD

- 后端是 queue/task/progress 的唯一状态源。
- 所有状态转换可测试且不会跳过非法状态。
- 并发有上限，修改 concurrency 不破坏在途任务。
- snapshot 带 revision/version，事件丢失可重同步。
- Tauri commands 保持薄层并返回结构化错误。
- 高频 progress 不阻塞 engine 或淹没前端。
- 应用退出、panic、event receiver 消失有定义行为。

## 6. Codec Feature DoD

每个 codec 单独满足：

- 参数、范围、默认值与选定 rimage 版本一致。
- Domain config 与 IPC contract 已定义。
- Engine adapter 已映射并有测试。
- Create Task panel 已实现或明确说明无 codec-specific 参数。
- 所有可见控件能影响最终请求和输出。
- 支持/不支持的颜色空间、动画、bit depth 有说明。
- 至少一个成功 fixture 和一个错误/边界 fixture。
- 与其他 codec 不共享会互相污染的 mutable defaults。

## 7. Integration DoD

- Create → queue → processing → result 完整链路通过。
- 进度、取消、失败、重试、清理在前后端一致。
- 事件乱序/丢失后可恢复。
- 真实文件输出符合 output contract。
- 无权限、冲突、损坏输入、磁盘错误至少完成代表性验证。
- Tauri dev 与打包应用均通过。

## 8. Release DoD

- rimage 来源固定且许可记录完整。
- Release 不使用开发机 sibling path。
- MSI/NSIS 从干净构建产生。
- 安装版在无 Rust/Node 开发环境机器上验证。
- feature matrix 与安装包能力一致。
- 升级、回滚和已知问题有记录。
- 不包含调试端口、开发日志或宽泛无必要权限。

## 9. 证据模板

每个 Agent 的完成报告至少填写：

```text
Work package:
Delivered:
Changed paths:
Contract impact:
Tests/checks:
Manual verification:
Known limitations:
Risks added/closed:
Follow-up dependencies:
```

## 10. 不接受的“完成”定义

- “TypeScript/Cargo 能编译”但功能未形成闭环。
- “按钮能点”但请求未到 engine。
- “Rust 返回成功”但输出/状态不正确。
- “生产包有效”但 `tauri dev` 失效，或反之。
- 通过 sleep、忽略错误或前端乐观伪造掩盖竞态。
- 复制 CLI 代码但没有去除 Clap/global pool/console 耦合。
- 只测试单一小图，未考虑错误和资源边界。

