---
title: MCP 数据导入与表格
description: 通过 MCP 导入数据、读取表格、派生和修改表格。
---

## 从欢迎页导入原生数据

只打开 Plotium 即可调用 `plotium_get_state {}`；后台工作、恢复或保存阻塞项目访问时也可查询。
响应包含 `availability` 中的 `project_read`、`project_modify`、
`native_import`，各项包含 `available` 和可为空的 `{code,message}` 原因。
可直接从欢迎页导入原生数据或表格。空闲时 `project` 返回数据集和画布数量，即使尚未导入数据；
忙碌时 `project` 与 `selection` 均为 `null`。规划操作前请检查 `availability`。
查询不会新增文档审计记录或未保存修改。
所有状态下均可通过 `plotium_capabilities_get {}` 发现连接能力。

启用 **读取并修改**，从状态复制 `project_context` 和 `revision`，再调用 `plotium_plan`：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 0,
  "tool_id": "data.import",
  "parameters": {"paths": ["C:/Data/experiment.jdf"]}
}
```

请使用实际绝对路径。原生导入接受桌面加载器支持的明确数据文件及原子采集目录，
例如 Bruker 采集或处理数据目录；可读取采集必需的伴随文件，不递归批量发现或扫描相邻实验。
此操作不支持 ZIP 归档、项目文件或新格式。可选 `sampling_declaration` 沿用核心导入约定。
完整共享 schema 可用
`plotium_capabilities_get {"section":"operation_constraints","key":"data.import"}` 读取。

预检在后台完成，`import.items[].metadata` 返回各输入的格式、维度、警告、失败原因和拟创建的
数据集／画布 ID。预检不创建资源或修改文档。调用 `plotium_execute {"plan_id":"PLAN_ID"}`
提交冻结数据；预检后修改或删除源文件不影响执行。成功后向当前项目添加原生数据集和默认画布，
保留项目上下文和当前选择。

结果为 `succeeded`、`partial` 或 `failed`；所有成功项一次原子提交、一次撤销。
撤销首批内容后移除导入的内容；重做恢复相同资源 ID。全部输入失败或导入取消时，
不新增资源、撤销记录或未保存修改。
成功导入仍保留科学溯源记录。
提交时重新检查权限、上下文和版本；即使执行被拒绝，计划也已消耗，不能重复使用。
遇到 `project_changed` 后重新查询状态并预检。

每批最多 16 个输入、1 个预检在途、最多 4 个保留的 prepared import（与表格导入共享）。
所有保留计划及在途预检的原生冻结负载保守预留合计最多 512 MiB；超限返回
`import_budget_exceeded`。超过 16 个路径时调用失败，报错写明收到的数量；其余文件请分批
导入。预检响应最多 256 KiB。科学处理与 DOSY 使用独立的共享科学操作。
CSV、TSV 和 XLSX 文件是表格，用 `mode: "table_data_only"` 导入；原生导入收到它们会报错并给出这一提示。

## 导入 CSV/TSV/XLSX 数据，不创建图

让 agent 导入明确指定的 UTF-8 CSV/TSV 或 XLSX 工作簿，检查列信息，再分页读完整张表。先调用
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
`format: "csv" | "tsv"` 指定。`.xlsx` 文件（或 `format: "xlsx"`）把每个有内容的可见
工作表各导入成一张表，表名取工作表名，工作簿只有一张表时取文件名，与桌面端导入一致。
`sheets: ["Figure 3", …]` 只导入列出的工作表；名字不存在时该文件失败，并列出可见工作表。
按 Source Data 方式排布的工作表会按[导入数据](/zh-cn/guides/importing-data/#source-data-工作表)
中的规则拆成多张表，每项会报告其 `range`。某张表识别不对时，用
`regions: [{"sheet": "Figure 3", "range": "J2:AC4411", "header_rows": 1}]` 重新导入。
每个区域成为一张表，名称取 `name`，否则取工作表名加范围；`header_rows` 从区域内第一个
非空行起算，默认为第一个数值之上的行数；`split_segments: false` 让上下拼接的段保留在
同一列。给了 `regions` 时，只导入这些区域和 `sheets` 中列出的工作表。每个工作表单独
准备：没有表格或超出限制的工作表、没有单元格的区域都作为单独一项失败，其余照常导入。
超过 256 列的表会失败，并提示改用更窄的区域。一个文件
最多读取 8,000,000 个工作表单元格，按各工作表的已用范围计算。`header`、`schema` 和
`schema_path` 只用于 CSV/TSV。含 XLSX 的批次摘要超过 128 KiB 时预检失败，并按文件列出
工作表，可每次导入一个工作簿并减少 `sheets` 中的工作表。只读所列普通文件，不扫描目录、不自动寻找邻接
sidecar、不回写源文件、不创建图或 X/Y 绑定。

预检在后台完成准备后才返回。`import.items` 逐文件（XLSX 逐工作表，带 `sheet`）报告 `prepared` 或 `failed`，
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
已有表格，但不可导入。预算见 `limits` 能力分区，导入权限条件见索引。

## 派生表格而不修改源数据

可以让 Agent：“计算 `signal_x2 = y_left * 2`，排除 `x_left` 等于零的行，按 X
降序排列，再把结果绑定到这条已有曲线。”处理操作只创建一张原生表格，不创建图。
重绑使用[现有曲线绑定操作](/zh-cn/reference/mcp/plots/#精确重绑已有曲线不影响另一张图)。

1. 读取 `plotium_get_state`，再用 DatasetId 和表 revision 调用
   `plotium_table_columns`。复制实际 ColumnId、`table_id`、`table_revision`
   和 `snapshot_fingerprint`；名称和 `source_column_id` 不能作为身份。
2. 用 `plotium_capabilities_get {"section":"operation_constraints","key":"data.transform"}`
   发现原生计划格式。`ids` 恰好指定一个 DatasetId，模式为 `table_data_only`。
   叶节点必须固定该数据集的当前快照；为 `operation_id` 和每个计算列的 `id`
   提供新的 UUID。
3. 调用 `plotium_plan`，检查 `transform.status: "prepared"` 及其 `output`，再将
   返回的 `plan_id` 交给 `plotium_execute`。准备完成不代表已插入数据。成功执行返回
   `value.status: "succeeded"`、`value.output` 和一个 `produced` 数据集；提交保留
   准备阶段返回的输出身份。
4. 用 `plotium_table_read_rows` 读取完整结果，再将实际 DatasetId、FieldId、TableId、
   revision 和 X/Y ColumnId 绑定到已有曲线，用 `plotium_render_preview` 检查原生图像。若要新建图，
   则将相同输出身份交给 [`plot.create`](/zh-cn/reference/mcp/plots/#从已导入表格创建原生单图)，再预览返回的画布。
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
512 MiB 预留预算，每项按请求声明的预算计费。完整响应最多 256 KiB；准备结果摘要最多 128 KiB，为请求和计划外层预留空间。限制也可用
`plotium_capabilities_get {"section":"limits","key":"table_transform"}` 发现。预算失败返回
`transform_budget_exceeded`，取消返回 `request_cancelled`，均不留下派生表或成功执行
结果。修正输入或预算后重新规划。后续更新使用下述独立的单元格补丁及显式刷新操作。

## 补丁修改单元格，再显式刷新派生表

`table.patch` 的顶层 `ids` 恰好指定一个 DatasetId，不传 `targets`；参数须包含
当前表 revision。RowId、ColumnId 必须复制自表格读取结果，分页 offset、行号及
列名不能代替身份。例如：

```json
{
  "tool_id": "table.patch",
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 12,
  "ids": ["DATASET_UUID"],
  "parameters": {
    "table_revision": "TABLE_REVISION_UUID",
    "edits": [{
      "row_id": "RETURNED_ROW_UUID",
      "column_id": "RETURNED_COLUMN_UUID",
      "value": {"kind": "float64", "value": 2.5}
    }]
  }
}
```

值沿用读取结果的 `kind`／`value` 格式：Int64 及纳秒时间类型用十进制字符串；
Float64 用 JSON 数字或 `"NaN"`／`"+Inf"`／`"-Inf"`；缺失值为
`{"kind":"null"}`。类型及 nullable 必须严格匹配，不能用 Float64 补丁隐式扩宽
Int64 列；分类值只能指定已有 level 的索引。此操作不修改扩展类型或实时计算的区域表。

每批 1–1000 个不同单元格，参数最多 64 KiB。缺行、缺列、重复单元格、类型错误或
旧 revision 均拒绝整批，不留下部分修改。不增删行、改 schema、重导入或回写源
CSV/TSV。准备返回 `update:{status:"prepared",output,before_table_revision,changed}`，
执行前不修改项目。相同值返回 `changed:false`，不推进文档 revision 或增加 Undo。
补丁或刷新提交前，其数值必须仍满足已有曲线绑定的要求。例如，用作 Y 误差的列
不能包含负幅度。拒绝更新时，表格、图和 Undo 历史均保持不变。

表格元数据、行页和 Series 来源返回
`freshness:{status,refreshable,reason,sources,mcp_refreshable,mcp_refresh_reason}`。
`refreshable` 表示原生刷新能力；`mcp_refreshable` 另检查连接支持的配方范围，
不表示当前权限模式。数据集资源元数据分别暴露 `table_refreshable`、
`table_mcp_refreshable` 和 `table_mcp_refresh_reason`，以及 `table_freshness` 及其原因。`source` 为没有派生配方的源表，`current`
为已记录输入 revision 仍匹配，`stale` 为直接或上游输入已变化，`unavailable`
为来源或受支持配方缺失。数据浏览器将受影响的关系派生表标为 **Out of date**，
数据表窗口说明原因。旧结果仍可读取、绘制，同时明确带有失效状态。修改表会清除
该表的已存曲线拟合分析，Undo 恢复；失效派生表中的分析只描述旧结果，不代表当前源数据。

只有明确请求才刷新：以 `ids:["DERIVED_DATASET_UUID"]` 和
`parameters:{"table_revision":"CURRENT_DERIVED_REVISION"}` 规划 `table.refresh`。
仅支持已保存的单源 `snapshot_read`／`project`／`filter`／`stable_sort`／
`computed_column` 配方，表达式和复杂度限制与 data-only 处理相同；按当前源快照，
沿既有 reference 执行器和 lineage 路径刷新。其他原生配方（包括 Pivot）会被 MCP
刷新明确拒绝；原生 GUI/core 的刷新能力不变。采用候选前验证 TableId、列 schema
（含 ColumnId、类型和单位）保留，每个输出 RowId 必须属于当前输入；不匹配时
整次拒绝，不修改表、图绑定或 Undo。先刷新失效上游；缺来源或配方时说明原因，不自动重算其他数据。派生表已有人工
单元格修改按稳定 RowId rebase，身份缺失或歧义时拒绝整次刷新，不回退到行位置；
连续刷新仍保留这些修改。`include_provenance:true` 返回当前 `operation`，以及
沿编辑／rebase 历史解析的有效 `refresh_recipe`。

可选 `memory_limit_bytes` 依次采用显式值、配方已保存预算、512 MiB 默认值，范围
1–536870912；仍表示物化结果估算，不是峰值内存。补丁／处理／刷新共享一个后台
预检、四个待执行候选及 512 MiB 预留上限，每个补丁预留 64 KiB。准备摘要最多
128 KiB，完整响应最多 256 KiB；schema 和限制见 `operation_constraints` 与
`limits`（`table_update`）能力分区。

两项操作的预检和执行都要求 **Read and modify**；候选采用前重验权限、文档／表
revision 及当前输入。取消、切换项目、停止连接或输入过期丢弃未提交结果，失败执行
也消费 plan。响应丢失后先检查实际状态，不重放已提交操作。

补丁与刷新各自对应一次 Undo／Redo。DatasetId、TableId、ColumnId、已有 RowId
及原生图／Series 身份保留，筛选可改变结果包含哪些已有行。只更新 revision 和来源
映射，不改人工颜色、轴范围或面板 frame。失效状态由保存的 provenance 与当前
revision 计算，因此 Undo／Redo 同时恢复数值及对应失效状态。原生表格编辑与刷新
使用同一个 typed revision 提交服务。

## 删除数据集

`dataset.delete` 的参数为 `{}`，在 `ids` 中给出 DatasetId。如果仍有图、投影、
派生数据集或分析报告使用该数据集，预检会拒绝，并指出使用它的页面与面板、
数据集或报告；请先删除这些内容。派生数据集需单独一次请求先删除，再删除其来源。一次撤销即可按原 ID 和原位置恢复全部被删除的数据集。
