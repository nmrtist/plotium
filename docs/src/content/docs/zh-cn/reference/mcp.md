---
title: MCP 接口参考
description: Plotium 本机 MCP 客户端的手动配置、工具调用与限制。
---

连接步骤和日常使用请先阅读 [AI 客户端连接](/zh-cn/guides/automation/#本机-agent-连接mcp)。本页用于自定义客户端和工具调用层面的排查。

## 手动配置与诊断

在 **Preferences → AI connections (MCP)** 中展开 **Manual configuration and diagnostics**，
点击 **Copy stable stdio configuration**。复制的 JSON 含实际连接器绝对路径；
下例中的路径仅为占位符：

```json
{"mcpServers":{"plotium":{"command":"/ABSOLUTE/USER/DATA/bin/plotium-mcp","args":[]}}}
```

请使用页面复制的真实路径，Windows 文件名为 `plotium-mcp.exe`。启用时连接器会
复制到用户数据目录，应用升级时更新。发行安装目录中请将 `plotium-mcp` 保留在主
程序旁。官方 CLI 注册形式为：

```sh
codex mcp add plotium -- /ABSOLUTE/USER/DATA/bin/plotium-mcp
claude mcp add --transport stdio --scope user plotium -- /ABSOLUTE/USER/DATA/bin/plotium-mcp
```

参见 [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)、
[Claude Code MCP](https://code.claude.com/docs/en/mcp) 和
[MCP Bundle manifest](https://github.com/modelcontextprotocol/mcpb/blob/main/MANIFEST.md)。
临时 HTTP 诊断信息包含密钥，服务重启后失效，仅用于高级诊断，不是保存的客户端
配置。发现文件只面向当前用户和本机；不支持远程或 WSL 跨环境访问。

## 权限与支持范围

支持的修改使用 Plotium 的 **Undo** 与 **Redo**。只读立即阻止后续修改执行，包括
待执行计划；表格读取及属性预检仍可用，导入、原生曲线绑定、创建及处理预检要求读取和
修改权限。权限会记住，直到手动更改。

项目查询及计划创建须同时提交 `expected_revision` 和 `expected_project_context`，
后者取自 `plotium_get_state` 的 `project_context`；下文示例用 `PROJECT_CONTEXT`
占位。每次打开、新建、关闭或恢复项目都会更换上下文，即使 revision 相同也不可
混用。切换清空计划并拒绝旧队列工作；切换期间的 `busy` 表示需等待完成或取消。
收到 `project_changed` 后重新读取状态、发现目标并规划，不沿用旧目标或计划。
执行计划绑定创建时的上下文。

实际 agent 项目读取和执行保存运行记录，使项目进入未保存状态；握手、工具发现、
已认证状态查询、每五秒心跳及静态属性目录查询均不生成记录。

连接支持 CSV/TSV 仅数据导入、原生表格查询、画布预览、文档结构、原生单图创建、显式数值曲线绑定、受支持的表格处理和
资源查询、数据集／画布／画布对象重命名，以及以下显示属性：

| 属性 ID | 目标 |
| --- | --- |
| `series.line.stroke_width` | 图对象中的线图序列 |
| `series.contour.line_width` | 图对象中的等高线序列 |
| `series.contour.count` | 图对象中的等高线序列 |
| `document.figure.typography.tick_pt` | `document` |
| `document.figure.typography.label_pt` | `document` |

此接口暂不支持处理参数、应用偏好、其他格式导入、其他计算、导出及内置聊天。

## 不依赖名称发现结构

先用 `{}` 调用 `plotium_get_state`。其 `connection_capabilities` 描述当前连接的
十一个工具、输入 schema、结果约定、支持的操作、属性 ID、输出限制和当前执行权限。
标准 MCP `tools/list` 也提供工具输入 schema。资源描述中的能力属于 **core** 的
数据和渲染语义；出现 `figure.export` 或 `data.transform` 不代表此连接已开放该操作。
这里可预检 `resource.rename`、`properties.set`、`properties.reset`，以及下述受限的
`data.import`／`data.transform` 模式、`plot.create` 及 `plot.series.bind`／`plot.series.materialize_default`。
只读权限允许表格查询及既有属性预检，但拒绝导入／绑定／创建／处理预检和所有修改执行。

使用返回的 revision 列举文档画布：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "target": {"resource":{"id":"document","kind":"plotium.document"}},
  "view": "children",
  "offset": 0,
  "limit": 50
}
```

将此请求传给 `plotium_document_inspect`。复制完整的 `value.items[].target`
作为下一次请求目标，依次进入画布、面板、内容和 Series。画布先列面板，再列未归属
面板的内容；面板内的内容只在所属面板下列出。使用 `view: "self"` 并省略分页参数，
读取目标详情、`ancestors` 归属链和 `child_count`。内容类型包括图、文字、形状和
栅格图片，不返回图片像素或数据行。frame 使用 pt：面板相对页面，面板内内容相对
面板；画布尺寸使用 mm。

Series 发现与属性寻址一样，使用图对象的当前显示绑定。非当前通道或显示模式的
替代 Series 仍保存在项目中，但在重新激活前不列出、也不可寻址。当前绑定中由
用户隐藏的 Series 仍会列出，`visible` 为 false。切换通道或显示模式后，须读取
当前 revision，再发现当前有效目标。

Series 详情包含可直接用于属性查询的完整 target、编码、可见性、显式标签，以及
`source` 中的数据集和字段资源引用、所属数据集内的 `field_id`、可选 `item_id`、
单位和绑定状态。用返回的资源 ID 调用 `plotium_resources_inspect`，可读取字段和
列的描述。字段失效时保留 `field_id`，但不提供可解析的字段引用。来源状态包括
`resolved`、`missing_dataset`、`missing_field`、`missing_item`、
`incompatible_encoding`、`unbound` 和 `missing_column`。

表格的 `binding_mode: "table_default"` 返回共享的 `x_column` 和全部
`series_columns` Y／误差列绑定。一个逻辑 Series 可能展开为多条表格显示曲线，
这些显示曲线不会因此获得独立 Series ID。
显式来源返回 `binding_mode: "table_explicit"`、TableId、当前表 revision、
`x_column`、`y_column` 和可选 `error_column`，不把默认值误报为其来源。
`coordinate_units` 返回图坐标单位契约。
名称（包括同名曲线）仅作显示，不能代替身份。面板地址为
`CANVAS_UUID/panels/PANEL_ID`；内容与 Series 保持下文既有地址。

子项查询默认每页 50 项，允许 1–200，返回 `total`、`next_offset` 和 `truncated`。
保持**相同 revision 和 target**，按 `next_offset` 继续读取；256 KiB 结构化响应
预算可能使一页提前结束。详情或单个条目超过预算时返回 `response_too_large`。
不提供递归展开整个项目的接口。revision 变化会拒绝续页，须重读状态并重新列举。
目标失效或身份伪造返回 `invalid_parameters`，不会改用当前选择。

`plotium_properties_list {}` 只返回当前五项静态定义，包括 `schema`、`unit`
（无量纲时为空）、`scope`、`access` 和静态 `applicability`，不混入当前目标值。
用 `plotium_properties_inspect` 查询明确目标的当前值、默认值、范围和适用性。
属性是否适用与连接是否允许修改分别表达。

## 导入 CSV/TSV 数据，不创建图

让 agent 导入明确指定的 UTF-8 CSV/TSV，检查列信息，再分页读完整张表。先调用
`plotium_get_state {}`，然后调用 `plotium_plan`：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "tool_id": "data.import",
  "parameters": {
    "mode": "table_data_only",
    "paths": ["/absolute/path/measurement.csv"],
    "header": "present"
  }
}
```

导入不提供 `ids` 或 `targets`。`header` 可为默认的 `auto`、`present` 或 `absent`；
自动判断有歧义则失败。`.csv` 与 `.tsv` 分别选择逗号和制表符；可显式用
`format: "csv" | "tsv"` 指定。只读所列普通文件，不扫描目录、不自动寻找邻接
sidecar、不回写源文件、不创建图或 X/Y 绑定。

预检在后台完成准备后才返回。`import.items` 逐文件报告 `prepared` 或 `failed`，
包含拟建原生 ID、元数据和诊断，此时项目尚未新增数据集。确认结果后调用
`plotium_execute {"plan_id":"RETURNED_PLAN_ID"}`。执行使用预检冻结的字节和 typed
snapshot，不重新读取可能已变化的文件；提交前重验 revision 和权限，将全部已准备
成功项作为一次原子修改提交。`value.status` 为 `succeeded`、`partial` 或 `failed`，
须逐项查看结果和已创建 ID；失败文件不留下半成品。一次 Undo 移除本批成功项，
Redo 恢复同一 DatasetId、FieldId、ColumnId 和 RowId。既有 GUI/CLI 导入行为不变。

元数据包含 DatasetId、TableId、表 RevisionId、字段身份、稳定 ColumnId、逻辑
类型、nullable、单位、总行数及诊断。字段和列是不同的原生身份。缺失值保持 null，
混合数值/文本列保留文本；超出有符号 64 位范围的整数字样保留文本，不舍入。
不根据数值或表头拼写猜单位。`type_source` 区分 `file/user/inferred/unknown`，
`unit_source` 区分 `file/user/unknown`。

可选 `schema` 为有序原生 TableSchema，包含 typed ColumnSchema ID、逻辑类型、
nullable 和可选 UnitSpec；也可用 `schema_path` 明确指定 v1 PlotiumDelimitedSchema
JSON 文件。声明必须与表头及顺序完全对应，非法值明确失败，不回退自动推断。
支持声明 null、boolean、int64、float64、utf8、date 和 timestamp；本导入不接受
唯一性/业务键约束及不确定度关系，也不从 `_sigma` 列名赋予科研误差含义。每次
导入分配新的原生 ID，文件/用户 schema 中的 ID 仅描述来源，不寻址已有项目目标。

每批最多 16 个文件、每文件 16 MiB、指定输入合计 64 MiB，每文件最多 256 列、
1000000 行；明确指定的元数据文件最多 256 KiB。已准备项摘要上限 128 KiB，
完整预检响应上限 256 KiB，超出时缩小批次或 schema。每连接最多保留 4 个已准备导入
和总计 32 个计划，同时只运行一个导入预检。这些是输入预算，不是进程总内存
上限。取消、切换项目、撤销权限或期间发生编辑，都会阻止待完成预检成为可执行
计划。排队或运行不代表完成；失败或过期提交也会消费计划，须重新读状态并预检，
不能盲目重放创建请求。

## 有界分页读完整张表

从导入元数据或数据集资源描述符复制 DatasetId 和表 RevisionId。将它们与
`expected_revision` 传给 `plotium_table_columns` 查看元数据，再调用
`plotium_table_read_rows`：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 8,
  "dataset_id": "DATASET_UUID",
  "table_revision": "TABLE_REVISION_UUID",
  "offset": 0,
  "limit": 200
}
```

用返回的 ID 替换占位符。可选 `column_ids` 按稳定 ColumnId 投影并保持所给顺序；
省略或空数组表示 schema 顺序的全部列。行按不可变原生 snapshot 顺序读取。
结果包含 `column_ids`、`rows: [{row_id, values}]`、`total_rows`、`requested_rows`、
`returned_rows`、`next_offset` 和 `end`。每个值以 `kind` 标明类型，null 无 value。
Int64、time、timestamp 和 duration 用十进制字符串避免 JSON 舍入；date 为 Unix
纪元以来天数，后三种时间类型为纳秒。非有限浮点值明确返回 `NaN/+Inf/-Inf`
字符串，不转为 null。

保持数据集、文档 revision、表 revision 和投影不变，沿 `next_offset` 连续读取，
直到 `end: true` 且 `next_offset: null`。默认每页 200 行，最多 1000 行和 20000 个
投影单元格；完整结构化 ToolResult 上限 256 KiB。字节预算可缩短整页，但不截断
单元格；以实际数量和继续位置为准。单行超预算返回 `response_too_large`。
offset 等于总量时返回空的结束页，大于总量时失败。非法 limit、重复/不存在列及
超预算请求直接拒绝，不静默裁剪参数。

每次读取都检查两个 revision。人工编辑、Undo/Redo 会使旧文档 revision 失效，
须重新读状态并重启分页，不能混读。无效 ID 不回退当前选择。只读连接可查询
已有表格，但不可导入。预算和导入权限条件可从 `connection_capabilities` 发现。

## 派生表格而不修改源数据

可以让 Agent：“计算 `signal_x2 = y_left * 2`，排除 `x_left` 等于零的行，按 X
降序排列，再把结果绑定到这条已有曲线。”处理操作只创建一张原生表格，不创建图。
重绑使用[现有曲线绑定操作](#精确重绑已有曲线不影响另一张图)。

1. 读取 `plotium_get_state`，再用 DatasetId 和表 revision 调用
   `plotium_table_columns`。复制实际 ColumnId、`table_id`、`table_revision`
   和 `snapshot_fingerprint`；名称和 `source_column_id` 不能作为身份。
2. 从 `connection_capabilities.operation_constraints["data.transform"].schema`
   发现原生计划格式。`ids` 恰好指定一个 DatasetId，模式为 `table_data_only`。
   叶节点必须固定该数据集的当前快照；为 `operation_id` 和每个计算列的 `id`
   提供新的 UUID。
3. 调用 `plotium_plan`，检查 `transform.status: "prepared"` 及其 `output`，再将
   返回的 `plan_id` 交给 `plotium_execute`。准备完成不代表已插入数据。成功执行返回
   `value.status: "succeeded"`、`value.output` 和一个 `produced` 数据集；提交保留
   准备阶段返回的输出身份。
4. 用 `plotium_table_read_rows` 读取完整结果，再将实际 DatasetId、FieldId、TableId、
   revision 和 X/Y ColumnId 绑定到已有曲线，用 `plotium_render_preview` 检查原生图像。若要新建图，
   则将相同输出身份交给 [`plot.create`](#从已导入表格创建原生单图)，再预览返回的画布。
   数据处理与建图仍是可分别撤销的两个操作。

以下完整原生计划追加一个 Float64 列。将占位符替换为发现的身份或新 UUID。
示例以 `unit: null` 明确表示未知单位；若 `y_left` 有明确单位，此处乘以无量纲常数，
应将源列的**整个 UnitSpec**复制到输出列。

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "tool_id": "data.transform",
  "ids": ["DATASET_UUID"],
  "parameters": {
    "mode": "table_data_only",
    "name": "Scaled signal",
    "memory_limit_bytes": 16777216,
    "plan": {
      "version": 1,
      "operation_id": "NEW_OPERATION_UUID",
      "root": {
        "op": "computed_column",
        "input": {
          "op": "snapshot_read",
          "table": "TABLE_UUID",
          "revision": "TABLE_REVISION_UUID",
          "fingerprint": "SNAPSHOT_FINGERPRINT"
        },
        "column": {
          "id": "NEW_COLUMN_UUID", "name": "signal_x2",
          "logical_type": {"kind": "float64"}, "nullable": true, "unit": null
        },
        "expression": {
          "expr": "call", "function": "multiply.v1",
          "args": [
            {"expr": "cast", "input": {"expr": "column", "column": "Y_COLUMN_UUID"},
             "target": {"kind": "float64"}, "failure": "error"},
            {"expr": "literal", "value": {"type": "float64",
              "value": {"kind": "finite", "value": 2.0}}}
          ]
        }
      }
    }
  }
}
```

支持的关系节点只有 `snapshot_read`、`project`（有序 ColumnId）、`filter`（Boolean
谓词）、`stable_sort`（按优先级排列的键，含 `column`、
`direction: ascending|descending` 和 `nulls: first|last`），以及 `computed_column`
（明确列 schema 和表达式）。将已有节点放入后一个节点的 `input` 即可组合。完成上述
任务时，先用 `not.v1(eq.v1(column(X_COLUMN_UUID), literal(int64, 0)))` 包装计算节点
进行筛选，再按 X 降序，最后投影 X、Y 和新 ColumnId；精确 JSON 形式由 schema 提供。

表达式使用 `column`、`literal`、`cast` 或带版本的 `call`。可用函数是 `is_null.v1`、
`is_finite.v1`、`eq.v1`、`not.v1`、`and.v1`、`or.v1`、`add.v1`、`subtract.v1`、
`multiply.v1` 和 `divide.v1`，不接受任意函数、SQL 或脚本。算术要求 Float64 操作数，
类型转换必须明确指定；`failure: error` 拒绝整次处理，`null` 将转换失败值置为缺失。
原生 Int64 转 Float64 可能损失精度。计算列输出支持 Boolean、Int64、Float64 和 Utf8。
单位由调用方明确声明，算术不会推导单位或自动换算。

筛选只保留 true，false 和 null 都被排除。排序保留 RowId，同键按 RowId 决胜，null
位置不受升降序影响；非 null 浮点值按 `-Inf < finite < +Inf < NaN` 排列。投影列保留
原有 ID、类型和单位，计算列保留配方中的新 ID。源表数值、表格默认绑定、画布和已有
人工样式不变。

输出元数据包含固定的原生配方、backend、输入 revision／fingerprint、列 lineage
和行映射契约。稍后可用 `plotium_table_columns` 的 `include_provenance: true`
读取；在 `plotium_table_read_rows` 中加同一参数，则每行返回 `sources`：已记录的源
DatasetId、TableId、revision、fingerprint、原始 RowId 和从零开始的 `input_position`。
它解码本页对应的原生存储映射，不跟随源表最新 revision。导入行没有派生来源；其他
关系计划的来源形态在请求行来源分页时明确拒绝，不带该参数的普通读取保持既有行为。

准备工作不阻塞桌面线程。规划和执行均需 **Read and modify**；后台准备完成及实际
提交前都会重新检查权限、文档 revision 和源快照。执行提交一次即消耗 plan，即使失败
也不能重放。取消、停止连接、切换项目或输入过期会丢弃未提交结果；已经开始的原子
提交使用普通 Undo 撤销。一次成功处理对应一次 Undo，Redo 恢复相同表和列身份。

每个连接同时最多一个处理预检。参数最多 64 KiB、64 个关系／表达式节点、深度 16
（JSON 嵌套最多 48 层）。内存预算默认和上限均为 512 MiB，含义是 reference 执行器
**求值后检查的物化结果估算**，不是进程峰值内存限制。最多保留四个待提交结果，共享
512 MiB 预留预算，每项按请求声明的预算计费。完整响应最多 256 KiB；准备结果摘要最多 128 KiB，为请求和计划外层预留空间。限制也可从
`connection_capabilities.limits.table_transform` 发现。预算失败返回
`transform_budget_exceeded`，取消返回 `request_cancelled`，均不留下派生表或成功执行
结果。修正输入或预算后重新规划。刷新和源数据补丁不由本接口开放。


## 精确重绑已有曲线，不影响另一张图

两个图可以引用同一张原生表，各自选择不同的数值 X、Y 和可选对称 Y 误差列。
在 GUI 中选中图对象，展开其 Series 的 **Data → Curve columns**，选列后点击
**Apply curve binding**。表格共享的默认绑定不会改变。

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
`targets`，每项编辑已携带完整目标。`y_error_column_id` 为 ColumnId，或明确的
null，后者移除误差绑定。检查 `preflight.binding_plan.bindings` 中的来源映射和
逐行诊断，再以返回的一次性 `plan_id` 调用 `plotium_execute`。

一个默认表格 Series 可能代表多条显示曲线，须先调用
`plot.series.materialize_default`，参数为
`{"edits":[{"target":EXACT_SERIES_TARGET,
"retain_y_column_id":"EXISTING_Y_COLUMN_UUID",
"table_revision":"TABLE_REVISION_UUID"}]}`。
GUI 对应 **Expand default curves**：选择保留原 SeriesId 的 Y 列，其余已有曲线
获得新的对象内 SeriesId，预检和执行结果均返回映射。此操作只展开已有内容，不创建
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
GUI 保存重开保留原生绑定和单位；MCP 不开放保存和导出，新建图使用下文的 `plot.create`。

## 从已导入表格创建原生单图

可以要求 Agent：“导入这个 CSV，时间单位为 s，电流和误差单位为 mA，
创建一张可编辑折线图并显示对称 Y 误差。查看预览，再只把该曲线线宽改为
1.5 pt 并再次预览。”提供绝对输入路径和明确单位。完整导入、创建参数可从
`plotium_get_state` 的连接能力清单及 `plotium_plan` 工具 schema 发现。

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

示例中的每个身份（包括 FieldId）均须替换成实际返回值。`kind` 为 `line`
或 `scatter`，误差列必须明确传 ColumnId 或 `null`。多条 `series` 表示
同一图对象内相互独立的原生曲线，不是多张图。全行提取、数值、缺失值、
误差和单位规则与上文显式曲线绑定相同；不修改表格共享默认 X/Y、不影响
其他图，也不复制源表。

完整 `parent` 决定新建范围：

- document 根：新建画布、单面板和图对象。可选
  `canvas: {name, size_mm: [宽, 高]}`，每边为 10–1000 mm；省略时在预检中
  冻结原生默认尺寸，画布名称使用图对象名称。
- 没有面板或图对象的已有画布：按 GUI 共用的原生空画布放置服务及已有
  布局设置、边距创建面板和图对象。不传 `canvas`。
- 该画布唯一的空、可见、未锁定且自由布局的面板：仅创建图对象，保留
  面板的页坐标和尺寸。不传 `canvas` 或 `panel_name`。

`panel_name` 仅用于新建面板，省略时使用图对象名称。名称不得空白，最长
1024 UTF-8 字节。本操作不向已有成图添加第二面板/图对象，也不重建已有
内容。初始放置由原生服务决定，接口不要求 Agent 指定内部网格、留白或
避让算法。

`preflight.binding_plan.creation` 返回拟建完整目标 `canvas`、`panel`、
`plot`、`created`，以及 `size_mm`、`panel_frame_pt`（页坐标）、`frame_pt`
（面板内内容局部坐标）、`coordinate_units` 和 `series_defaults`（图型及
pt 线宽）。`binding_plan.bindings` 返回每条 owner+SeriesId 完整目标、实际
来源映射/表 revision 和全行诊断。预检不创建对象；执行一次性 plan 后，
`ToolResult.value` 返回同一身份和绑定摘要，结果的 `after_revision` 表示
提交后的文档 revision。

创建是原子的：来源错误、单位不兼容、非法误差或父目标/revision 变化时，
不留下新画布、面板或半成品图。只读连接拒绝创建预检和执行。每图 1–200
条曲线、最多 2,000,000 曲线行访问；待执行创建和绑定计划共享 2,000,000
行预算，每连接最多 32 个计划，结构化响应上限 256 KiB。失败提交也消费
plan。执行响应丢失后，先查询当前状态及预检返回的目标，不能盲目重新创建。

使用返回目标进行结构发现与属性操作，以当前 revision 预览完整画布。
结果可在 GUI 选择和继续编辑。一次 Undo 只移除本次新建子树，保留已有父
容器及导入数据；Redo 恢复同一身份。后续原生尺寸和布局调整保留图对象、
Series 身份和来源绑定。编辑或 Undo/Redo 后先读取最新 revision。

## 读取实际画布图像

可以请 agent：“预览这张画布，描述文字、曲线和本地图片。”先通过
`plotium_document_inspect` 取得完整画布 target，再以当前 revision 调用
`plotium_render_preview`：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "target": {"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
  "max_edge": 1440
}
```

将 `CANVAS_UUID` 换成发现返回的画布 ID；不接受面板、内容、当前选择或 Series。
结果包含标准 MCP PNG **image 内容块**，以及 `ToolResult.value` 中的
`target`、`render_revision`、`width_px`、`height_px` 和 `mime_type`。
无需打开文件或自行渲染 SVG 文本。MCP 客户端须将图像内容交给 agent；纯文本
客户端无法读取此预览。

图像覆盖整页，使用原生版式、背景、面板裁剪、文字、曲线及项目内嵌栅格资产。
渲染基于不可变快照在后台执行，`render_revision` 标识该快照；截取后发生的人工
修改不会被标成图像内容。执行后续修改前请重新读取状态。只读连接也允许预览。
完成的读取保存仅含元数据的运行记录，使项目变为未保存，但不推进 revision。

`max_edge` 默认 1440 px，可取 1–2048；保持宽高比，按像素取整，每轴至少一个
像素。预算为输出 4,194,304 像素、引用资产编码数据合计 16 MiB、按实际文件头
探测的资产页 RGBA8 合计 64 MiB、中间 SVG 8 MiB、PNG 4 MiB、JSON-RPC 预览响应
6 MiB（包含请求 ID）。同一资产编码数据只计一次，同一资产页解码只计一次。
这些限制约束负载和图像缓冲区，不代表整个进程的内存上限。所有引用资产均校验，包括隐藏项。
发现信息 `connection_capabilities.limits.image_preview` 也提供这些预算。

资产缺失、损坏或图片页不可用时返回 `preview_asset_unavailable`，不会以占位图
或缺图结果报告成功。超过预算返回 `preview_budget_exceeded`，不静默降质；输出
超限时降低 `max_edge`，资产超限时缩小源资产。无效完整身份返回
`invalid_parameters`，过期 revision 返回 `stale_revision`。
每连接最多同时渲染一张预览，重复请求返回 `busy`；渲染期间其他已有工具仍可用。
取消请求或停止连接后不交付图像；停止连接、切换项目会丢弃旧任务。

## 查询并修改一条明确曲线

示例任务：“找到我的线图，读取线宽，预检改为 1.5 pt，再执行保存的计划。”
工具调用顺序如下：

1. 用 `{}` 调用 `plotium_get_state`，取得 `revision`、项目概况、选择和连接权限。
2. `plotium_resources_search` 接受 `expected_project_context`、`expected_revision` 和 `query`，例如
   `{"kinds":["plotium.canvas.object"],"name_contains":"Plot"}`。先将结果解析为
   显式稳定 ID；`plotium_resources_inspect` 用 `ids` 和 `expected_revision` 读取描述。
3. 用 `{}` 调用 `plotium_properties_list` 列出支持的属性及范围。
   使用 `plotium_properties_inspect` 查询曲线地址：

   ```json
   {"ids":["CANVAS_UUID/OBJECT_ID"],"key":"series.line.stroke_width","expected_project_context":"PROJECT_CONTEXT","expected_revision":7}
   ```

   返回的 `value.readings` 为每条适用曲线提供一行。复制所选曲线的完整
   `value.readings[0].target`，例如：

   ```json
   {
     "resource": {
       "id":"CANVAS_UUID/OBJECT_ID",
       "kind":"plotium.canvas.object",
       "parent_id":"CANVAS_UUID",
       "local_id":"OBJECT_ID"
     },
     "component":{"kind":"series","id":0}
   }
   ```

4. 将该 target 原样传入 `plotium_plan`，仅修改这一条曲线：

   ```json
   {
     "tool_id":"properties.set",
     "parameters":{"key":"series.line.stroke_width","value":1.5},
     "targets":[{
       "resource":{
         "id":"CANVAS_UUID/OBJECT_ID",
         "kind":"plotium.canvas.object",
         "parent_id":"CANVAS_UUID",
         "local_id":"OBJECT_ID"
       },
       "component":{"kind":"series","id":0}
     }],
     "expected_project_context": "PROJECT_CONTEXT",
     "expected_revision":7
   }
   ```

5. 查看 `preflight` 后，仅以 `{"plan_id":"RETURNED_PLAN_ID"}` 调用
   `plotium_execute`。桌面立即显示修改；**Undo** 恢复原值，**Redo** 重新应用。
   相同值报告为跳过；整批修改合并为一次撤销。

属性查询和属性编辑预检必须且只能提供一个非空的 `ids` 或 `targets` 数组。
`ids` 将每个资源展开为该属性对应的所有组件；`targets` 只操作指定的 Series 组件。
Series ID 仅在所属图对象内唯一，因此务必复制包含资源信息的完整 target。
重复目标仅处理一次；组件不存在或资源身份被伪造时整次请求拒绝；曲线存在但属性
不适用时报告为跳过。查询会返回当前值、默认值、范围及可用性；只查询一条曲线时，
也可将查询参数改为 `targets`。恢复默认值时，用同一 target 预检 `properties.reset`，
参数为 `{"key":"series.line.stroke_width"}`。组件寻址仅支持所列五项显示属性，
不开放处理参数。`resource.rename` 仍须使用 `ids`。

工具结果同时提供结构化内容及 JSON 文本。项目读取与预检要求当前
`expected_revision`；修改或撤销后重新获取状态。执行时重新检查保存的 revision
和当前权限。计划仅可提交一次，失败后也需重新预检；最多保留 32 个待执行计划，
重启连接会清空计划。桌面开始执行前已取消的请求不会修改项目；已进入原子
提交的修改无法通过取消回滚，请使用 **Undo**。

稳定错误码包括 `project_changed`、`no_project`、`unavailable`、`connection_lost`、`permission_denied`、`expired_plan`、`stale_revision`、`busy`、
`invalid_parameters`、`unsupported_operation`、`plan_limit`、
`request_cancelled`、`connection_stopped`、`response_too_large` 和 `execution_failed`。`busy` 可在编辑手势、
恢复确认或后台项目操作结束后重试；过期计划及 revision 冲突需重新读取和预检。
AI connections (MCP) 设置页面显示最近操作和错误，后台反馈不会自行打开设置页面。
