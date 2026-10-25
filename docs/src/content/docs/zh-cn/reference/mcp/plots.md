---
title: MCP 建图与曲线
description: 通过 MCP 创建原生图，添加或重绑曲线。
---

## 从已导入表格创建原生单图

可以要求 Agent：“导入这个 CSV，时间单位为 s，电流和误差单位为 mA，
创建一张可编辑折线图并显示对称 Y 误差。查看预览，再只把该曲线线宽改为
1.5 pt 并再次预览。”提供绝对输入路径和明确单位。完整导入、创建参数可从
`plotium_capabilities_get` 的 `operation_constraints` 分区发现。

仅数据导入后，从实际导入结果或列元数据读取 DatasetId、FieldId、TableId、
表 revision 和 ColumnId。通过 `plotium_plan` 规划 `plot.create`，不传顶层
`ids` 或 `targets`：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 1,
  "tool_id": "plot.create",
  "parameters": {
    "parent": {"resource": {"id": "document", "kind": "plotium.document"}},
    "name": "Current",
    "series": [{
      "kind": "line",
      "source": {
        "dataset_id": "RETURNED_DATASET_UUID",
        "field_id": 0,
        "table_id": "RETURNED_TABLE_UUID",
        "table_revision": "RETURNED_TABLE_REVISION_UUID",
        "x_column_id": "RETURNED_TIME_COLUMN_UUID",
        "y_column_id": "RETURNED_CURRENT_COLUMN_UUID",
        "y_error_column_id": "RETURNED_ERROR_COLUMN_UUID"
      }
    }]
  }
}
```

示例中的每个身份（包括 FieldId）均须替换成实际返回值。`kind` 为 `line`、
`scatter`、`bar`、`box` 或 `violin`；`y_error_column_id` 为 ColumnId、`null`，或省略表示没有误差棒。
source 也可以按列名写：`{"dataset_id":"DATASET_UUID","x":"time","y":"signal","y_error":"sd"}`。
`x`、`y` 和可选的 `y_error` 是精确（区分大小写）的列名或 ColumnId；Plotium 补上默认字段、
表格和当前表格 revision。列名不存在或重复时返回 `invalid_parameters`，`error.details.field`
给出路径，`error.details.candidates` 列出该表的全部列。按列名解析的每个 source 会在计划响应的
`resolved_sources` 中列出。`plot.series.append` 和 `plot.series.bind` 也接受同样的 source 写法。`bar` 曲线绘制从零起的
实心柱，可带误差线，填充色即曲线颜色，并带细描边。误差线使用描边的颜色和宽度，横帽宽度为柱宽的 45%。按印刷尺寸，柱子过小、放不下描边时（宽度不足描边宽度的 4 倍，或高度不足 2 倍，例如接近 0 的值），只画填充、不画描边，误差线保留。数值型 x 上，槽位宽度取相邻位置的常见间距，只有相邻位置更近处才变窄。其 x 列可为数值列或 Utf8 文本列；文本值按
表格行顺序成为类别，相同文本共用一个槽位。共用同一槽位的柱状曲线在槽位内
并排分组；槽位内只有一根柱时占满整个宽度，因此每个类别一条柱状曲线即可得到居中、
各自配色的柱。`box`（箱线图）和 `violin`（小提琴图）曲线汇总每个 x 上的原始 y 样本
（箱线图：中位数、四分位距及 1.5 倍四分位距的须，并标出离群点；小提琴图：高斯核密度，
内含四分位条），不接受 y 误差列，与柱子共用槽位。`line` 和 `scatter` 曲线也可以用文本
x，叠加在槽位上：第 k 条折线或散点曲线对齐第 k 条柱状、箱线或小提琴曲线的位置，
因此原始数据点会落在对应的柱上。`series.points.spread` 让同一 x 上的重复数据点在该
柱宽度内按行顺序散开（0–1，占柱宽的比例）；`series.value_labels` 在每根柱或每个点的
末端（或误差线末端）之外用刻度字号写出数值。`object.chart.orientation` 设为
`horizontal` 时把显式数据图横过来：类别沿纵轴排列、第一个在最上方，数值沿横轴；误差线、
数值标签和轴标题随之转向，视图重置。同一图内所有曲线须同为文本 x 或同为数值 x。含柱状、
箱线或小提琴曲线的图不支持对数坐标轴。折线或散点曲线可选 `y_axis`：`left`（默认）或
`right`；柱状、箱线或小提琴曲线设为 `right` 会被拒绝。只要有曲线在右侧，图就会有一条右
y 轴，拥有自己的自动范围、刻度类型和标题，标题初始为第一条右轴曲线的列名和单位，该曲线
也决定右轴的单位。参考线、标注和箭头仍按左轴坐标。多条 `series` 表示
同一图对象内相互独立的原生曲线，不是多张图。全行提取、数值、缺失值、
误差和单位规则与下文显式曲线绑定相同；不修改表格共享默认 X/Y、不影响
其他图，也不复制源表。

可选的 `intent: {purpose, claim}` 记录这张图要表达什么。`purpose` 取 `compare`
（比较各组大小）、`trend`（随连续变量的趋势）、`distribution`（分布或离散程度）、
`relationship`（两个量的关系）、`composition`（组成占比）、`change`（变化、比值或倍数差）
或 `detail`（放大或插图）；`claim` 是一句话，最多 500 个字符。建图前先请用户选目的、
写 claim，不要自己猜；用户没填时，给出你的理解，让用户确认或修改。意图不会改变图的画法。
它随项目保存，之后通过 `object.intent.purpose`（设为 `unspecified` 即清除）和
`object.intent.claim` 修改，也显示在对象检查器 **Chart** 区的 **Advanced** 中。

完整 `parent` 决定新建范围：

- document 根：新建画布、单面板和图对象。可选
  `canvas: {name, size_mm: [宽, 高]}`，每边为 10–1000 mm；或用
  `canvas: {name, size_preset}` 选内置页面尺寸，例如 `nature-1col`（89 mm）、
  `nature-1p5col`（136 mm）、`nature-2col`（183 mm），并采用该预设的默认高度和
  边距（期刊栏宽 0 mm、纸张 15 mm、幻灯片 10 mm），结果中返回所用 `size_preset`；省略时在预检中冻结原生默认尺寸，画布名称使用图对象名称。
- 没有面板或图对象的已有画布：按 GUI 共用的原生空画布放置服务及已有
  布局设置、边距创建面板和图对象。不传 `canvas`。
- 已有内容的画布：明确提供 `panel_frame_pt: [x,y,宽,高]`，以页面 pt 坐标
  创建另一面板及图对象。已有面板、曲线和 loose 内容原位保留，允许重叠。
- 该画布唯一的空、可见、未锁定且自由布局的面板：仅创建图对象，保留
  面板的页坐标和尺寸。不传 `canvas`、`panel_name` 或 `panel_frame_pt`。

`panel_name` 仅用于新建面板，省略时使用图对象名称。名称不得空白，最长
1024 UTF-8 字节。`panel_frame_pt` 只设置新建面板：X/Y 为有限页面 pt，宽高
为有限正数。省略时保留原有空画布原生放置约定；已有成图的画布必须提供
明确 frame。不重排已有内容，也不将已有图重建为替代对象。

MCP 计划返回 `plan_id`、`project_context` 和 `basis_revision`。
`preflight` 包含 `required_authority`、`created_target_count`、异常 `targets` 和 `outputs`。
`preflight.binding_plan.creation` 包含精确 `parent`、拟建完整目标 `canvas`、`panel`、
`plot` 和 `created`（含全部 Series），以及 `size_mm`、所用 `size_preset`（如有）、
`panel_frame_pt`（页坐标）、`frame_pt`（面板内局部坐标）、`coordinate_units` 和
`series_defaults`（图型及 pt 线宽）。结果不确定时保留这些冻结身份用于核查。

`preflight.binding_plan.binding_count` 统计曲线数，`row_counts` 汇总各曲线的
`total_rows` 和 `plotted_rows`。`binding_plan.bindings` 只列出数据缺失、非有限值或
缺失误差的曲线，包含完整目标和全部诊断（含 `affected_rows` 样例）。空列表表示
没有这些问题，不表示没有绑定。响应省略请求回显、冻结目标重复信息、来源映射和
成功目标说明；存储的权威计划仍保留完整已验证来源及候选。

预检不创建对象。执行一次性计划后，`value.creation` 返回同一 canvas/panel/plot 和
布局；`value.bindings` 返回**每条** Series 的完整目标及 `total_rows`、`plotted_rows`、
`missing_xy`、`non_finite_xy` 和 `missing_error`。默认值仅在预检披露一次。
`produced` 列出新建资源，`summary` 统计目标结果，`targets` 只保留异常项；诊断与验证
继续保留。`project_context` 和 `after_revision` 标识已提交结果。

创建是原子的：来源错误、单位不兼容、非法误差或父目标/revision 变化时，
不留下新画布、面板或半成品图。只读连接拒绝创建预检和执行。每图 1–200
条曲线、最多 2,000,000 曲线行访问；待执行创建和绑定计划共享 2,000,000
行预算，每连接最多 32 个计划，结构化响应上限 256 KiB。失败提交也消费
plan。执行响应丢失后，先查询当前状态及预检返回的目标，不能盲目重新创建。

使用返回目标进行结构发现与属性操作，以当前 revision 预览完整画布。
结果可在 GUI 选择和继续编辑。一次 Undo 只移除本次新建子树，保留已有父
容器及导入数据；Redo 恢复同一身份。后续原生尺寸和布局调整保留图对象、
Series 身份和来源绑定。已知目标可复用同步提交结果的 context 与 revision；桌面编辑或 Undo/Redo 后先读取状态。

## 把派生曲线加入已有图

新计算的表需要与原曲线同图显示时，使用 `plot.series.append`，无需替换图对象。

MCP 不传顶层 `ids`/`targets`，在参数内指定完整图对象目标：

```json
{
  "expected_project_context": "<当前 context>",
  "expected_revision": 23,
  "tool_id": "plot.series.append",
  "parameters": {
    "plot": {"resource": {"id": "<canvas-id>/<content-id>", "kind": "plotium.canvas.object", "parent_id": "<canvas-id>", "local_id": "<content-id>"}},
    "series": [{"kind": "line", "source": {
      "dataset_id": "<派生 DatasetId>", "field_id": 0,
      "table_id": "<派生 TableId>", "table_revision": "<派生表 revision>",
      "x_column_id": "<X ColumnId>", "y_column_id": "<signal_x2 ColumnId>",
      "y_error_column_id": null
    }}]
  }
}
```

目标、FieldId、表和列 ID 均从实际发现结果复制，名称不是身份。目标必须已有
显式坐标；每批追加 1–200 条明确选择的折线/散点/柱状/箱线/小提琴曲线，追加后该图最多 200 条曲线；已有柱状曲线会与同槽位新追加的柱重新并排分组。
共享绑定验证器在一次原子提交前检查全部源 revision、完整曲线数据、误差和单位。
兼容单位转换到图的现有坐标单位；不兼容的整批失败，不部分添加。沿用绑定服务的
2,000,000 曲线行及待执行计划预算和 256 KiB 响应限制，包含受影响图的已有曲线。

`preflight.binding_plan.appended` 返回拟新增的 Series 目标，`bindings` 同时包含
未改变的已有绑定及新增绑定。执行采用预检冻结的 ID；一次 Undo 仅移除新增曲线，
Redo 恢复同一身份。Undo 后另行追加不会复用已退役 ID。已有 Series 样式、坐标含义、
人工轴范围、plot/Panel 身份及 frame 均不改变。预检和执行都要求 Read and modify；
context、revision 和权限沿用绑定编辑的一次性计划生命周期。响应丢失后先查拟新增
目标，不盲目再次追加。

## 精确重绑已有曲线，不影响另一张图

两个图可以引用同一张原生表，各自选择不同的数值 X、Y 和可选对称 Y 误差列。
表格共享的默认绑定不会改变。

MCP 先通过 `plotium_document_inspect` 取得完整 Series target，再从
`plotium_table_columns` 复制 DatasetId、TableId、FieldId、表 revision 和 ColumnId。
使用项目 ColumnId，不使用 `source_column_id`、名称或显示 trace 的位置，也不猜字段
资源地址。调用 `plotium_plan`：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 8,
  "tool_id": "plot.series.bind",
  "parameters": {
    "edits": [{
      "target": {
        "resource": {"id":"CANVAS_UUID/OBJECT_ID","kind":"plotium.canvas.object",
          "parent_id":"CANVAS_UUID","local_id":"OBJECT_ID"},
        "component": {"kind":"series","id":0}
      },
      "source": {
        "dataset_id":"DATASET_UUID", "field_id":0,
        "table_id":"TABLE_UUID", "table_revision":"TABLE_REVISION_UUID",
        "x_column_id":"X_COLUMN_UUID", "y_column_id":"Y_COLUMN_UUID",
        "y_error_column_id":null
      }
    }]
  }
}
```

将占位符和示例中的数值 ID 全部换成实际发现结果。不要另传顶层 `ids` 或
`targets`，每项编辑已携带完整目标。`y_error_column_id` 为 ColumnId，或
null／省略，后两者移除误差绑定。检查 `preflight.binding_plan.bindings` 中的来源映射和
逐行诊断，再以返回的一次性 `plan_id` 调用 `plotium_execute`。

一个默认表格 Series 可能代表多条显示曲线，须先调用
`plot.series.materialize_default`，参数为
`{"edits":[{"target":EXACT_SERIES_TARGET,
"retain_y_column_id":"EXISTING_Y_COLUMN_UUID",
"table_revision":"TABLE_REVISION_UUID"}]}`。
保留的 Y 列沿用原 SeriesId，其余已有曲线获得新的对象内 SeriesId，预检和执行结果均返回映射。此操作只展开已有内容，不创建
图对象。拟合叠加及归一化／偏移的多曲线默认组合无法无损展开，因而明确拒绝；
未展开的旧默认绘图入口仍可使用。

只接受 Int64 和 Float64 列；无法在 f64 图坐标中精确表达的整数明确拒绝。
X/Y 为 null 或非有限值时形成断点，不补零、不跨断点连线。误差为 null 时保留
有效点但不画该点误差线；零误差合法。负值或非有限误差使整批失败，即使该行的
X/Y 缺失也不忽略非法误差。误差表示非负绝对幅度，不从列名赋予标准差、标准误或
置信区间含义。诊断统计全部行，每条曲线最多附 20 个受影响 RowId；绑定校验和
原生曲线提取不抽样行。

兼容单位换算到图对象持久化的坐标单位；误差幅度仅使用比例，不应用仿射偏移。
已知单位与未知单位不能混用。重绑、删除首条曲线和重排后，手动范围及位移保持
原坐标含义；不兼容时拒绝，而不是重新解释原数字。人工标签、颜色、线／散点编码、
可见性、面板位置及轴设置随原目标保留。显式图保持线／散点编码，不切换到无关
表格图型。

每批允许 1–200 项编辑，受影响图合计最多 2,000,000 次曲线行访问；待执行 MCP
曲线计划共享同样的行预算，完整响应最多 256 KiB。全部目标、列、单位、行和
revision 检查通过后才一次原子提交、一次 Undo，不部分成功、不跳过无效目标。
只读连接拒绝绑定预检和执行。失败或过期执行也消费计划，须重读状态并重新规划。
GUI 保存重开保留原生绑定和单位；MCP `project.save` 需桌面单独批准；SVG/PDF/PNG 导出需另行桌面批准，新建图使用上文的 `plot.create`。
