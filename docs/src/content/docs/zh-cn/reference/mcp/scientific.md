---
title: MCP 科学运算
description: 通过 MCP 运行处理、扩散拟合和 DOSY，并为结果建图。
---

这些操作与桌面端的[处理](/zh-cn/guides/processing/)和 [DOSY](/zh-cn/guides/dosy/)
相同，命令行也可以通过 [`plotium-cli tool run`](/zh-cn/reference/cli/#单步科学操作)运行。

## 科学操作与任务

`plotium_processing_inspect`、`plotium_diffusion_inspect` 和
`plotium_analysis_results_inspect` 是只读查询，使用显式 `ids`、
`expected_project_context` 和 `expected_revision`。结果检查接受 `offset`、
`limit`（1–100）；ILT 网格较大时请减小页长。发现信息中的
`scientific_operations` 来自核心操作定义，含参数 schema、能力、权限及执行方式。

以下写操作继续通过 `plotium_plan` / `plotium_execute`：

| 操作 | 参数与目标 |
| --- | --- |
| `processing.edit` | 一个 NMR、XRD、XPS 或 XAS 数据集；`edits` 中对稳定 StepId 使用 `replace`、`enable`、`add`、`remove` 或 `move`；XPS 的 `add` 以区域 ID 作为 `axis`，步骤为 `Window`、`Smooth` 或 `Normalize`；XRD 使用 `set_xrd`，给出完整的 `processing` 设置；XAS 使用 `set_xas`，给出 `signal` 和从 `processing.inspect` 复制的完整 `processing` 设置，或用 `apply_xas_preset` 选择 `athena` 或 `larch` |
| `processing.apply_scheme` | `path`，可选 `compatible_only`；方案字节、指纹和兼容目标配置在预检时冻结；方案只应用于同类数据集，XPS 方案替换当前区域的配方 |
| `diffusion.set_input` | 一个数据集；完整 `input`；非法输入错误带结构化校验报告 |
| `analysis.regions.extract` | 一个支持区域序列的字段；`regions` 引用已有 RegionId 或给出边界和指标；可选显式 `input` |
| `analysis.diffusion.fit` | 一个原生表格；`response_column` 为稳定 ColumnId；可选原生 `options`；x 列单位必须为 `s/m²` |
| `analysis.dosy.build` | 一个扩散来源字段；`method` 为 `mono_exp` 或 `ilt`；可选 `input` 和 ILT `params` |

显式区域格式为
`{"source":"explicit","bounds":[0.8,1.2],"metric":"Height","name":"Signal"}`；
引用已有区域使用 `{"source":"existing","id":0}`。指标还包括 `Area`、
`Mean`、`Max` 和 `Min`。区域提取生成独立原生表格，保留来源、区域到列的
对应及扩散增量证据，不自动创建绘图。

除输入设置外，上述写操作返回 `ToolResult.value.task`。预检即分配
`task_id`；`queued` 只表示已登记。使用 `plotium_tasks_get({task_id})`、
`plotium_tasks_list({offset,limit})` 查询，使用
`plotium_tasks_cancel({task_id})` 取消（需要修改权限）。状态为
`queued / running / committing / succeeded / failed / cancelled / stale`。
回执包括原始 plan_id、项目归属、起止版本、来源字段版本、阶段、已完成列数、
失败代码、资源引用和终态摘要；未知总量为 `null`。回执查询不受忙碌或项目切换阻塞。

回执由应用进程持有，断线不会取消已提交任务；同一进程内重连可继续查询。
保留所有活跃任务及最近 128 个终态回执，进程退出后不保证保留。
`task_not_found` 表示未知或已淘汰，不能据此自动重算。重复执行已消费的科学
计划只返回现有回执。取消幂等，提交前取消保证不写入，终态后取消返回已有终态。

计算冻结输入后进入原有 ComputeService。主线程严格复核项目、文档版本、
字段版本、代次、取消状态及调用权限；计算期间任何文档编辑都会使任务过期。
成功时一次提交配置、结果和关联表格，形成一个 Undo 步骤；Redo 复用计算结果。
后台完成不会打开或关闭面板。同步工作流保留 `processing.apply_scheme`，等待其提交后
才执行后续节点；其他后台科学操作请使用单步调用并等待终态。

扩散检查区分可执行、缺失参数、证据待确认和输入非法，报告问题代码、参数路径、
预期单位、实际值、证据和修复建议。b 值按 IncrementId 对齐并统一为 `s/m²`，
D 为 `m²/s`；原始输入单位及换算来源保留。解析优先级为显式输入、目标结果
来源、已存默认值／采集信息。JEOL 检查完整程序身份、梯度形状、时间及厂商系数。

单指数结果包括 D、I₀、D 标准误、有效点数、R²、残差和拒绝原因。
ILT 返回 D 网格、λ、非负解、重建残差及求解终止信息，不附单指数置信度。
`empty` 表示没有可接受的科学结果；不可用数值为 `null`，附原因。
结果检查分页返回逐列诊断，来源记录保留算法版本、实际参数及输入指纹。

衰减拟合任务完成后，任务的 `produced` 列表包含稳定的 `plotium.analysis` 资源。将其 ID 传给 `plotium_analysis_results_inspect`，即可直接读取该项分析。任务状态 `succeeded` 表示计算与结果提交完成；科学收敛情况须另查 `converged`、`termination` 和 `notices`。外层 `converged` 与最低有限代价的尝试一致。耗尽迭代预算且未满足收敛条件，或因信赖域坍缩终止的尝试，仍为未收敛，其估计会保留供诊断并附警告。

## 科学结果原生建图

通过 `plotium_plan`、`plotium_execute` 调用以下可撤销操作。两者的
`parameters` 都为 `{source: ResourceRef, name: string}`，省略顶层 `ids`、
`targets`；先发现完整来源引用和 schema。

| 操作 | 来源与原生产物 |
| --- | --- |
| `plot.field.create` | 一个原生字段，包括单指数 `nmr.dosy_map` 或 ILT `nmr.ilt_map`；新建画布，固定绑定该字段及其原生编码。 |
| `analysis.diffusion.plot` | 一个仅含单组响应数据的扩散拟合分析；新建冻结表格及画布，包含 b–强度散点、已存拟合曲线、残差面板、D 标准误、R² 和有效点数。 |

预检的 `preflight.scientific_plot.summary` 返回拟议的 `canvas`、`source`、
`produced`、科学 `evidence`，执行后 `value` 返回同一摘要。建图不重新计算科学结果。
过期或空 DOSY 结果，以及过期、未收敛、多组响应拟合，须先修正。
无法估计的 D 不确定度明确标为不可用。字段准备最多 4,000,000 个元素、200 条序列，
支持 20,480 点光谱的默认 128 行 ILT；拟合图最多 20,000 行。
图中的点、等高线顶点和热图单元与显式曲线行共享 2,000,000 的待执行预算；
等高线图按生成的顶点数计入该预算，热图按单元数计入。
一次 Undo 移除新资源，Redo 恢复相同身份及数据。切换数据集显示模式不改变字段图来源。
冻结拟合表格保留来源表格血缘；自动化运行记录保留所选分析、行选择、提取证据和扩散输入。

将 `value.canvas` 传给 `plotium_figure_diagnose`、`plotium_render_preview`，
并实际查看 PNG。图像有效并不代表科学结果有效：

1. 从欢迎页调用 `plotium_get_state`，导入采集数据，检查处理状态，等待处理任务成功。
2. 检查扩散单位、IncrementId 对应关系、缺失参数和实验依据；只有证据充分时才提交已确认输入。
3. 提取前明确区域规则、边界、指标和重叠处理策略。等待提取完成，再按返回的响应 ColumnId 拟合。
4. 检查准确的分析资源：有效性、`m²/s` 单位的 D、标准误、有效点数、R²、残差、拒绝行和来源，再生成原生图。独立计算并检查单指数或 ILT DOSY，核对接受／拒绝列、算法设置、输入指纹及与区域拟合的数值一致性；ILT 使用求解诊断，不使用单指数置信度。
5. 为 DOSY 结果字段建图，检查资源身份与状态、几何诊断及两张 PNG 的坐标轴、曲线、等高线、残差和标签可读性，明确报告科学或图像问题。
