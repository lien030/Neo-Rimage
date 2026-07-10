---
title: neo-rimage 风险登记册
status: active
area: governance
tags:
  - neo-rimage
  - risks
  - architecture
depends_on:
  - "[[docs/implementation/01-overview/01-target-architecture]]"
---

# neo-rimage 风险登记册

## 1. 评分规则

- 影响：Critical / High / Medium / Low。
- 概率：High / Medium / Low。
- Owner 负责监控和推动缓解，不代表只能由 Owner 实现。
- 风险触发后必须记录实际影响和决策，不能只保留初始描述。

## 2. 活跃风险

| ID | 风险 | 影响 | 概率 | Owner | 主要缓解 |
| --- | --- | --- | --- | --- | --- |
| R-001 | Local engine 与 rimage 上游 pipeline 漂移 | High | High | Engine owner | 固定 revision、来源注释、fixture parity、升级 diff checklist |
| R-002 | path dependency 依赖开发机目录 | Critical | High | Release owner | 发布使用 Git revision/submodule/vendor，不依赖 sibling path |
| R-003 | codec native dependency 在 Windows 打包失败 | High | Medium | Backend/Release | CI 干净构建、feature matrix、MSVC/工具链记录 |
| R-004 | 外层文件并发与 codec 内部线程叠加 | Critical | High | JobManager owner | 统一并发预算、私有 pool、内存压力测试 |
| R-005 | 大图/动画导致 OOM | Critical | Medium | Engine owner | 有界并发、输入预检、内存指标、动画能力说明 |
| R-006 | backup/output 失败造成源文件丢失 | Critical | Medium | Output owner | 临时文件、原子替换、回滚、故障注入 |
| R-007 | cancellation 在编码中途无法及时生效 | High | High | Engine/JobManager | 定义 checkpoint 和“请求取消”状态，不承诺不可实现的即时中断 |
| R-008 | Tauri events 丢失或乱序导致 UI 假状态 | High | Medium | Integration owner | revision、snapshot 重同步、事件幂等 |
| R-009 | 前后端 contract 各自演化 | Critical | Medium | Architecture owner | contract owner、fixtures、版本字段、变更 gate |
| R-010 | 数字枚举顺序变化产生静默错配 | High | Medium | Contract owner | 字符串 tagged enums、未知值处理 |
| R-011 | 前端显示参数但 engine 未消费 | High | High | Frontend/QA | visible-control audit、纵向 fixture、feature flags |
| R-012 | 文件类型前端 MIME 判断与核心不一致 | Medium | High | Platform/Engine | 格式发现移至 Rust；前端不维护支持列表真相 |
| R-013 | 宽泛文件系统权限扩大攻击面 | High | Medium | Tauri adapter | 最小权限、后端文件操作、路径 scope 审查 |
| R-014 | codec panic 终止 Tauri 进程 | Critical | Low-Medium | Engine owner | panic boundary、fixture/fuzz、单 task 隔离 |
| R-015 | 应用关闭时在途任务和临时文件不一致 | High | Medium | JobManager/Output | close policy、graceful shutdown、恢复/清理规则 |
| R-016 | rimage feature 组合与 CLI 能力不一致 | High | Medium | Backend owner | 显式 feature matrix，特别是 ICC/metadata/threads |
| R-017 | copied orchestration 许可/来源缺失 | High | Low | Release owner | MIT/Apache 记录、文件来源 header、third-party notices |
| R-018 | HMR 与 production bundler 行为不同 | Medium | Medium | Frontend QA | `tauri dev` 与 production build 双模式 gate |
| R-019 | 任务/设置持久化 schema 升级失败 | High | Medium | Integration owner | v1 前明确是否持久化；版本化迁移与备份 |
| R-020 | 文档过度设计导致首个纵向切片延迟 | Medium | Medium | Program owner | Phase gates、MozJPEG first、禁止同时铺开全部 codecs |

## 3. 关键风险详述

### R-001：上游漂移

**触发信号**：rimage 更新 codec options、decode dispatch、metadata 或输出行为；本地 engine fixtures 开始与 CLI 不一致。

**缓解**：

- 本地 engine 文件记录来源 commit 和对应上游路径；
- 升级时检查 `src/main.rs`、`src/cli/pipeline.rs`、codec option structs；
- 只复制 orchestration，不复制 codec；
- 对每个已支持 codec 运行 parity fixtures。

**应急**：冻结旧 revision，先发布兼容版本，再单独处理升级差异。

### R-004/R-005：并发与内存

**触发信号**：高并发时内存非线性增长、系统换页、任务大量同时 decode、CPU 线程数显著超过预算。

**缓解**：

- 默认低外层并发；
- 任务进入 decode 前获取 permit；
- 记录输入尺寸/帧数并允许拒绝极端任务；
- 单独测试 codec `threads` feature 开/关组合。

**应急**：降低 concurrency、暂时关闭部分 codec 内部并行、提供安全模式。

### R-006：数据安全

**触发信号**：backup rename 后 encode/write 失败；最终输出被截断；同名任务互相覆盖。

**缓解**：

- 先写临时文件，成功后原子替换；
- backup 和最终写入形成可回滚步骤；
- 输出冲突策略必须由 request 明确；
- 任何删除/覆盖操作都有专项测试。

**应急**：保留临时/backup 文件并返回恢复指引，不做静默清理。

### R-007：取消语义

**触发信号**：用户点击取消后 codec 长时间无响应，UI 显示 cancelled 但后端仍写出文件。

**缓解**：

- 区分 `cancelling` 和 `cancelled`；
- 只有 engine 到达安全 checkpoint 后才进入 cancelled；
- 正在执行的不可中断 codec 明确提示“等待当前阶段结束”；
- 取消后禁止提交最终输出。

### R-008/R-009：状态与协议

**触发信号**：UI task 状态倒退、刷新后队列不同、未知 enum 导致页面崩溃。

**缓解**：

- 每个 snapshot/event 携带 revision；
- 启动和检测缺口时全量重同步；
- 未知字段向前兼容，未知关键 kind 返回可诊断错误；
- contract fixtures 同时被前后端测试消费。

## 4. 风险审查节奏

- 每个 Phase Exit Gate 前审查一次。
- rimage revision/feature 变化时立即审查 R-001、R-003、R-016、R-017。
- output/backup/cancellation 合并前专项审查数据安全风险。
- Release candidate 前确认所有 Critical 风险有关闭证据或明确接受人。

## 5. 风险新增模板

```text
ID:
Title:
Description:
Impact:
Probability:
Owner:
Trigger:
Mitigation:
Contingency:
Affected work packages:
Status:
```

