---
title: Create Task 表单实施计划
status: draft
area: frontend
tags:
  - neo-rimage
  - frontend
  - create-task
  - form
depends_on:
  - "[[00-frontend-overview]]"
  - "[[../../rimage-core-integration-review]]"
---

# Create Task 表单实施计划

## 1. 目标

将 Create Task 从“可打开的参数面板”变为完整任务创建流程：用户选择输入、配置处理方式、看到有效性反馈、提交给后端，并仅使用后端返回的任务快照进入队列。

表单必须与队列状态严格分离：

- 表单是可编辑草稿，允许暂时不完整。
- 提交请求是已验证且不可变的一次性意图。
- 队列任务是后端创建的权威快照，不能从表单对象直接派生并写入任务表。

总体状态原则见 [[00-frontend-overview#4. 状态分层原则|前端状态分层原则]]。

## 2. 当前未闭环项

| 项目 | 当前表现 | 目标 |
| --- | --- | --- |
| 提交动作 | Dialog 没有 Create/Confirm | 提供明确创建动作并显示提交状态 |
| 取消动作 | 关闭即清空输入缓存，缺少 dirty 提示 | 明确取消、保留草稿、丢弃草稿规则 |
| 输入选择 | 拖放可形成前端 cache；文件按钮只打开 Dialog | 文件/目录选择与拖放进入同一候选输入流程 |
| 编码器选择 | tab 默认值不写回配置 | 选择结果进入提交请求 |
| 参数写回 | 多个 MozJPEG、Resize 控件仅展示或只在失焦写入 | 所有可见控件具备稳定、可验证的值 |
| Resize 顺序 | `firstly` checkbox 未写回且表达能力不足 | 用有序预处理步骤表达 |
| 输出选项 | suffix、backup、输出目录、保留结构等尚无完整 UI | 形成明确的 Output 区域 |
| 参数校验 | 无范围、依赖、路径和冲突校验 | 提交前基础校验，后端做最终校验 |
| 创建任务 | 未调用完整创建命令 | 发送一次请求并接收任务快照 |
| 失败恢复 | 调用错误没有统一呈现 | 保留草稿、定位字段或显示全局错误 |

## 3. 范围

### 3.1 本模块负责

- 文件与目录候选的添加、去重、移除、全选和空状态。
- 编码器选择及其参数面板。
- 预处理步骤的配置与顺序。
- 输出、metadata 和执行相关设置的编辑。
- 字段级与跨字段基础校验。
- 创建请求提交、loading、防重复提交和错误恢复。
- 成功后关闭 Dialog，并将后端快照交给队列同步层。

### 3.2 本模块不负责

- 在前端确认文件一定可解码。
- 在前端展开目录并决定最终支持文件集合。
- 计算最终输出路径、覆盖结果或备份文件名。
- 直接新增 `taskList`、设置任务状态或启动处理。
- 管理 worker 数量和任务执行线程。

队列操作与运行态由 [[02-task-queue-and-workers]] 负责。

## 4. 依赖

- 创建任务请求、验证错误和任务快照契约已经冻结。
- 后端能接受文件/目录输入，并返回规范化、拒绝或创建结果。
- 后端提供编码器 capability，或双方冻结相同的第一阶段支持矩阵。
- 文件与目录选择的原生能力可用，见 [[03-native-shell-settings-i18n#4. 文件选择与拖放|文件选择与拖放计划]]。
- i18n 已定义字段名、说明、错误和格式说明。

## 5. 表单信息架构

建议将 Dialog 分为四个稳定区域，避免所有选项挤在编码器 tab 中。

### 5.1 Inputs

- 展示用户选择或拖入的文件、目录及其来源类型。
- 使用完整规范化路径或候选 id 区分项目；文件名只用于显示。
- 支持移除单项、清空、重复项提示和后端拒绝项说明。
- 输入目录递归扫描是输入发现选项，名称必须与输出结构保留区分。
- 大量输入时使用摘要和虚拟化/分页策略，避免 Dialog 因上千条目卡顿。

### 5.2 Encoder

- 编码器选择是单选，切换时保留各编码器草稿，但最终只提交当前编码器配置。
- 每个 tab 都必须有内容；没有 codec-specific 参数的格式应展示格式说明，而不是空白。
- 不支持或尚未接入的编码器应隐藏或标记为不可用并解释原因。
- 编码器能力和默认值以 engine 契约为准，不从旧 CLI UI 猜测。

### 5.3 Operations

- 将 resize、quantization、dithering、premultiply alpha 表达为有序步骤。
- 用户能看懂执行顺序，并能调整、删除和恢复默认顺序。
- 依赖项应联动：例如 dithering 依赖 quantization，不允许形成无效组合。
- 旧的 `resize firstly` 不能继续作为唯一顺序模型。

### 5.4 Output & Execution

- 输出目录、结构保留、suffix、源文件备份、metadata strip/report 单独分组。
- 并发属于队列/应用执行策略，不应被误解为“单图编码线程数”。若允许在创建时指定，应明确它是队列级策略还是任务级覆盖。
- 对覆盖、同名输出、备份失败和无写权限只做预提示；最终判断由后端完成。

## 6. 参数交付范围

### 6.1 General / Output

| 功能 | 产品语义 | 第一阶段要求 |
| --- | --- | --- |
| 输入文件/目录 | 创建任务的候选输入 | 必须闭环 |
| 输入目录递归 | 是否递归发现目录内文件 | 必须与输出结构保留分开命名 |
| 输出目录 | 指定输出根目录 | 必须闭环 |
| 保留目录结构 | 输出时保留输入层级 | 必须闭环 |
| suffix | 输出文件名附加后缀 | 必须校验非法字符和空值语义 |
| 备份源文件 | 覆盖相关场景保留源文件 | 必须展示风险说明 |
| strip metadata | 移除支持的 metadata | 能力可用后交付 |
| metadata report | 生成处理报告及保存位置 | 可作为第二阶段 |
| concurrency | 队列并发预算 | 由全局队列设置优先管理 |

CLI 专属的 quiet/no-progress 不进入 Create Task。

### 6.2 Preprocessors

| 功能 | 要求 |
| --- | --- |
| Resize 模式 | 支持固定尺寸、按宽、按高、百分比和倍率时，使用互斥模式而非同时暴露冲突输入 |
| 放大/缩小策略 | 明确允许 upscale、downscale 的行为和默认值 |
| Resize filter | 只展示 engine 确认支持的 filter；不能漏掉支持项或出现无效项 |
| 多步骤 Resize | 后端支持后再开放；必须展示顺序 |
| Quantization | quality 范围、默认值和是否启用需清晰 |
| Dithering | 仅在 quantization 启用时可配置 |
| Premultiply alpha | 作为有序 operation，不作为无上下文的孤立开关 |

第一阶段可先交付单个 Resize + MozJPEG 的纵向闭环，但数据模型应允许后续扩展有序步骤。

### 6.3 Encoder 选项矩阵

| Encoder | 必要选项 | 交付说明 |
| --- | --- | --- |
| MozJPEG | quality、chroma quality、progressive/baseline、optimize coding、smoothing、colorspace、multipass、chroma subsample、qtable | 第一优先级；所有可见控件必须写回并校验 |
| JPEG | quality、progressive | 作为简化 JPEG 方案交付 |
| AVIF | quality、alpha quality、speed、colorspace、alpha mode | 需要解释速度与压缩率取舍 |
| OxiPNG | interlace、effort | 只暴露 rimage 已验证 preset，不扩张底层高级项 |
| WebP | lossless、quality、slight loss、exact | 处理 lossless 与 quality 的冲突；`discrete` 暂不开放 |
| JPEG XL | 当前为 lossless | 无参数时展示能力说明 |
| PNG | 无 codec-specific 参数 | 展示能力说明和输出格式 |
| Farbfeld | 无 codec-specific 参数 | 展示能力说明和输出格式 |
| PPM | 无 codec-specific 参数 | 展示能力说明和输出格式 |
| QOI | 无 codec-specific 参数 | 展示能力说明和输出格式 |

注意：当前 Create Task 缺少 AVIF 和 Farbfeld tab，其他多数 tab 只有 trigger，没有实际内容。不能把“有 tab”视为已实现。

## 7. 交互流程

### 7.1 打开

- 从主界面“添加”、文件选择或拖放进入同一 Create Task 流程。
- 新建模式使用明确默认值；若存在未提交草稿，按产品规则提示继续编辑或重新开始。
- Dialog 打开后不因 tab 切换、翻译切换或输入候选变化而重置参数。

### 7.2 编辑

- 字段变更立即反映在草稿中；失焦校验用于补充，不作为唯一写回时机。
- 无效字段显示靠近控件的原因；跨字段冲突显示在相关分组。
- 切换编码器时保留该编码器先前草稿，但只验证当前激活配置。
- 高风险选项提供短说明，完整解释放在 tooltip 或帮助入口。

### 7.3 提交

- 空输入、无效范围、缺失路径和冲突选项不能提交。
- 提交期间防止重复请求，允许用户看到“正在创建”状态。
- 前端基础校验通过后提交完整请求，后端仍执行最终校验。
- 后端返回部分成功时，必须分别展示已创建项和被拒绝项，不可静默丢弃。

### 7.4 成功与失败

- 全部成功：接收任务快照、同步队列、关闭 Dialog，并清理本次草稿。
- 部分成功：已创建任务进入队列，被拒绝输入保留在 Dialog 并附错误。
- 全部失败：保持 Dialog 和所有草稿，聚焦首个可修复问题。
- 用户取消：若草稿已修改，明确询问丢弃；未修改则直接关闭。

## 8. 交付物

- 稳定的 Create Task 草稿模型和默认值策略。
- 输入候选管理与文件/目录统一入口。
- General、Output、Operations 和 Encoder 分区。
- 第一阶段支持的编码器表单及能力说明。
- 字段级、跨字段和提交级错误呈现。
- 创建任务 loading、成功、部分成功、失败和取消流程。
- Create Task 功能测试用例和参数覆盖表。

## 9. 阶段任务

### Phase CT1：表单骨架与请求边界

- 冻结表单值、创建请求、后端错误三者边界。
- 建立稳定默认值、dirty 判定、取消和 reset 规则。
- 统一输入来源和候选去重规则。

### Phase CT2：最小纵向闭环

- 完成 Inputs、MozJPEG、单 Resize、基础 Output 和 Create/Cancel。
- 接入真实创建命令，使用后端快照进入队列。
- 覆盖成功、部分成功、后端拒绝和命令失败。

### Phase CT3：补齐通用能力

- 完成输出目录、结构保留、suffix、backup、metadata 相关选项。
- 将 Operations 升级为有序步骤并补 quantization/dithering。
- 完成路径、数值、依赖和冲突提示。

### Phase CT4：扩展编码器

- 按 JPEG、AVIF、OxiPNG、WebP 的顺序逐个交付并做契约验收。
- 为无专属参数格式补充说明页。
- 对 engine 未消费的参数保持隐藏，待后端确认后再开放。

## 10. 验收标准

- 用户可从添加按钮、选择器和拖放进入同一 Create Task 流程。
- Dialog 任意正常重渲染不会丢失表单输入。
- 每个可见控件均能影响最终创建请求，且请求字段与界面文案一致。
- 编码器切换、operation 顺序和输出选项能被完整校验。
- 同名不同路径输入可分别移除和提交，重复路径不会重复创建。
- 提交成功后任务来自后端快照；前端不自行生成任务状态。
- 后端返回字段错误时可定位到相应控件，非字段错误有统一提示。
- 部分成功不会丢失失败项，重复点击不会创建重复任务。
- 未实现参数不会以可操作控件形式出现。

## 11. 避坑点

- 不要在组件 render 中重新构造整个默认配置。
- 不要依赖 `defaultValue` 和失焦事件作为业务值的唯一来源。
- 不要用数字 tab 值直接充当跨 IPC 的编码器标识。
- 不要继续用 `firstly: boolean` 模拟完整 operation 顺序。
- 不要把 MozJPEG 的 baseline 反向 flag 直接显示成含义相反的 switch。
- 不要展示 MozJPEG 核心不支持的 colorspace 候选。
- 不要开放 WebP `discrete`，直到确认 engine 实际消费该参数。
- 不要让切换编码器时残留的无关配置进入请求。
- 不要在前端预判最终输出文件名后承诺不会覆盖；以 Rust 校验为准。
- 不要在提交失败后清空草稿，否则用户无法修正问题。

## 12. 可交给子 Agent 的工作包

| 工作包 | 任务范围 | 依赖 | 交付物 | 完成条件 |
| --- | --- | --- | --- | --- |
| CT-A 草稿与校验 | 表单生命周期、默认值、dirty、reset、错误映射 | 冻结请求契约 | 表单状态与校验规则说明/实现 | 重渲染和失败不丢值 |
| CT-B 输入候选 | 选择、拖放、去重、移除、部分拒绝 | 原生选择能力、后端输入校验 | Inputs 区域 | 同名路径安全、批量可用 |
| CT-C 基础闭环 | MozJPEG、单 Resize、Output、Create/Cancel | CT-A、CT-B、创建命令 | 可创建真实任务的最小版本 | 后端快照进入队列 |
| CT-D Operations | 有序 resize、quantize、dither、premultiply | engine operation 契约 | Operations 编辑区 | 顺序和依赖可验证 |
| CT-E Codec 扩展 | JPEG、AVIF、OxiPNG、WebP 及说明页 | capability 矩阵 | 完整 encoder tabs | 可见参数全部有效 |
| CT-F 错误体验 | 字段错误、全局错误、部分成功、重试 | 后端错误结构 | 统一反馈流程 | 无“点击无反应”路径 |

CT-C 应先于 CT-D/CT-E 完成，确保后续每新增一个参数都沿同一提交链交付。
