---
title: MCP 接口参考
description: Plotium 本机 MCP 客户端的手动配置、工具调用与限制。
---

连接步骤和日常使用请先阅读 [AI 客户端连接](/zh-cn/guides/automation/#本机-agent-连接mcp)。本页用于自定义客户端和工具调用层面的排查。

## 手动配置与诊断

Windows 上，Plotium 从 `PATH` 查找独立的客户端可执行文件。对于通过 npm 安装的
Codex，会在 `PATH` 中的 npm 前缀和默认的 `%APPDATA%\npm` 前缀内查找包中的原生
可执行文件，支持 x64 和 ARM64 安装。使用自定义 npm 前缀时，请将它加入 `PATH`
并重启 Plotium，或通过 **Choose executable…** 选择包内的 `codex.exe`。
添加 Codex 客户端需要先安装 Codex CLI。

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

升级 Plotium 后，请重启 AI 客户端中的 Plotium MCP 连接，以加载更新后的工具。
如果连接器安装失败，请打开 **AI connections (MCP)** 查看错误并点击 **Retry**。

## 权限与支持范围

支持的修改使用 Plotium 的 **Undo** 与 **Redo**。只读立即阻止后续修改执行，包括
待执行计划；表格读取及属性预检仍可用，导入、原生曲线绑定、创建及处理／补丁／刷新预检要求读取和
修改权限。权限会记住，直到手动更改。

项目查询及计划创建须同时提交 `expected_revision` 和 `expected_project_context`，
后者取自 `plotium_get_state` 的 `project_context`；下文示例用 `PROJECT_CONTEXT`
占位。每次打开、新建、关闭或恢复项目都会更换上下文，即使 revision 相同也不可
混用。切换清空计划并拒绝旧队列工作；切换期间的 `busy` 表示需等待完成或取消。
收到 `project_changed` 后重新读取状态、发现目标并规划，不沿用旧目标或计划。
执行计划绑定创建时的上下文。

项目编辑、科学计算和数据变换保留持久运行溯源，使项目进入未保存状态。普通查询（状态、
资源／属性／表格／文档浏览、科学结果查看、预览及诊断）不追加项目审计记录，也不改变
dirty、edit generation 或 automation revision。纯查询工作流同样不改变项目；包含修改
操作的工作流仍保留完整运行记录，包括复现操作所需的查询输入。握手、工具发现、
已认证状态查询、每五秒心跳、静态属性目录以及保存协调（预检、提交与
`plotium_save_status`），以及导出预检／提交／状态查询均不生成审计记录。

连接支持原生采集导入、CSV/TSV 仅数据导入、原生表格查询、画布预览、图面几何诊断、文档结构、原生建图及页面／面板／文字／本地图片编辑、显式数值曲线绑定、受支持的表格处理、单元格补丁、已存配方显式刷新和
资源查询、数据集／画布／画布对象重命名、为画布套用内置主题或已保存的画布风格，以及以下显示属性：

| 属性 ID | 目标 | 取值 |
| --- | --- | --- |
| `series.line.color` | 图对象＋Series | `#RRGGBB` |
| `series.line.stroke_width` | 图对象＋Series | 0.05–10 pt |
| `series.line.style` | 图对象＋Series | `solid`, `dashed`, `dotted`, `dash_dot` |
| `series.line.marker` | 图对象＋Series | `none`, `circle`, `square`, `diamond`, `triangle_up`, `triangle_down`, `star`, `pentagon`, `hexagon`, `plus`, `cross` |
| `series.line.marker_size` | 图对象＋Series | 0.1–20 pt，直径 |
| `series.line.marker_fill / series.line.marker_edge_color` | 图对象＋Series | `#RRGGBB` |
| `series.line.marker_edge_width` | 图对象＋Series | 0–5 pt；0 表示无边框（空心时为 0.5 pt） |
| `series.line.marker_hollow` | 图对象＋Series | 布尔值 |
| `series.bar.width` | 图对象＋Series | 0.05–1，占所在槽位宽度的比例（仅柱状曲线；默认 0.8） |
| `series.bar.edge_color` | 图对象＋Series | `#RRGGBB`，柱描边颜色（仅柱状曲线；未设置时为带填充色色相的近黑色，并随曲线颜色变化） |
| `series.bar.edge_width` | 图对象＋Series | 0–5 pt，柱描边宽度；0 表示无描边（仅柱状曲线；默认 0.5 pt，主题规定了辅助线宽时用它） |
| `series.y_axis` | 图对象＋Series | `left`、`right`（折线和散点曲线；柱状曲线只能在左轴） |
| `series.points.spread` | 图对象＋Series | 0–1，重复数据点在所在柱宽度内的散开程度（散点和折线曲线；默认 0） |
| `series.value_labels` | 图对象＋Series | 布尔值；写出每根柱或每个点的数值（柱状、散点和折线曲线） |
| `object.chart.orientation` | 图对象 | `vertical`、`horizontal`（按列绑定的图） |
| `series.visible` | 图对象＋Series | 布尔值 |
| `series.label` | 图对象＋Series | 文本 |
| `object.axes.x_label / object.axes.y_label` | 图对象 | 文本 |
| `object.axes.x_show_label / object.axes.y_show_label` | 图对象 | 布尔值；显示轴标题 |
| `object.axes.x_show_tick_labels / object.axes.y_show_tick_labels` | 图对象 | 布尔值 |
| `object.axes.x_thousands_separator / object.axes.y_thousands_separator` | 图对象 | 布尔值；刻度数字按千分位分组 |
| `object.axes.frame` | 图对象 | `open`（左、下两条轴）、`box`（四边框） |
| `object.axes.x_scale / object.axes.y_scale` | 图对象 | `linear`、`log`（仅折线、散点和棒状图；手动范围须大于零） |
| `object.axes.x_display_unit / object.axes.y_display_unit` | 图对象 | 属性查询返回的兼容单位代码 |
| `object.axes.x_range / object.axes.y_range` | 图对象 | 自动或手动范围对象 |
| `object.axes.y2_label` | 图对象 | 文本；右 y 轴标题 |
| `object.axes.y2_show_tick_labels / object.axes.y2_show_label` | 图对象 | 布尔值 |
| `object.axes.y2_scale` | 图对象 | `linear`、`log` |
| `object.axes.y2_display_unit` | 图对象 | 属性查询返回的兼容单位代码 |
| `object.axes.y2_range` | 图对象 | 自动或手动范围对象 |
| `object.figure.typography.tick_pt / object.figure.typography.label_pt / object.figure.typography.legend_pt` | 图对象 | 1–72 pt |
| `object.guides.visibility` | 图对象 | `auto`, `show`, `hide` |
| `object.guides.placement` | 图对象 | `auto`, `inside`, `outside_right`, `outside_bottom`, `custom` |
| `object.guides.layout` | 图对象 | `auto`, `vertical`, `horizontal` |
| `object.guides.title` | 图对象 | 文本 |
| `object.guides.position` | 图对象 | [0,1] 内的 `[x,y]` 或 `null` |
| `series.contour.line_width` | 图对象＋Series | 0.05–10 pt |
| `series.contour.count` | 图对象＋Series | 1–256 |
| `document.figure.typography.tick_pt / document.figure.typography.label_pt` | document | 1–72 pt |
| `document.figure.typography.axis_line_pt` | document | 0.05–10 pt；坐标轴线与刻度线 |
| `document.figure.typography.tick_length_pt` | document | 0–20 pt |
| `document.figure.typography.tick_direction` | document | `out`、`in` |
| `document.figure.typography.minor_ticks` | document | 每个主刻度间隔 0–9 个子刻度；0 为不显示 |
| `document.figure.typography.typeface` | document | `arial`、`helvetica`、`times_new_roman`；作用于所有图内文字 |
| `document.figure.typography.font_weight` | document | `regular`、`bold`；作用于所有图内文字 |

`object.axes.y2_*` 属性只在图中有曲线的 `series.y_axis` 为 `right` 时适用；其他图上为不适用。

科学处理与任务接口见下文。应用偏好、新格式解析、科学结果自动建图及内置聊天暂不在此接口范围内。

### 工具 annotations 与客户端审批

每个工具都在 `tools/list` 和 `connection_capabilities.tools` 中提供 MCP
`annotations`，不随当前 **Access** 设置变化。`readOnlyHint` 表示是否改变环境；
`destructiveHint` 区分可能破坏性写入与仅追加操作；`idempotentHint` 表示相同参数的
重复调用是否产生额外影响；`openWorldHint` 表示是否与外部实体交互，包括本地输入／
输出文件。破坏性和幂等提示仅对非只读工具有意义。参见
[MCP 注解定义](https://modelcontextprotocol.io/specification/2025-11-25/schema#toolannotations)。

查询、预览和回执查看均为只读，仅访问原生项目／应用状态。诊断也是只读，但续查游标
仅可使用一次。`plotium_plan` 不是只读：每次成功调用都会保留新的可执行计划，部分
操作会读取外部数据、schema、处理方案或图片文件，并准备计算；它不提交候选结果，
也不写出保存／导出目标文件。`plotium_execute` 覆盖编辑、删除、科学任务提交及保存／
导出请求，因此整个工具保守声明为可能破坏性、非幂等、开放交互域。任务取消会改变任务
状态，属于可能破坏性写操作；对同一保留任务重复取消不会产生额外影响。

客户端可以依据策略使用可信服务端的提示来决定是否询问审批，但仍可能询问；不可信
注解不能作为授权依据。客户端审批、Plotium **Access** 与桌面文件确认是独立决定。
注解不授予权限：revision／context 检查、撤权与一次性计划规则仍然生效。每次保存／导出
请求仍保留独立桌面确认及准确的文件／覆盖范围，包括 **Read only** 模式。断连或结果
未知后绝不重放调用，即使标为幂等；应查询状态或保留回执。升级 Plotium 及其连接器后，
重新连接客户端以刷新工具目录。

## 读取状态与按需发现能力

`plotium_get_state {}` 读取最新动态状态，返回 `project_context`、`revision`、
`availability`、`permissions`、`save_state`、`capability_fingerprint`、项目数量摘要
和有上限的选择分页，不附带完整能力清单或项目蓝图。用绑定 revision 的
`plotium_document_inspect` 分页或 `plotium_resources_search` 发现精确目标，
再用 `plotium_resources_inspect` 按明确 ID 读取详情。复制返回的完整目标，不用名称或集合位置代替。

空闲时 `selection` 为 `{items,total,next_offset}`，默认最多返回 16 个稳定资源引用。
传入 `selection_offset`（默认 0）和 `selection_limit`（1–50）读取后续页，例如：

```json
{"selection_offset":16,"selection_limit":50}
```

每次调用都读取当前选择，分页不是冻结快照；GUI 选择变化不一定改变文档 revision。
不能将多页视作原子选择或把选择当作执行授权。

`hand_edits` 以 `{items,total,next_offset}` 列出用户在 Plotium 中手动设置的图形值，
每项包含 `target`、属性 `key`、`label` 和当前 `value`。传入 `hand_edits_offset` 和
`hand_edits_limit`（1–50，默认 16）读取后续页。继续编辑时请保留这些值，见
[用户手动设置的值](#用户手动设置的值)。

忙碌时 `project`、`selection` 与 `hand_edits` 为 `null`；仍可读取可用性、权限、能力指纹
和保存回执 ID。

调用 `plotium_capabilities_get {}` 获取
`{capability_fingerprint,connection_capabilities}`。指纹是完整静态发现契约按规范 JSON
对象键顺序计算的 SHA-256。按响应中的指纹缓存契约；状态中的指纹不同时重新发现。
若读取状态后、能力查询完成前状态已变化，以能力响应随附的指纹标识该契约。
项目编辑、项目切换、选择或权限变化均不改变静态能力。

静态发现提供所需权限，不返回 `currently_permitted` 标记；当前权限和可用性始终从状态读取。
执行仍检查服务端权限、上下文、revision 和一次性计划。指纹、缓存的 schema 及 annotations
不授予权限，也不允许自动重试。保存和导出仍须对每个精确请求取得新的桌面确认。
升级时一起更新桌面与连接器，并重连客户端以刷新工具目录。

## 从欢迎页导入原生数据

只打开 Plotium 即可调用 `plotium_get_state {}`；后台工作、恢复或保存阻塞项目访问时也可查询。
响应包含 `availability` 中的 `project_read`、`project_modify`、
`native_import`，各项包含 `available` 和可为空的 `{code,message}` 原因。
可直接从欢迎页导入原生数据或表格。空闲时 `project` 返回数据集和画布数量，即使尚未导入数据；
忙碌时 `project` 与 `selection` 均为 `null`。规划操作前请检查 `availability`。
查询不会新增文档审计记录或未保存修改。
所有状态下均可通过 `plotium_capabilities_get {}` 发现完整连接能力。

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
完整共享 schema 位于 `connection_capabilities.operation_constraints["data.import"].schema`。

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
CSV/TSV 的 `mode: "table_data_only"` 保持既有仅创建数据的行为和限制。

## 不依赖名称发现结构

先用 `{}` 调用 `plotium_get_state` 获取当前上下文、revision 和权限。需要静态契约时，
调用 `plotium_capabilities_get {}`；其 `connection_capabilities` 描述当前连接的工具、
输入 schema、结果约定、支持的操作、属性定义和输出限制。
标准 MCP `tools/list` 也提供工具输入 schema。资源描述中的能力属于 **core** 的
数据和渲染语义；资源列出某项能力，不代表此连接已开放该操作。
这里可预检 `project.save`、`figure.export`、`resource.rename`、`figure.apply_theme`、`properties.set`、`properties.set_many`、`properties.reset`，以及下述受限的
`data.import`／`data.transform` 模式、`table.patch`、`table.refresh`、`figure.edit`、`plot.create` 及 `plot.series.append`／`plot.series.bind`／`plot.series.materialize_default`。
只读权限允许表格查询及既有属性预检，但拒绝导入／绑定／创建／版式编辑／处理／补丁／刷新预检和所有可撤销修改执行；保存和导出使用下文各自独立的逐次桌面授权，只读也可请求。

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

`encoding` 表示原生编码家族：`line` 同时包含折线和散点；`plot.create` 的
`series_defaults[].kind` 返回创建时的具体类型。

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

`plotium_properties_list {}` 只返回当前允许的静态定义，包括 `schema`、`unit`
（无量纲时为空）、`scope`、`access` 和静态 `applicability`，不混入当前目标值。
颜色 schema 以 `"format": "#rrggbb"` 说明输入格式。单位代码 schema 列出所有已注册的
单位代码；某个坐标轴实际可用的代码以 `plotium_properties_inspect` 为该目标返回的为准，
即与其坐标单位兼容的代码。
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
`limits.table_update`。

两项操作的预检和执行都要求 **Read and modify**；候选采用前重验权限、文档／表
revision 及当前输入。取消、切换项目、停止连接或输入过期丢弃未提交结果，失败执行
也消费 plan。响应丢失后先检查实际状态，不重放已提交操作。

补丁与刷新各自对应一次 Undo／Redo。DatasetId、TableId、ColumnId、已有 RowId
及原生图／Series 身份保留，筛选可改变结果包含哪些已有行。只更新 revision 和来源
映射，不改人工颜色、轴范围或面板 frame。失效状态由保存的 provenance 与当前
revision 计算，因此 Undo／Redo 同时恢复数值及对应失效状态。原生表格编辑与刷新
使用同一个 typed revision 提交服务。

## 把派生曲线加入已有图

新计算的表需要与原曲线同图显示时，使用 `plot.series.append`，无需替换图对象。
GUI 中选择已有显式坐标图，打开 **Data → Add curve**，选择源表、X/Y 与可选的
Y 误差列，再按 **Append curve**。非显式图原有的默认字段 **Add series** 行为不变。

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
GUI 保存重开保留原生绑定和单位；MCP `project.save` 需桌面单独批准；SVG/PDF/PNG 导出需另行桌面批准，新建图使用下文的 `plot.create`。

## 从已导入表格创建原生单图

可以要求 Agent：“导入这个 CSV，时间单位为 s，电流和误差单位为 mA，
创建一张可编辑折线图并显示对称 Y 误差。查看预览，再只把该曲线线宽改为
1.5 pt 并再次预览。”提供绝对输入路径和明确单位。完整导入、创建参数可从
`plotium_capabilities_get` 的连接能力清单及 `plotium_plan` 工具 schema 发现。

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
`scatter`、`bar`、`box` 或 `violin`，误差列必须明确传 ColumnId 或 `null`。`bar` 曲线绘制从零起的
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
误差和单位规则与上文显式曲线绑定相同；不修改表格共享默认 X/Y、不影响
其他图，也不复制源表。

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

`preflight.binding_plan.creation` 返回拟建完整目标 `canvas`、`panel`、
`plot`、`created`，以及 `size_mm`、`panel_frame_pt`（页坐标）、`frame_pt`
（面板内内容局部坐标）、`coordinate_units` 和 `series_defaults`（图型及
pt 线宽）。`binding_plan.bindings` 返回每条 owner+SeriesId 完整目标、实际
来源映射/表 revision 和全行诊断。预检不创建对象；执行一次性 plan 后，
`ToolResult.value.creation` 返回同一 `canvas`、`panel`、`plot`、`size_mm`、
所用的 `size_preset`（如有）、`frame_pt`、`panel_frame_pt`、
`coordinate_units` 和 `series_defaults`，`value.bindings` 返回每条曲线的
`target` 及行数统计（`total_rows`、`plotted_rows`、`missing_xy`、
`non_finite_xy`、`missing_error`）。父目标、来源映射和受影响行 ID 样例只在
预检里给出；`ToolResult.targets` 列出全部新建目标，结果的 `after_revision`
表示提交后的文档 revision。

创建是原子的：来源错误、单位不兼容、非法误差或父目标/revision 变化时，
不留下新画布、面板或半成品图。只读连接拒绝创建预检和执行。每图 1–200
条曲线、最多 2,000,000 曲线行访问；待执行创建和绑定计划共享 2,000,000
行预算，每连接最多 32 个计划，结构化响应上限 256 KiB。失败提交也消费
plan。执行响应丢失后，先查询当前状态及预检返回的目标，不能盲目重新创建。

使用返回目标进行结构发现与属性操作，以当前 revision 预览完整画布。
结果可在 GUI 选择和继续编辑。一次 Undo 只移除本次新建子树，保留已有父
容器及导入数据；Redo 恢复同一身份。后续原生尺寸和布局调整保留图对象、
Series 身份和来源绑定。编辑或 Undo/Redo 后先读取最新 revision。

## 原子编辑页面、面板、文字与本地图片

以 `parameters: {canvas: 完整画布目标, edits: [...]}` 预检 `figure.edit`，不传顶层
`ids`／`targets`。完整带 `op` 标签的 schema 由
`connection_capabilities.operation_constraints["figure.edit"].schema` 提供。
一个批次按编辑顺序修改同一画布，合为一次 Undo／Redo：

| `op` | 必需字段及含义 |
| --- | --- |
| `page` | 至少提供 `size_mm: [宽,高]` 或 `size_preset`、`margin_mm: [上,右,下,左]`、`gutter_mm` 之一，长度均为 mm。期刊 `size_preset` 设定栏宽，已有内容的页面保持原高度；纸张和幻灯片预设同时设定宽高。未给 `margin_mm` 时页边距改为预设值（期刊 0 mm、纸张 15 mm、幻灯片 10 mm）；结果中返回 `size_preset` |
| `panel_frame` | `target: 完整面板目标`、`frame_pt: [x,y,宽,高]`，页面 pt |
| `content_frame` | `target: 完整内容目标`、完整 `frame_pt: [x,y,宽,高]`，父局部 pt；只改该内容 |
| `image_replace` | `target: 完整 raster 目标`、`path`；以冻结的本地 PNG/JPEG 仅替换资产引用 |
| `content_delete` | `target: 完整内容目标`；删除内容并保留空父 panel |
| `panel_delete` | `target: 完整 Panel 目标`；要求原始状态已为空 |
| `series_delete` | `target: 完整 Series 目标`；删除非末条显式 Series |
| `panel_label` | `target`、`text`、`visible`；写入原生手动面板标签，保留位置和字号 |
| `panel_stack` | `panel: 完整面板目标`、`shared_x: true\|false`；可选 `gap_pt`（0–144）和 `merge: [完整面板目标,...]`，二者仅限 `shared_x: true`。把面板中的图堆叠到同一条共享 x 轴上，或恢复为自由布局 |
| `align` | `panels: [完整面板目标,...]`、`edge: left\|center\|right\|top\|middle\|bottom`；至少两个面板 |
| `distribute` | `panels`、`axis: horizontal\|vertical`；至少三个面板 |
| `grid` | `panels`、`rows`、`cols`；按所给目标顺序分配原生 Frame-basis 网格格子 |
| `text_create` | `parent: 完整画布或面板目标`、`name`、`frame_pt`、`text`、`font_pt` |
| `text_set` | `target: 完整文字目标`、`text`；可选 `font_pt` |
| `image_create` | `parent`、`name`、`frame_pt`、`path: "/absolute/local/image.png"`（PNG 或 JPEG） |
| `annotation_create` | `target: 完整图目标`、`text`、`at: [x,y]`（该图的数据坐标）、`font_pt`；可选 `kind: "text"` |
| `annotation_create`（参考线） | `target: 完整图目标`、`kind: "reference_line"`、`orientation: vertical\|horizontal`、`value`；可选 `label`、`font_pt`、`line: {color, width_pt, style}` |
| `annotation_create`（箭头） | `target: 完整图目标`、`kind: "arrow"`、箭尾 `at: [x,y]`、箭头 `to: [x,y]`；可选 `head: none\|open\|filled\|double`、`line_width_pt`、`color: "#rrggbb"` |
| `annotation_set` | `target: 完整图目标`、`annotation_id`；文字标注至少提供 `text`、`at`、`font_pt`、`offset_pt`、`placement` 之一，参考线至少提供 `orientation`、`value`、`label`、`font_pt`、`line`、`placement` 之一，箭头至少提供 `at`、`to`、`head`、`line_width_pt`、`color` 之一；可选 `override_hand_edits` |
| `annotation_delete` | `target: 完整图目标`、`annotation_id` |

创建两张图后，可设置页面并排列明确选定的两个面板：

```json
{
  "tool_id": "figure.edit",
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 3,
  "parameters": {
    "canvas": {"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
    "edits": [
      {"op":"page","size_mm":[180,90],"margin_mm":[4,4,15,4],"gutter_mm":5},
      {"op":"grid","panels":[{"resource":{"id":"CANVAS_UUID/panels/PANEL_A_UUID","kind":"plotium.canvas.panel","parent_id":"CANVAS_UUID","local_id":"PANEL_A_UUID"}},{"resource":{"id":"CANVAS_UUID/panels/PANEL_B_UUID","kind":"plotium.canvas.panel","parent_id":"CANVAS_UUID","local_id":"PANEL_B_UUID"}}],"rows":1,"cols":2}
    ]
  }
}
```

将全部身份占位符换成实际发现的完整目标；名称不能代替身份。
结果为 (4,4,83.5,71) 与 (92.5,4,83.5,71) mm。frame 原点在左上，X 向右、Y 向下，
1 pt = 1/72 inch。面板 frame 复用 **Object Geometry／拖动编辑**语义：移动保留
子内容局部坐标，改尺寸按比例缩放全部子内容 frame，不改曲线身份、绑定、样式、
轴范围或字号。这不同于自由 Panel inspector 仅修改容器尺寸的行为。
新文字／图片 frame 使用父面板局部 pt；父目标为画布时则使用页面 pt。

`panel_stack` 的 `shared_x: true` 要求至少有两张图。`plot.create` 会把每张新图
放进各自的面板，因此请在 `merge` 中列出其他图所在的面板：它们会先合并到 `panel`，
其中的图按页面上的上下位置从上到下堆叠，与桌面端 **Stack with Shared X** 相同。
不传 `merge` 时保持面板内容顺序。堆叠填满面板内宽；高度
保持当前 frame 的比例，用 `content_frame` 修改某张图的高度即可调整其所占比例。
开启堆叠时间距默认为 0 pt。只有最下面的图绘制 x 刻度数字和 x 轴标题，只有
最上面的图绘制图标题，内部边界紧贴且 x 刻度朝内，各绘图区共用左右边缘。进入堆叠
时，会把覆盖所有成员数据的 x 范围（按各自坐标单位）写入每个成员，因此各成员须先
使用相同的 x 刻度类型；所有成员绘制最下面成员的 x 方向，但不保存该方向。各图共享
x 视窗；之后在任一成员上修改 x 范围、x 刻度类型、x 显示单位或 x 千位分隔符，都会
写到所有成员，并合为一次 Undo；若任一成员不能接受该值则拒绝写入，`applied` 会
列出各成员的修改。`shared_x: false` 恢复为自由布局并保留各 frame。结果在 `panel_edits` 中列出
每个操作的 `layout`、`gap_pt`、`members_top_to_bottom` 和 `merged_panels`。
`plotium_document_inspect` 为每个面板返回 `layout`（`free`、`vertical_stack`、
`horizontal_stack`、`grid` 或 `shared_x_stack`）；堆叠面板另含
`shared_x_stack: {members_top_to_bottom, gap_pt}`。`plotium_figure_diagnose`
和计划预览使用同一套堆叠几何。

图内标注按数据坐标定位，而不是页面 pt：文字居中显示在 `at` 点上方，画布和所有
导出中都跟随该图的轴范围、缩放和平移移动；锚点在可见范围外时随图一起裁切。该位置
压到数据、图例、数值标签或其他标签时，文字挪到约三个字高范围内最近的空位；指向它的
箭头不算遮挡。`offset_pt: [dx, dy]`（pt，x 向右、y 向上）把文字固定在该偏移处，
`[0, 0]` 即正好在 `at`；`placement: "auto"` 恢复自动摆放。用户在画布上拖过的标签在
修改 `text`、`at`、`font_pt` 时保持原位；对它使用 `offset_pt` 或 `placement` 会被
拒绝，除非以 `override_hand_edits: true` 表明是用户要求。`annotations` 中每个文字
标注报告 `placement`（`auto`、`fixed` 或 `hand`）和实际绘制的 `offset_pt`；
`plotium_get_state` 在 `hand_edits` 中以键 `annotation.label_position` 列出拖过的标签。
`annotation_id` 只在所属图内有效，删除后不会复用；可从结果的 `annotation_edits`
或 `plotium_document_inspect` 返回的图 `annotations` 列表读取。`at` 使用
`coordinate_units` 给出的源单位，而不是刻度标签所选的显示单位。标注文字最多 1024
UTF-8 字节且不得空白，每张图最多 256 个标注。图及其父面板须未锁定；不要求关闭
Auto height。

参考线是 `kind: "reference_line"` 的图内标注：在 x = `value` 处画竖线，或在
y = `value` 处画横线，单位与 `at` 相同，贯穿整个绘图区。它跟随轴范围、缩放和平移，
`value` 不在可见范围内时不显示。`line` 设置 `color`（`#rrggbb`）、`width_pt`
（0.1–20）和 `style`（`solid`、`dashed`、`dotted` 或 `dash_dot`）；新参考线默认
`#000000`、0.75 pt（若主题规定了辅助线宽则用它，Nature 主题为 0.3 pt）、虚线，`annotation_set` 中省略的 `line` 字段保持原值。可选
`label` 以线条颜色显示在竖线顶部附近或横线右端，那里压到数据、图例或其他标签时沿线
挪到别处或换到线的另一侧；检查结果报告 `label_placement` 和实际的
`label_position: {along, side}`，`placement: "auto"` 按同样的 `override_hand_edits`
规则把拖过的标签恢复为自动摆放；未设置 `font_pt` 时使用刻度标签
字号，一旦用 `font_pt` 设定字号便保持该值直至再次修改；`label` 为空字符串时移除
标签。参考线与文字标注共用同一 id 序列和
每张图 256 个的上限，`plotium_document_inspect` 在 `annotations` 中同时列出二者，
并以 `kind` 区分。提供另一类标注的字段会被拒绝，而不是被忽略。

```json
{"op":"annotation_create","target":EXACT_PLOT_TARGET,"kind":"reference_line",
 "orientation":"horizontal","value":0,"label":"baseline",
 "line":{"color":"#7f7f7f","width_pt":0.5,"style":"dotted"}}
```

箭头标注（`kind: "arrow"`）是从箭尾 `at` 到箭头 `to` 的直线，两端使用同一数据
坐标，因此与文字标注一样跟随坐标轴移动并裁切在绘图区内。`head` 默认为 `filled`
（`double` 两端均为实心箭头，`none` 为无箭头直线）；`line_width_pt`（0.1–20 pt）默认 1 pt（若主题规定了辅助线宽则用它，Nature 主题为
0.3 pt），`color` 默认黑色；两端不得重合。箭头不带文字，需要说明时另建一个
文字标注（例如放在箭尾）。箭头与其他标注共用该图的 `annotation_id` 序列和每张图 256 个
的上限，在 `annotations` 中以 `kind: "arrow"` 列出；提供其他类标注的字段会被拒绝。

Align 依据所选边界／中心；Distribute 在固定两端中心间等分中心位置，不等分边缘
间隙。Grid 将已有原生格子算法与 authoring 尺寸 setter 组合用于明确子集，不表示
GUI 已有子集 Grid 按钮；使用当前页面边距／gutter，不修改已保存的网格偏好或简化
轴文字。仅改页面尺寸／边距不重排内容。非目标面板和 loose 内容原位保留，即使
因此重叠或裁切，也不自动移动或缩字号。

面板目标须同属此画布、互不重复、未锁定，且为自由布局或共享 x 轴堆叠布局；新建文字和图片
要求父面板为自由布局；锁定子内容也拒绝面板编辑。
明确指定的隐藏目标仍可寻址。精确页面／布局编辑须先在 GUI 关闭 Auto height。尺寸须有限且为正，页面每边 10–1000 mm；边距／gutter
须非负并留出正空间。Grid 行列各 1–16、须容纳所有目标，且每格达到原生最小尺寸 1 pt。文字最多 16384 UTF-8
字节、字号 1–72 pt；名称／手动标签最多 1024 字节且不得空白。每批 1–200 项，
每次布局最多 200 个面板。

图片预检仅读取明确指定的绝对路径普通文件：每批最多 8 次图片输入（创建与替换合计，重复输入也计数）、每文件编码大小 1 MiB、
每批编码合计 4 MiB，每张解码 RGBA8 最多 4 MiB。PNG/JPEG 探测和解码复用原生 I/O；
预检冻结字节，提交后嵌入原生资产库，不依赖外部路径，相同编码资产复用。资产缺失、
损坏或格式不支持使整批以 `invalid_parameters` 及明确资产错误拒绝。不扫描目录、
访问网络或回写源文件。

`preflight.figure_plan` 返回拟建 `created`／`modified` 完整目标及 `result` 中的页面
尺寸、边距／gutter 和精确 `panel_frames`；执行在 `ToolResult.value` 返回该结果。
预检不提交内容；非法目标、几何、资产或旧 revision 不留下半成品。预检及执行都要求
Read and modify。失败执行也消费一次性计划；沿用 32 个计划及 256 KiB 响应限制。
一次 Undo 恢复整批，Redo 恢复同一原生内容／资产身份。执行响应丢失后先检查拟建目标，
不能盲目重复创建。可继续用普通 GUI Geometry、文字和图片 inspector 编辑，并查看
实际 PNG；原生文字 frame 过小时可能裁切，布局不自动消除冲突。

## 移动已有内容或原位换图

需要扩宽已有说明框，或只替换两份共享资产图片中的一份时，先用
`plotium_document_inspect(view:"self")` 读取该内容。复制完整目标，不用名称、选择状态
或集合位置；使用当前 context/revision 预检 `figure.edit`，例如：

```json
[
  {"op":"content_frame","target":{"resource":{"id":"CANVAS_UUID/CONTENT_ID","kind":"plotium.canvas.object","parent_id":"CANVAS_UUID","local_id":"CONTENT_ID"}},"frame_pt":[12,18,140,30]},
  {"op":"image_replace","target":{"resource":{"id":"CANVAS_UUID/IMAGE_ID","kind":"plotium.canvas.object","parent_id":"CANVAS_UUID","local_id":"IMAGE_ID"}},"path":"/absolute/local/replacement.png"}
]
```

将数组放入 `parameters.edits`，同级 `parameters.canvas` 使用发现的完整画布目标，
不传顶层 `ids/targets`。示例为两个不同内容；移动并替换同一图片时重复其完整目标。
文字内容／字号仍用 `text_set`。

`content_frame` 支持 plot、text、raster 及已有 rect/ellipse/line/arrow，不创建或
改父级。输入必须是完整 frame，不是相对增量。面板内使用父局部 pt，页直属使用页面
pt；左上原点，X 向右、Y 向下。读取新增完整 `parent`、`frame_space`、原生平移换算的
`page_frame` 和 `geometry` 限制。位置允许超出面板／页面，不自动钳制或吸附。
分量及换算边界须有限，宽高至少 1 pt；不改变 GUI 拖拽下限。

只改目标内容（在共享 x 轴堆叠中，兄弟会按新比例重排）：其余情况下父级、兄弟 frame／顺序，plot 的绑定、Series 身份、来源、单位、
手动轴范围和样式，以及文字内容／字号均保留。目标和父级须解锁，父面板须为 Free
布局，Auto height 须关闭；无关兄弟锁定不阻止编辑。`geometry.frame_editable` 和
`frame_edit_error` 返回原生限制，与连接修改权限独立。

图片 `preserve_aspect:true` 保持**现有 frame 比例**，不是替换图片的像素比例。
复用原生算法：宽／高相对缩放中与 1 偏离较大者决定等比缩放（相等时取宽），并满足
1 pt 下限；请求 X/Y 固定为左上锚点。预检明确返回可能不同的 `requested_frame_pt`
与 `effective_frame_pt`。`geometry` 同时提供 `min_size_pt`、`preserve_aspect`、
`aspect_ratio`。`fit` 独立控制框内 contain/cover/stretch。

`image_replace` 仅赋值目标 AssetId，保留 ContentId、父级、frame、z-order、名称、
可见性及全部图片属性：`page_index`、crop、fit、90°步进旋转、opacity、interpolation、
preserve_aspect。其他旧资产引用不变。保留的 crop/fit 可能使新图呈现不同，不自动清空
设置。PNG/JPEG 要求既有 `page_index=0`；非零页整批拒绝，不静默归零。换图要求目标／
父级解锁，但不要求 Free 布局或关闭 Auto height；`image.replaceable`、`replace_error`
返回适用性。

创建和替换共享上述图片预算、字节冻结、编码资产去重、嵌入及 Undo 生命周期。
预检后改动／删除源文件不影响候选或 Redo，不回写源文件、不回收资产。坏文件、不支持
格式、错误／失效目标、锁定或无效几何均拒绝整批。

`preflight.figure_plan.result.content_edits` 按输入顺序逐项提供 `edit_index`、`op`、
`target`、`parent`、`frame_space`、`before_frame_pt`、`effective_frame_pt`、`page_frame`
和 `geometry`；frame 修改另有 `requested_frame_pt`，换图另有 `asset_change`，包含
前后 AssetId（`before/after`）、`format` 和 `pixel_size`。这些值表示该编辑步骤的状态。
`result.content_frames` 返回各不同目标在整批完成后的最终几何，包含后续显式父面板缩放的影响。
执行在 `ToolResult.value` 返回同一结果。两项沿用原有权限、context/revision 屏障和
一次性计划；一次 Undo 恢复整批 frame／资产引用，Redo 恢复同一身份和冻结图片。
提交后重新读取并查看实际图像。

## 精确删除内容或指定 Series

可以要求 Agent：“删除这段重复说明和这条冗余曲线，保留另一条同名曲线。”先发现完整
canvas/content 目标及所属 plot 的 Series 目标，不用名称、下标、显示 trace 序号或 GUI 当前选中项定位。

使用限定一个 canvas、只包含删除操作的 `figure.edit` 批次：

```json
{"tool_id":"figure.edit","expected_project_context":"PROJECT_CONTEXT","expected_revision":12,"parameters":{
  "canvas":{"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
  "edits":[
    {"op":"content_delete","target":{"resource":{"id":"CANVAS_UUID/CONTENT_ID","kind":"plotium.canvas.object","parent_id":"CANVAS_UUID","local_id":"CONTENT_ID"}}},
    {"op":"series_delete","target":{"resource":{"id":"CANVAS_UUID/PLOT_ID","kind":"plotium.canvas.object","parent_id":"CANVAS_UUID","local_id":"PLOT_ID"},"component":{"kind":"series","id":2}}}
  ]
}}
```

`content_delete` 删除一份完整原生 plot、text、raster 或已有 rect/ellipse/line/arrow shape。
删 plot 不删来源 Dataset。`panel_delete` 接收完整 Panel 目标，要求预检原始状态已为空；
同批先删全部子内容不能让非空 panel 变成合法目标。删除内容保留空父 panel，不重排兄弟。

`series_delete` 只删除所属 plot 内的指定 SeriesId。支持有既定坐标合同和显式列绑定的
line/scatter plot，包括绑定派生表的 Series。拒绝删除末条 Series；移除整图须明确使用
`content_delete`。拒绝 table_default 和动态多通道显示，不自动物化、换绑或修改 Dataset。
剩余 Series 的身份、来源、顺序、encoding、坐标单位和手动范围保留，删除首条也不重新定义坐标。

重复目标、整 plot 与其 Series 重叠、跨 canvas、目标或父级锁定，以及删除和非删除操作混批
均整批拒绝。隐藏目标仍可精确寻址，锁定兄弟不阻碍无关删除。原有 Auto height 开启时保留
该设置，预检报告其原生派生高度变化，不擅自关闭。

执行前检查 `preflight.figure_plan.deleted` 和 `result.deleted_targets`。
`result.deletions` 包含父级、顺序及随整 plot 移除的全部所属 Series。
`parent_changes`、`group_changes`、`viewport_link_changes`、`selection_change` 列出必要
关系更新；分组/链接保留幸存成员，不足两个成员时解除该关系。
`remaining_series` 给出剩余顺序和 active Series 身份，`retained_asset_ids` 列出被删 raster 所引用但保留的资产，
`auto_height` 在开启时给出派生前后高度。这些是元数据，不是候选图像。
执行回执 `ToolResult.value` 返回同一删除清单。

预检不提交。一次成功执行对应一次原子 Undo/Redo，失败不半删除。Undo 恢复精确身份、父级、
对象/面板顺序、分组、链接和必要选择；ID 分配高水位不回退。不删除原文件、Dataset 或资产。
运行期 Undo 保留嵌入资产；保存沿既有可达资产策略，重开项目不恢复 Undo 历史。

沿用修改权限、project context/revision 和一次性 plan 规则。GUI 选择不推进 revision，
但若变化会改变预检列出的删除选择影响，也须重新规划。每批最多 200 个显式删除目标；
Series 候选批次最多访问 200 万剩余 curve-row；共享最多 32 个 pending plans，完整响应
不超过 256 KiB。清单超限拒绝，不截断后执行。目标或合同错误返回带原因的
`invalid_parameters`；`stale_revision`、`project_changed`、`permission_denied`、
`expired_plan`、`plan_limit`、`response_too_large` 沿用原含义。

执行结果未知时先重新发现当前 context、revision 和精确目标，不自动重放删除。
提交后查看实际 PNG，再用原生 Undo/Redo、保存重开核对结果。

## 按输出尺寸诊断图面几何

可以说：“按 180 × 90 mm 检查出界、裁切、文字溢出、这两段说明的有意重叠，以及
普通折线／散点与图例的潜在遮挡。不要修改内容；按返回 target 定位，并实际查看原生 PNG。”

发现画布后直接调用 `plotium_figure_diagnose`，无需编辑计划：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "canvas": {"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
  "output": {"size_mm":[180,90]},
  "checks": ["bounds","clipping","text_overflow","content_overlap","legend_occlusion","data_shape","layout_aesthetics"],
  "detail": "full",
  "limit": 100
}
```

可用 `targets` 限定完整的 content 或 Series target；Series target 还包含所属 plot 的 frame 和 guide 上下文。省略即扫描画布，绝不使用 GUI
选区。`content_overlap` 另需至少两个完整内容的 `overlap_targets`，比较可见且经过
页面裁剪的 frame，不判断不透明像素。`view` 可取 `issues`（默认，返回问题）、`layout`（改为返回测得的几何行）或 `both`。
`detail` 可取 `compact`（默认）或 `full`：精简的问题行保留 `target`、`component`、
`code`、`severity`、`message`、`measurement_basis` 和 `suggestions`，`evidence` 只留
`other_target`、`other_component`、`point_range` 和 `interpretation`，也不列已完成
检查的覆盖行；`full` 返回全部证据几何和所有覆盖行。报告局限的覆盖行始终返回。

`output.size_mm` 须为正数并保持画布宽高比（高度容差 0.001 mm）。与原生导出一致，
它等比缩放既有页面，不重排布局；每个初始查询重新计算几何。结果使用
`output_page_pt`：72 pt/inch，左上原点、X 向右、Y 向下。`output` 包含原生尺寸与
尺度，文字、线宽和 bounds 一起缩放。panel-local frame 经原生 page-frame 服务换算。
数据使用实际 plot 裁剪区；panel 边框**不是**所有内容的裁剪区。原生 raster frame
和 `clip_children` 的裁剪另行列明。

`ToolResult.value` 包含 `project_context`、`basis_revision`、`canvas`、`summary`、
`output`、`items`、`coverage`、`budget`、`truncated` 和 `continuation`。`summary`
统计本页的问题（`issues`、`by_code`、`by_severity`）；分页扫描时把各页计数相加。
每个问题带完整原生 `target`、所属对象内的 `component`、`code`、`severity` 和说明；
`detail: "full"` 时证据另带 bounds、clip 和交集几何。
将 `target` 直接用于 `plotium_document_inspect` 的 `view: "self"`；重名不影响定位。
`guide.legend` 属于 plot，并非新资源。曲线冲突指向现有 Series，并通过
`evidence.other_target` / `other_component` 关联所属 plot 的图例。

- `out_of_canvas`、`partially_clipped`、`fully_clipped`、`curve_primitives_clipped`／`shape_clipped`
  是信息性事实，包括有意的手动范围裁切；曲线证据只覆盖所列原生 run 的点范围。
- `text_container_overflow` 是基于原生宽度／行距**估计**的警告，不是精确字形边界；
  文字不会自动换行或缩小。
- `content_frame_overlap` 是信息性事实，允许有意重叠。
- `right_axis_untitled` 是警告：有曲线使用右 y 轴，但右轴没有显示标题。设置
  `object.axes.y2_label`（或重新打开 `object.axes.y2_show_label`）。
- `legend_occlusion_potential` 表示潜在视觉冲突，不表示数据必然不可见。逐段／逐点
  检查实际投影、有效裁剪后的线与 marker，分批保留虚线相位，不使用整条曲线包围盒，
  不抽样。线条包络与图例布局矩形是保守边界；圆角、半透明和抗锯齿可能有所不同。

`data_shape` 和 `layout_aesthetics` 检查报告 `severity` 为 `hint` 的问题：图可以表达得
更清楚，但没有错误。省略 `checks` 时包含这两项。提示从不修改文档。每条提示带
`suggestions`，即 `{action, property, target, reason}` 列表；`property` 是要在 `target`
上设置的属性，没有则两者都为 null。长度按请求的 `output.size_mm` 测量；阈值是暂定值。
限定到 Series 的查询只报告这些 Series 的提示。

| 代码 | 触发条件 |
| --- | --- |
| `small_values_invisible` | 某根柱的数值不到数值轴跨度的 1 %，或打印长度不到 0.5 mm，而旁边有至少 5 mm 长的柱 |
| `bars_too_narrow` | 某根柱打印宽度不到 0.6 mm |
| `few_samples_hidden` | 箱线、小提琴 Series，或每个 x 有多行的柱状 Series，每组都少于 10 个值，且图中没有散点 Series |
| `too_many_categories` | 类别轴显示标签，但省略了部分标签，或相邻标签相互重叠 |
| `log_scale_candidate` | 不含柱、箱线或小提琴的图，线性数值轴上的折线或散点值全为正，且跨越三个及以上数量级 |
| `bar_baseline_not_zero` | 数值轴范围不包含柱的基线 |
| `labels_overlap` | 画布上两个文字框在两个方向上都重叠超过 0.2 mm；证据给出两者和第二个的移动方向。同一图例的条目之间、同一坐标轴的刻度标签之间不互相比较 |
| `text_over_data` | 标注或数值标签压在柱、箱线、小提琴、折线或标记上 |
| `panels_misaligned` | 同一列（左边缘相差 1 mm 以内）或同一行（下边缘相差 1 mm 以内）的两个框，绘图区左边缘或下边缘相差超过 0.5 mm；或同一行／列三个及以上框之间的间距相差超过 0.5 mm |

同类提示超过 50 条，或检查用完预算时，会增加一条 `unavailable` 覆盖行。

原生 guide 布局本身使用文字宽度估计。字体可用性沿用原生导出的字体发现；实际字形
覆盖和跨后端 ink 边界仍未验证。图像视口几何使用头部尺寸、整数裁剪、旋转及 fit；
是否成功解码、透明像素分布需看原生预览。首轮不覆盖非普通折线／散点的内部几何、
误差棒裁剪／遮挡、科研派生标注及不透明像素遮挡；原生矩形、椭圆、直线和箭头的极值边界使用实际页面裁剪区；仍可观察其 frame/guide，覆盖
结果会说明限制。除上述提示外，不做审美评分、科研正确性或统计意义判断。

**没有问题时也必须读 coverage。** 各目标区分 `complete`、`unsupported`、
`unavailable`、`not_applicable`（如隐藏、空渲染数据或无图例）。整体 `partial`
表示尚有分页；`limited` 表示遍历结束但存在未支持／不可用部分。遍历完成不会把估计
变为精确测量。数量是扫描对象、源点和重叠对数，不是像素覆盖百分比。缺资产／字体、
失效来源和排除的几何会明确报告，不当作“图面无问题”。

`continuation` 非 null 时，仅用以下参数继续：

```json
{"expected_project_context":"PROJECT_CONTEXT","expected_revision":7,"cursor":"RETURNED_CURSOR"}
```

一次性 cursor 绑定原目标、输出、检查及分页配置，闲置 300 秒失效。新的初始查询
替换旧扫描。完成所有分页才可说明覆盖；问题行为空也可能截断。每个桌面连接仅保留
一个执行中 batch 和一个未读快照。原生 stdio facade 为诊断续页保留一个 transport，
stdio 结束时关闭；其他工具仍使用原来的单次调用 transport，绝不重放。限制：200 个指定目标、200 个重叠目标／19,900
对，每批 200 个对象和 1,000,000 个源点，每个 plot 最多 4,096 个渲染 run 与 polygon，1 MiB
文字、64 MiB 估算快照 payload、16 MiB 已编码资产探测。默认每页 100 行、最多 200
行，页面值 96 KiB、JSON-RPC 响应 256 KiB。这些是 payload／工作量限制，不是进程
总内存上限。快照／文字超限返回 `diagnostic_budget_exceeded`，需缩小目标；batch／
行数用尽则返回 `truncated: true` 和 cursor，不抽样。

交付前重查 context、revision、busy 屏障和连接权限。编辑、项目变化、取消、停止连接
及传输关闭会使相关工作失效；过期／已消费 cursor 返回 `expired_cursor`。
遇到 `stale_revision` 或 `project_changed`，重新读状态并开始扫描，不混合新旧页。
只读权限即可使用；对象、绑定、手动范围、几何、Undo 历史和 automation revision
以及 dirty、edit generation 不变，不追加项目审计记录。不持久化诊断几何、问题、cursor 或 PNG 副本。

按 `preview_request` 取得并实际查看原生 PNG。X 坐标转像素为
`x * width_px / output.size_pt[0]`，Y 使用对应高度；核对其
`render_revision` 与 `basis_revision` 一致。编辑器 zoom 不影响上述坐标。直接 core
`figure.diagnose` 只返回一个有界 batch，不保留 cursor；MCP 查询提供继续读取。
诊断不会自动修图。

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
预览不追加项目运行记录，也不改变 dirty、edit generation 或 automation revision。

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

## 提交前读取计划候选

`plotium_plan` 返回后，用**原** plan ID、context、revision 和一个受影响的完整画布
target 调用 `plotium_preview_plan`：

```json
{
  "plan_id": "RETURNED_PLAN_ID",
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "target": {"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
  "max_edge": 1440
}
```

响应包含实际 MCP PNG image 内容块。`ToolResult.value` 返回 `plan_id`、
`basis_project_context`、`basis_revision`、`candidate: true`、`target`、
`width_px`、`height_px`、`mime_type` 和 `output`（`size_mm`、`size_pt`、`max_edge`）。
没有 `render_revision`：候选尚未提交。读取结果的 `before_revision` 和 `after_revision`
均指向基线。实际读图后，再明确以原 `plan_id` 调用 `plotium_execute`；随后读取
`plotium_render_preview` 和 fresh `plotium_figure_diagnose`。候选预览不增加候选诊断
或自动修图循环。保留诊断的 `measurement_basis`、`coverage` 和 `unsupported`；
估计文字边界不是字形墨迹，潜在图例冲突不是已确认遮挡。

| 已有计划 | 候选支持范围 |
| --- | --- |
| `properties.set` / `properties.reset` | 上述五个 `object.guides.*` 属性及原生多目标批次 |
| `figure.edit` 局部编辑 | `content_frame`、`image_replace`、`text_set`、`annotation_create`／`annotation_set`／`annotation_delete`（文字标注与参考线）及原生批次 |
| `figure.edit` 纯删除批次 | `content_delete`、原本为空的 `panel_delete`、显式列绑定 `series_delete`；原拒绝规则不变 |
| 其他操作，包括保存和导出 | 返回 `unsupported_operation`，不改变其正常 execute 合同 |

受影响画布须使用普通折线／散点、原生文字、形状和栅格内容；预览不启动科学计算。
跨画布 guide 计划在隔离原生上下文应用完整批次，可逐个请求受影响画布，仍是一个计划、
一个 Undo 步骤。自动轴、guide 依赖和派生页高复用原生 action 与 renderer，不另建布局模型。
基线、输出设置和字体相同时，候选内容和几何与 execute 一致；不保证跨机器字体／renderer
不同情况下的像素相等。

候选读取需要 **Read and modify**；截取和后台交付时均检查 context、revision、空闲状态、
当前权限及计划仍未提交。渲染中撤权再授权也会阻止交付。预览不消费计划、不延长生命周期、
不赋予执行权限。普通编辑计划没有时钟 TTL，共享每会话 32 个计划上限，context／连接变化
会清空。execute 提交只可一次，包括失败提交。`expired_plan` 表示计划不存在或已消费；
其他拒绝由 `stale_revision`、`project_changed`、`permission_denied`、`busy`、
`request_cancelled` 及上述图像错误表达。人工编辑或切项目后须重新发现和规划；连接中断后
不要重放结果未知的 execute。

预览借用冻结输入和图像字节，不重读换图路径、不重分配已规划身份，也不在真实项目执行
再 Undo。它不改变 authored 内容、数据、绑定、选择、历史、automation revision 或持久
资产、dirty 或 edit generation。不追加项目审计记录，**不保存 PNG 字节**。无候选文件或缓存。
候选与真实预览共享单个渲染槽、取消和上述图像预算；另以 64 MiB 原生快照估计约束候选
构造（画布几何、来源字段按每元素 64 字节、元数据及冻结资产），不表示整个进程内存上限。
切项目取消的任务在有界 renderer 退出前继续占用渲染槽。

## 精确放置单图图例

Guide 属性使用 `ids:[PLOT_OBJECT_ID]`，不接受 Series 组件。按完整画布/对象身份
发现目标，不依赖唯一名称。每次用 `properties.set` 设置一个 key/value，检查计划后
再执行返回的单次 plan ID：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "tool_id": "properties.set",
  "ids": ["CANVAS_UUID/OBJECT_ID"],
  "parameters": {"key":"object.guides.position","value":[0,1]}
}
```

`position` 接受恰好两个 [0,1] 内的有限数，或 `null`。坐标表示图例框左上角在
坐标轴绘图区内可供整个图例框移动的空间中的比例，x 向右、y 向下：
`origin = plot_origin + [x,y] * max(plot_size - legend_size, 0)`。
它不是屏幕像素、整页比例或数据坐标。[0,0] 为左上角；能够容纳时，[1,1] 使
右下角对齐。输出尺寸、图例内容或布局改变时，由 renderer 重新换算。

原生属性及必要联动如下：

- `visibility`：`auto`、`show`、`hide`。Auto 在至少两个原生条目时显示普通图例，
  Show 允许单条目，Hide 隐藏 guide 而非数据。
- `placement`：`auto`、`inside`、`outside_right`、`outside_bottom`、`custom`。
  Custom 保留已有原点，没有则从 [1,0] 开始，同时设置 Inside；其他模式清除自定义原点。
- `position`：[x,y] 设置 Inside/custom。`null`、position reset、placement Auto
  和 placement reset 都清除原点及 placement override，与 GUI 双击一致。
  读取 `null` 仅表示无自定义原点；实际模式由 placement 决定，也可能位于图外。
- `layout`：`auto`、`vertical`、`horizontal`，保留原生布局规则。
- `title`：文本；空白文本沿用原生规范化，清除标题 override。

visibility/layout/title 各自只重置自己的 override。位置修改及重置保留这些字段、
字号、绑定、曲线样式和手动轴范围。同名同色等条目仍按原生规则合并，因此六条曲线
不一定形成六个图例条目。已有四项属性保留 color scale 行为；二维位置仅适用于
原生普通图例内容，不受 guide 显示开关限制；无内容时仍可读取/重置保留的原点，
但不能设置新坐标，也不独立移动 colorbar。

适用属性的查询 readings 增加 `guide_geometry`，包含
`coordinate_space:"object_local_pt"`、`scale:1`、`plot_rect`、可空的
`legend_rect`/`color_scale_rect`（x/y/width/height）、`entry_count`、可空的
`available_travel_pt`、`effective_position`、`fits_plot`（两个尺寸容纳布尔值），
以及 `not_rendered_reason`（`hidden`、`no_entries`、`auto_single_entry` 或 null）。
`measurement_basis:"renderer_layout_text_width_estimate"` 表明这是渲染使用的布局框，
文字宽度沿用原生估算，不是精确字形边界；这里不检测对象/父容器的可见性和裁切。
实际位置按可移动距离解析，不钳制图外
placement；零移动距离的轴返回零。guide 隐藏或自动省略时几何返回 null 并说明原因，
属性值仍保留配置的原点。

图例过大时不静默缩字：相应轴的可移动距离为零，`fits_plot` 为 false，实际位置
可能不同于请求原点。尺寸能容纳不代表没有裁切或遮挡曲线，仍须查看原生 PNG 及
最终导出。此信息仅观察 guide 布局，不是全图诊断。

Guide 写入在提交前验证全部显式目标；存在但不适用的目标以 `invalid_parameters`
拒绝整批，不沿用旧属性的跳过策略；旧属性行为不变。重复 ID 处理一次。每次一个属性、
最多 200 个 ID、一次 Undo/Redo；标题最多 16,384 UTF-8 字节，结构化响应最多
256 KiB（超限为 `response_too_large`），继续共享 32 个待执行计划上限。
形状错误、非有限或越界值返回 `invalid_parameters`。查询仍报告不适用目标。
只读连接可以查询及预检，execute 需要可撤销修改权限；context/revision 变化、撤权、
busy 和过期计划保留原有单次生命周期。查询不追加项目审计记录，也不改变 dirty、
edit generation、automation revision 或 Undo 历史。能力发现见 `connection_capabilities.guide_contract`。

## 精确修改显示，不重建图对象

直接使用 `creation.series_defaults[].target`，或通过 `plotium_document_inspect`
发现完整目标。所属图对象与稳定 SeriesId 必须共同传递，同名曲线仍是不同目标。
`plotium_properties_list` 列出允许的属性；`plotium_properties_inspect` 返回当前值、
默认值、目标适用的单位选项和范围输入 schema。

先调用 `plotium_get_state`，取其 `project_context` 和当前 revision，再调用
`plotium_plan`。将以下占位符和数值身份替换为实际发现结果：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "tool_id": "properties.set",
  "targets": [{
    "resource": {"id":"CANVAS_UUID/OBJECT_ID","kind":"plotium.canvas.object",
      "parent_id":"CANVAS_UUID","local_id":"OBJECT_ID"},
    "component": {"kind":"series","id":1}
  }],
  "parameters": {"key":"series.line.color","value":"#0072B2"}
}
```

检查预检后，仅以返回的 `plan_id` 调用 `plotium_execute`。
只改变该曲线及其误差线的颜色，不改变线宽或来源列。轴和单图字号使用
`ids:[PLOT_OBJECT_ID]`。

每次请求修改一个 key，可包含多个目标，原子提交并合为一次 Undo。已有但不适用
的目标报告跳过；失效或伪造身份拒绝整次请求。非法颜色、非有限数值及非法范围
在预检拒绝；执行重查 revision 和权限，失败执行也消费 plan，须重读并重新规划。

范围值明确区分：

- `{"mode":"auto"}`：仅清除该轴手动范围。
- `{"mode":"manual","min":0,"max":10,"unit":"mA"}`：在明确的兼容单位内设置
  有限且递增的范围。`unit:null` 只用于坐标单位未知的图，不能凭显示修改引入单位。
- 重置**全部**轴设置是另一个原生操作，不是 Auto 的副作用；`properties.reset`
  轴属性 reset 只清除指定 override；guide 原点/placement 按上文的联动规则重置。

显示单位接受属性查询返回的兼容代码，例如 s→ms、mA→A。图对象仍保持 R4 坐标单位契约：
原始数据、手动范围存储、位移及误差幅度的含义不变；刻度及自动派生的单位文字改变，
人工轴标题保持原文。重绑、重排和删除首条曲线不重新定义坐标。隐藏轴或分类轴不能
设置数值手动范围，但可清除已保留的范围。属性查询中的 `unit_code.codes` 返回该目标的兼容选项；
`axis_range.input_schema` 描述输入，`view` 和 `unit` 表示当前显示窗口。

符号大小以 pt **直径**表示，显式设置后独立于线宽。没有覆盖时，原生点图仍沿用
现有基于 width 的半径，原线宽接口含义不变；reset 恢复原生符号/大小。
显式空图例文字使该图例项不显示。线型仅适用于折线，散点目标会按规则跳过。
符号适用于折线和散点，不适用于 stick、柱状、箱线和小提琴曲线；柱宽和描边宽度适用于柱状、箱线和小提琴曲线，描边颜色仅适用于柱状曲线。符号填充色和边框色作用于闭合符号，未设置时跟随
曲线颜色，reset 恢复跟随。空心符号不填充并保留边框，未设置边框宽度时为 0.5 pt。颜色由图型语义决定时，单曲线改色、改符号填充色或边框色按不适用跳过。

单图刻度、轴标题和图例字号在未覆盖时继承文档设置，reset 只移除相应覆盖；
改单图字体不影响其他图。字体/标题测量沿用原生渲染，不重排页面，不改变面板及
内容的位置、尺寸。GUI 在现有 Line、Axes、Legend & scales 中使用同一属性控制，单图单位、范围及
字号位于 Advanced。
已有文档级字号仍保持项目范围。

修改后读取新 revision，再为实际画布调用 `plotium_render_preview`，查看 PNG 并
核对 `render_revision`；用 GUI Undo/Redo 检查同一原生对象。MCP 保存需单独的桌面授权；导出另需独立桌面授权。

## 查询并修改一条明确曲线

示例任务：“找到我的线图，读取线宽，预检改为 1.5 pt，再执行保存的计划。”
工具调用顺序如下：

1. 用 `{}` 调用 `plotium_get_state`，取得 `project_context`、`revision`、项目概况、选择和连接权限。
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
参数为 `{"key":"series.line.stroke_width"}`。组件寻址仅支持所列显示属性，
处理参数使用独立的 `processing.edit` 操作。`resource.rename` 仍须使用 `ids`。

工具结果同时提供结构化内容及 JSON 文本。项目读取与预检要求当前
`expected_revision`；修改或撤销后重新获取状态。执行时重新检查保存的 revision
和当前权限。计划仅可提交一次，失败后也需重新预检；最多保留 32 个待执行计划，
重启连接会清空计划。桌面开始执行前已取消的请求不会修改项目；已进入原子
提交的修改无法通过取消回滚，请使用 **Undo**。

稳定错误码包括 `project_changed`、`unavailable`、`connection_lost`、`permission_denied`、`expired_plan`、`stale_revision`、`busy`、
`invalid_parameters`、`unsupported_operation`、`unsupported_property`、`plan_limit`、
`request_cancelled`、`connection_stopped`、`response_too_large` 和 `execution_failed`。
错误提供 `error.recovery`，异步保存、导出和任务回执提供 `recovery`。
`retryable: false` 表示不要重复失败的调用，应遵循下一步动作。`busy` 执行可能已经
消费计划。revision 冲突需读取状态并审核新计划；过期计划可能已经执行，必须先核查结果。
AI connections (MCP) 设置页面显示最近操作和错误，后台反馈不会自行打开设置页面。


## 恢复动作与未知结果

`recovery` 描述本次请求，不授予执行另一次请求的权限：

| 字段 | 含义 |
| --- | --- |
| `plan_consumed` | 确知时为 `true` 或 `false`，未知时为 `null`。已消费的计划不能重用。 |
| `submission` | `submitted`、`not_submitted` 或 `unknown`；后台工作被接受不代表完成。 |
| `committed` | 确知时为 `true` 或 `false`，副作用或持久化结果不确定时为 `null`。 |
| `completion` | `pending`、`succeeded`、`failed`、`partial` 或 `unknown`。 |
| `receipt` | 状态工具及 `arguments` 中的准确 ID；`retained` 找到时为 `true`，缺失时为 `false`，未知时为 `null`。整个 `receipt` 为 `null` 表示没有已知的回执查询。 |
| `next_action` | 一个下一步：带准确参数的 `call_tool`、`desktop_file_confirmation`、`check_desktop_connection`、`inspect_outcome` 或 `none`。 |
| `replay_allowed` | 始终为 `false`，恢复信息不授权重放或自动重新提交。 |

`call_tool` 只调用指定的读取工具。桌面确认时，在 Plotium 审核待处理请求的准确路径和
覆盖范围，再查询回执。`check_desktop_connection` 要求在桌面检查 AI connections (MCP)
及访问模式，然后读取状态。`inspect_outcome` 要求对照保留的回执、预检目标／路径和
当前状态；证据不足时，重新计算科学任务或请求文件写入前需人工审核。

- **Busy：**预检在提交前失败。执行可能先消费计划，再检查 busy；结束桌面操作、读取
  状态并审核新计划，不能重用已消费的 ID。
- **Revision 冲突／项目切换／撤权：**刷新状态并在桌面检查权限。切换项目后重新发现
  新 context 中的目标；计划 ID 和文件权限不能转移到新项目。
- **安装前保存拒绝或取消：**已消费计划的回执为终态，`phase: not_installed`、
  `committed: false`、`completion: failed`。关闭确认框也表示拒绝。先检查状态并询问
  是否仍需保存；新的保存意图需新计划和新的桌面确认。
  Worker 启动后的取消必须根据最终阶段判断，不会撤销已经安装的输出。
- **保存／任务尚未完成：**被接受、排队、运行或保存中都是 `pending`，不是成功。
  查询回执，不提交替代任务或保存。保存 worker 终止后，`installed_unconfirmed`
  仍表示未知结果。
- **导出：**只有批次完成且每个文件都成功，整批才成功。混合终态为 `partial`；逐个
  核查准确路径及错误。文件可能已安装后才发生持久化错误，因此失败不证明路径未变。
  不能自动重试整批，也不能重写其中已成功的文件。
- **断连／计划过期／回执未知或淘汰：**保留计划消费、提交和持久化状态的未知性。
  重连后在同一桌面进程查询已知回执 ID；不存在时核查保存的目标／路径。回执缺失
  不证明未执行，也不授权重算或覆盖。桌面重启后不保留回执。

这些动作保留服务端权限、revision/context 检查、一次性计划和逐次桌面文件确认。
工具 annotations 既不授权，也不是重试依据。桌面和连接器一起升级后，重新连接客户端
并刷新工具目录。

## 一次计划设置多条曲线的样式

`properties.set` 每次只写一个值。要给每条曲线分别设置颜色和图例标签，请改为预检
`properties.set_many`：每一项写入指定一个明确目标、一个受支持的 key 和一个值。
省略顶层 `ids` 和 `targets`；曲线属性使用查询得到的完整 Series 目标，坐标轴标题、
图例等对象级属性使用图对象的资源目标，不带 component。

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "tool_id": "properties.set_many",
  "parameters": {"writes": [
    {"target": SERIES_0_TARGET, "key": "series.line.color", "value": "#0072B2"},
    {"target": SERIES_0_TARGET, "key": "series.label", "value": "Sample 1"},
    {"target": SERIES_1_TARGET, "key": "series.line.color", "value": "#D55E00"},
    {"target": SERIES_1_TARGET, "key": "series.label", "value": "Sample 2"},
    {"target": {"resource": PLOT_OBJECT_RESOURCE}, "key": "object.axes.x_label", "value": "Temperature (°C)"}
  ]}
}
```

`plot.create` 执行后，可以直接复用它返回的完整稳定目标。例如，将解码后的创建
`ToolResult` 存为 `created`，原状态存为 `state`，构造以下 `plotium_plan` 参数：

```js
const request = {
  expected_project_context: state.project_context,
  expected_revision: created.after_revision,
  tool_id: "properties.set_many",
  parameters: { writes: [
    ...created.value.bindings.map((binding, index) => ({
      target: binding.target,
      key: "series.label",
      value: `Sample ${index + 1}`
    })),
    { target: created.value.creation.plot,
      key: "object.axes.x_label", value: "Temperature (°C)" }
  ] }
};
```

复制整个 `target`，保留所属资源的 `kind`、`id`、`parent_id`、`local_id` 和存在时的
组件身份。单独的 SeriesId、名称或数组下标不是目标选择器。总写入数保持在 200 以内。
预检上述对象，审核后仅执行一次新 `plan_id`。`unsupported_operation` 表示操作不可用；
`unsupported_property` 在 `error.details.write_index`（从零开始）和 `key` 指出不支持
的属性。查询 `plotium_properties_list`，修正整个批次后再预检。

一个批次包含 1–200 项写入，可使用上文列出的显示属性。只要有一项写入的目标不存在、属性不适用于该目标、值无效，
或同一目标的同一 key 写了两次，预检就拒绝整个批次，不提交任何修改。目标已是该值的
写入会报告为跳过，其余写入照常生效。`plotium_execute` 把全部写入作为一个**撤销**
步骤提交；`value.writes` 列出被跳过的写入，含序号、key、结果和 `skip_reason`。

写属性的返回很短。`properties.set`、`properties.set_many` 和 `properties.reset`
的预检返回 `required_authority`、`estimated_changes`、`summary`（`compatible`、
`skipped`、`failed`），`targets` 只列出不会写入的目标。执行结果加上 `summary`
（`succeeded`、`skipped`、`failed`，以及按 `skip_reason` 计数的 `by_reason`）；
`targets` 和批次的 `value.writes` 只列没有成功的项。全部生效时两者都为空。
`plotium_preview_plan` 仍按完整计划渲染。

## 用户手动设置的值

用户在 Plotium 中修改图形值（在属性面板中设置、在控件上滚动调整或拖动图例）时，
Plotium 会把它记为手动设置。`plotium_get_state` 在 `hand_edits` 中列出这些值，
`properties.inspect` 为每个这样的读数标注 `"hand_set": true`。

`properties.set`、`properties.set_many` 和 `properties.reset` 不会改动手动设置的值：
该目标在预检和结果中都报告为跳过，`skip_reason` 为 `"hand_set"`，其余目标照常生效。
图例的位置选项和拖动后的位置算作同一个值。只有用户要求修改时，才在 `parameters` 中
传入 `"override_hand_edits": true`；修改后的新值仍算作用户的设置。

不带该参数写入的值不会被记录，之后的主题或写入可能会替换它们。主题会保留手动设置的值，
见下一节。

## 为画布套用主题或已保存的风格

`figure.apply_theme` 用 Plotium 的内置主题（例如 `nature`）或在 Plotium 中保存的画布
风格整体重设画布样式。`theme_id` 可以填 `plotium_capabilities_get` 返回的
`operation_constraints["figure.apply_theme"].builtin_themes` 中的内置主题 id，也可以填
已保存风格的名称。已保存的风格不在固定的能力说明里：未知的 `theme_id` 会在预检时以
`invalid_parameters` 失败，`error.details.themes` 列出当前可用的全部内置主题和已保存
风格。预检时把画布 id 放在顶层 `ids`：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 7,
  "ids": ["CANVAS_ID"],
  "tool_id": "figure.apply_theme",
  "parameters": {"theme_id": "nature"}
}
```

预检只需只读权限，执行需要 **读取并修改**。未知的 `theme_id`、Series `targets`，
以及不含任何画布的 `ids` 会在预检时被拒绝；其余不是画布的 id 会被跳过。已保存的风格
会在执行时重新读取。执行时把每张画布的背景、文字和形状颜色，以及主题规定的每条曲线
颜色、符号填充与描边、符号大小和线宽作为一个**撤销**步骤一起修改。手动设置的曲线和符号颜色在
重新套用主题时会保留；重置曲线颜色后，曲线重新使用主题颜色。文档的样式默认值也会被替换：图的文字字号、坐标轴线和
刻度是整个文档共用的，所以其他画布上的图也会随之改变；新建文字、面板字母和形状的
默认样式同样改用主题的设置。用户手动设置的值（包括图形排版）会保留，`diagnostics` 会附一条
说明保留了几处。按期刊栏宽设计的主题（目前为 `nature`）不会改变画布尺寸；
画布宽度不符合任一栏宽预设时，结果的 `diagnostics` 会为该画布附一条提示，列出预设宽度
（89 / 136 / 183 mm）。

## 保存当前桌面项目

保存复用 **Save project** 的原生快照、后台 worker、备份和完成回调，保存当前
桌面实际状态，不另开 CLI 项目。先读 `plotium_get_state`；`save_state` 给出当前路径、
已存档案 revision、dirty、edit_generation 和最多 16 个保留的 `receipt_ids`。
逐个用 `plotium_save_status` 查询完整回执和恢复动作；仅有 ID 不代表提交成功或已经完成。

首次保存或另存时，不带 `ids`／`targets`：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 46,
  "tool_id": "project.save",
  "parameters": {"path": "/absolute/path/figure.plotium"}
}
```

省略 `path` 保存当前文件；未命名项目必须给出路径。父目录须已存在，目标为普通、
非符号链接的 `.plotium` 文件或尚不存在的路径。预检解析父目录并冻结已观察的目标。
可选 `include_view_snapshots` 覆盖当前项目的原生保存配置。计划返回 `save_id`、
实际路径及 `overwrite_required`，不授予磁盘写权限。

用 `plotium_execute {"plan_id":"PLAN_ID"}` 提交一次性计划，结果为
`save.status: "awaiting_authorization"`。在 Plotium 核对准确路径，点击
**Allow save**；已有文件还必须勾选 **Replace the existing file at this exact path**。
**Deny** 不改目标。这与 **Read and modify** 独立，只授权本次保存，不能通过工具
参数代为批准。Read only 模式也可发起此授权请求。目标变化须重新规划并确认；
同时最多一个外部保存请求。

序列化使用独占创建的私有唯一同目录临时文件，不写固定 `<target>.tmp`。
已有目标在备份准备完成后、实际替换前再次复验预检冻结的文件元数据；发现变化即拒绝
安装，保留后来版本。一次性权限准入与文件复验是两个边界；复验加替换并非跨进程
compare-and-swap。非协作写入若发生在最后复验之后，或保留了所检查的元数据，
仍可能无法察觉；应避免并发保存到同一路径。

用 `plotium_save_status {"save_id":"SAVE_ID"}` 查询，不带项目 revision/context，
保存中也可读取且不使项目 dirty。`awaiting_authorization`、`queued`、`saving`
均未完成，`phase` 为 null；只有 `completed` 带最终阶段：

| phase | 文件状态与下一步 |
| --- | --- |
| `not_installed` | 本次未把快照安装到目标。处理拒绝或错误、检查实际状态；仍需保存时重新取得授权。 |
| `installed_unconfirmed` | 快照已替换／创建目标，但备份收尾或持久同步失败。保持项目打开，检查目标及存储后再明确授权重试；不能假定旧文件还在。 |
| `committed` | 捕获的快照完成原生持久提交；后来的编辑仍可能未保存。 |

Windows 替换部分失败时，即使新快照为 `not_installed`，旧目标仍可能已被移走或删除。
Plotium 保留已序列化的候选文件，并报告准确恢复路径。只有不覆盖现有目标时才尝试
从已知备份恢复原文件；无法完成时，保留并报告该备份及更早的暂存备份。如果 Windows
移走原文件却未返回其位置，错误会明确说明。保持项目打开，检查所列文件后再决定是否
重试；此类失败不会清除 dirty 状态。

回执包含原始 `project_context`、`snapshot.document_revision`、
`snapshot.edit_generation`、`persisted_revision`、准确路径、`state_applied`、
`dirty_after_completion`、错误及恢复说明。查询结果另给当前 context/path/revision/dirty
和 `matches_current_project`。快照在桌面批准后、后台工作前捕获；之后的新编辑
保持 dirty，完成不等于当前所有编辑已落盘。
已安装但未确认同步时仍保持 dirty，并记录已安装基线供原生恢复使用。
保存规划／提交和状态查询仅用临时桌面回执，不加审计记录；普通项目读取同样不追加
项目审计记录，也不改变 dirty。

切项目或停止连接撤销待写权限。worker 只持有原项目快照和路径，并在安装边界检查
授权；即便 document revision 相同，旧完成也不能装入另一项目或清它的 dirty。
worker 原子确认授权并进入安装后，停止不保证取消这次原文件写入，须查回执；
在此之前撤销则阻止安装。保存不加入 Undo，Undo 不删除或还原磁盘文件。

桌面进程保留最近 16 条回执，跨连接恢复和项目切换有效，退出应用后不保留。响应丢失后
先查询原 `save_id`，不要重放已提交调用。应用重启或 `unknown_save` 时，检查目标和
新打开的项目，不自行假定成功或失败。稳定 stdio 配置继续可用；重开／重启后仍须
重新读取状态、发现目标并规划。连接恢复不是保存成功的证据。


## 将捕获画布导出为 SVG、PDF 和 PNG

用 `figure.export` 指定一张准确的完整画布 TargetRef，不带顶层 `ids`／`targets`。
复用原生导出器，保留背景、完整页面、曲线、绝对 Y 误差幅度、原生文字和嵌入图片；
不裁切、不套主题、不改项目。成品之外仍须保留可编辑 `.plotium` 项目。

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 48,
  "tool_id": "figure.export",
  "parameters": {
    "canvas": {"resource":{"id":"CANVAS_ID","kind":"plotium.canvas"}},
    "size_mm": [180,90],
    "dpi": 300,
    "outputs": [
      {"format":"svg","path":"/absolute/path/figure.svg"},
      {"format":"pdf","path":"/absolute/path/figure.pdf"},
      {"format":"png","path":"/absolute/path/figure.png"}
    ]
  }
}
```

`size_mm` 默认原生页面尺寸；显式尺寸等比缩放整图，须保留宽高比（高度误差容许
0.001 mm），不改变可编辑页面。`dpi` 默认 300，范围 1–2400，仅控制 PNG。
每轴按原生物理 pt / 72 × DPI × 等比缩放系数四舍五入，最小 1 像素；
180 × 90 mm、300 DPI 得到 **2126 × 1063 像素**。SVG/PDF 保留物理页尺寸。
文字使用原生通用 sans-serif 字族；无可用系统字体时报错。SVG 在另一台机器的
字形取决于当地可用 sans-serif 字体。

每批 1–8 个输出，绝对路径须带匹配的小写扩展名，父目录已存在；目标必须是普通
非符号链接文件或尚不存在。重复的解析后输出路径拒绝。预检返回 `export_id`、
物理／像素尺寸，以及逐文件准确目标或错误；坏目录不会掩盖其他文件的结果。
`overwrite` 默认 false，已有目标报告 `overwrite_not_requested` 并保留原字节。
确需替换时显式设 `overwrite:true`，这仍不授予写入权限。

用 `plotium_execute` 提交一次性计划。有效文件等待 Plotium 的
**Allow AI to export this figure?** 提示，其中列出画布、revision、尺寸及路径。
核对后点击 **Allow export**；已有文件还必须勾选默认未勾选的
**Replace the listed existing output files**。**Deny** 不安装任何输出。
**Read and modify** 不等于导出授权；只读可请求新的独立导出授权。
全无效批次直接逐文件报错，不弹授权窗。

用 `plotium_export_status {"export_id":"EXPORT_ID"}` 查询，不带 context/revision，
渲染中也可查询且不生成审计。返回 `export`、当前 context/revision 和
`matches_current_project`。批次状态为 `awaiting_authorization`、`rendering`、
`completed`；`files[]` 每项包含格式、准确路径、覆盖要求、`status`、`error_code`、
`message` 和已安装字节数。文件状态为 `pending`、`succeeded`、`failed` 或
`unknown`（worker 失败，先检查该路径再决定下一步）。批次完成不等于全部成功。
缺失／损坏图片、字体、目标变化、权限拒绝和渲染／预算错误会准确报告且不替换
该输出；同批此前成功文件保留。

渲染在后台使用桌面批准后捕获的画布／资产，绑定原 `project_context`、`canvas`
和 `render_revision`，后续编辑不改变快照。切项目、停止连接或修改访问权限会撤销
待安装权限。逐文件原子权限准入先于实际安装：准入前撤销阻止安装；已准入文件
仍可能完成，须查回执。旧完成不改变新项目的文档、保存路径、dirty 或状态。
独占唯一同目录暂存及原子不覆盖创建保护未授权文件；已有目标在实际替换前复验
元数据，但并非跨进程 compare-and-swap，应避免并发写相同路径。
导出成功表示输出已安装，不等于原生项目保存的持久提交阶段；Undo 不删除或还原磁盘文件。

预算：一个运行批次，共享待执行计划最多 32 个；路径最多 4096 字节；PNG 单边
16,384、总计 16,777,216 像素；引用的嵌入资产 16 MiB、解码后 RGBA 64 MiB、
SVG 8 MiB、逐文件编码结果 64 MiB。不查找外部资产、不替换成占位图。
完整 schema 和限制可从 `connection_capabilities` 发现。

桌面进程保留最近 16 批导出回执，跨重连及项目切换有效，退出应用即丢失。
响应丢失后先查原 `export_id`，不重放导出。重启或 `unknown_export` 时先检查
准确输出文件。重开可编辑项目后重新读取状态、发现目标；仅为新意图重新规划。


## 科学操作与任务

`plotium_processing_inspect`、`plotium_diffusion_inspect` 和
`plotium_analysis_results_inspect` 是只读查询，使用显式 `ids`、
`expected_project_context` 和 `expected_revision`。结果检查接受 `offset`、
`limit`（1–100）；ILT 网格较大时请减小页长。发现信息中的
`scientific_operations` 来自核心操作定义，含参数 schema、能力、权限及执行方式。

以下写操作继续通过 `plotium_plan` / `plotium_execute`：

| 操作 | 参数与目标 |
| --- | --- |
| `processing.edit` | 一个数据集；`edits` 中使用 `replace`、`enable`、`add`、`remove` 或 `move`；已有步骤使用稳定 StepId |
| `processing.apply_scheme` | `path`，可选 `compatible_only`；方案字节、指纹和兼容目标配置在预检时冻结 |
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
