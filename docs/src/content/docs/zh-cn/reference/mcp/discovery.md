---
title: MCP 状态与发现
description: 读取当前状态、发现能力并找到精确目标。
---

## 读取状态与按需发现能力

`plotium_get_state {}` 读取最新动态状态，返回 `project_context`、`revision`、
`availability`、`permissions`、`save_state`、`capability_fingerprint`、项目数量摘要
和有上限的选择分页，不附带完整能力清单或项目蓝图。用绑定 revision 的
`plotium_document_inspect` 分页或 `plotium_resources_search` 发现精确目标，
再用 `plotium_resources_inspect` 按明确 ID 读取详情。复制返回的完整目标，不用名称或集合位置代替。

有两种简写可以少抄 ID。资源引用可以省略 `parent_id` 和 `local_id`，Plotium 会从
`CANVAS_UUID/OBJECT_ID` 这类嵌套 ID 推出。规划和属性查询的 `targets`、曲线绑定和
`properties.set_many` 的写入目标可以把 Series 写成
`{"plot":"CANVAS_UUID/OBJECT_ID","series":0}`。两种简写都在规划前展开成完整目标；
计划、结果和审计记录仍保存完整形式。

空闲时 `selection` 为 `{items,total,next_offset}`，默认最多返回 16 个稳定资源引用。
传入 `selection_offset`（默认 0）和 `selection_limit`（1–50）读取后续页，例如：

```json
{"selection_offset":16,"selection_limit":50}
```

`selection.canvas` 报告用户在当前页面上选中的内容：`{items,total,part}`。`items`
最多 `selection_limit` 项，是所选 Panel 和对象的摘要（`target`、`kind`、`name`、
`visible`）。`part` 是用户在首个所选图中点中的图例、标签或标注，没有时为 `null`。图例为
`{target,part:"legend",property:"object.guides.position",value}`，`value` 为 `null`
表示自动位置；标签为 `{target,part:"label",kind,annotation_id,edit:"figure.edit annotation_set",…}`
并带当前位置；箭头或参考线本身为 `{target,part:"annotation",kind,annotation_id,edit,…}`，
带位置和 `hand_moved`。用户说“这个”时，改的就是这个目标或部件。

每次调用都读取当前选择，分页不是冻结快照；GUI 选择变化不一定改变文档 revision。
不能将多页视作原子选择或把选择当作执行授权。

`hand_edits` 以 `{items,total,next_offset}` 列出用户在 Plotium 中手动设置的图形值，
每项包含 `target`、属性 `key`、`label` 和当前 `value`。传入 `hand_edits_offset` 和
`hand_edits_limit`（1–50，默认 16）读取后续页。继续编辑时请保留这些值，见
[用户手动设置的值](/zh-cn/reference/mcp/styling/#用户手动设置的值)。

忙碌时 `project`、`selection` 与 `hand_edits` 为 `null`；仍可读取可用性、权限、能力指纹
和保存回执 ID。

调用 `plotium_capabilities_get {}` 获取精简索引
`{capability_fingerprint,connection_capabilities}`。索引列出各工具的操作和所需权限、
按预检与执行权限分组的可编辑操作、科学操作、恢复／revision／属性批量契约、文档根和
`sections`。工具说明、输入 schema 和 annotations 见 `tools/list`；属性定义见
`plotium_properties_list`。

`sections` 列出每个详情分区的字节大小及是否接受 `key`。`operation_constraints` 和
`operation_details` 为每个 key 附一句用途，便于先按任务找到操作再读取详情。用 `{"section":...,"key":...}`
读取单项详情，返回 `{capability_fingerprint,section,key,contract}`：

| 分区 | key | 内容 |
| --- | --- | --- |
| `tools` | 可选，工具名 | 结果约定和目标规则；指定 key 时也返回工具输入 schema |
| `operation_constraints` | 必填，操作 id | 单个可编辑操作的 schema、权限和结果约定 |
| `operation_details` | 必填，操作/子操作 key，或 `keys` | 聚焦的图形编辑 schema、权限、结果约定和示例 |
| `scientific_operations` | 必填，操作 id | 描述符和所需权限；可编辑操作引用其规范输入 schema |
| `guide_contract` | 无 | 图例和色标定位规则 |
| `candidate_preview` | 无 | 未提交计划的预览约定 |
| `limits` | 可选，限制名 | 连接预算 |

例如读取 `figure.edit` 的 schema：

```json
{"section":"operation_constraints","key":"figure.edit"}
```

已知操作可直接查询详情，无须先读索引。例如页面编辑使用
`{"section":"operation_details","key":"figure.edit/page"}`。多项编辑用 `keys` 代替 `key`，
例如 `{"section":"operation_details","keys":["figure.edit/panel_label","figure.edit/annotation_create.arrow"]}`。
响应包含 `keys` 和以请求名称为键的 `contract` 映射。此分区每次接受 1–32 个不重复的 key；
其他分区只接受 `key`。每项详情包含一个精确的编辑 schema，以及只适用于该编辑的限制和
结果字段；请将示例目标替换为文档检查返回的完整稳定引用。混合编辑的
完整 schema 仍见 `operation_constraints`。可编辑科学描述符通过
`input_schema_ref: {section,key}` 引用规范输入 schema，不重复返回该 schema。

未知分区或 key 返回 `invalid_parameters`，`error.details.sections` 或
`error.details.keys` 列出有效值。

指纹是完整静态发现契约（索引、全部分区、MCP 工具定义和属性目录）按规范 JSON
对象键顺序计算的 SHA-256。按响应中的指纹缓存索引和已读取的分区；状态中的指纹不同时重新发现。
若读取状态后、能力查询完成前状态已变化，以能力响应随附的指纹标识该契约。
项目编辑、项目切换、选择或权限变化均不改变静态能力。

静态发现提供所需权限，不返回 `currently_permitted` 标记；当前权限和可用性始终从状态读取。
执行仍检查服务端权限、上下文、revision 和一次性计划。指纹、缓存的 schema 及 annotations
不授予权限，也不允许自动重试。保存和导出仍须对每个精确请求取得新的桌面确认。
升级时一起更新桌面与连接器，并重连客户端以刷新工具目录。

规划、属性检查、文档检查与图形诊断的工具输入直接列出字段，便于客户端展示完整签名。
Plotium 仍在服务端检查条件约束：需要选择器时只提供一个非空 `ids` 或 `targets`；
操作在参数内部指定目标时省略二者；表操作只接收一个数据集 ID。只有
`properties.set` 和 `properties.reset` 接收 Series 目标。文档分页仅适用于
`view: "children"`；诊断续页只接收 `cursor`、context 和 revision，首次诊断必须提供
`canvas` 与 `output`。

同步提交的 `plotium_execute` 结果包含 `project_context`、`before_revision` 和
`after_revision`。已知该上下文中的精确目标时，可直接将返回的 context 和
`after_revision` 用于下一次查询或规划，无须额外读取状态。这不刷新选择、手动值、
可用性或权限；首次访问、桌面编辑或 Undo/Redo 后、需要这些动态值时，以及
stale/context 错误或结果不确定时，仍须读取状态。执行继续检查上下文、revision 与权限。

图形编辑、曲线追加／绑定／展开、科学结果绘图、重命名、主题和删除数据集的计划返回
`plan_id`、`project_context`、`basis_revision` 和 `preflight`，不再回显请求和冻结目标集。
`preflight.summary` 统计可执行、跳过和失败的目标数，`preflight.targets` 只列出将被跳过或
失败的目标。它们的执行结果用 `summary` 统计成功、跳过和失败数，`targets` 只列跳过或
失败的行，已修改资源见 `produced` 和 `modified`。

`plotium_apply` 的输入与 `plotium_plan` 相同，一次调用完成一项可撤销编辑的规划和提交，
返回 `plotium_execute` 的结果。它接受 `properties.set`、`properties.set_many`、
`properties.reset`、不含删除的 `figure.edit`、`plot.create`、`plot.series.append`、
`plot.series.bind`、`plot.series.materialize_default`、`resource.rename` 和
`figure.apply_theme`，需要“读取和修改”权限。预检、revision 与权限检查、Undo 和运行记录
都与两步形式相同，且不保留计划。保存、导出、删除、导入、表格和科学操作仍用
`plotium_plan` 加 `plotium_execute`，预览候选图也是如此。被拒绝的 apply 不提交任何内容。
断线后先读状态：revision 等于请求的 `expected_revision` 表示编辑没有提交，否则检查目标；
不要重放这次调用。

科学任务排队或提交并不代表修改已提交，不可复用即时响应的 `after_revision`。
只有已知项目上下文中终态为 `succeeded` 且包含 `end_revision` 的回执才允许复用。
科学有效性与收敛情况须单独检查：任务成功并不证明拟合有效。失败、取消、失效、
缺失或不确定的回执继续遵循恢复流程，绝不授权自动重放。保存/导出回执描述文件操作，
不提供新的文档 revision。

## 不依赖名称发现结构

资源描述中的能力属于 **core** 的数据和渲染语义；资源列出某项能力，不代表此连接
已开放该操作。可规划的操作以能力索引为准。

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

`plotium_properties_list {}` 默认以紧凑形式浏览全部支持属性，每个属性一行：`id`、`label`、
值类型 `type`，以及适用时的 `min`/`max`、`unit`、`format`、枚举 `values`、曲线 `encoding`
和所需字段能力 `requires`。以 `document.` 开头的 ID 属于文档范围，其他 ID 属于对象范围，
`series.` 开头的 ID 以 Series 为目标。标为 `requires_full` 的行（嵌套输入和显示单位代码）
需要读取完整定义。可一次读取多个属性的完整定义：

```json
{"keys":["series.line.stroke_width","series.line.color"],"detail":"full"}
```

`scope` 使用目录中的范围名称筛选，`prefix` 按属性 ID 开头筛选，例如 `series.line` 或
`object.axes`；没有匹配的前缀会报错，并在 `error.details.prefixes` 中列出可用前缀。`keys` 必须非空且不重复；不支持的键、未知 scope
和键与 scope 冲突均明确报错，不会静默省略请求项。无筛选的 `detail: "full"` 返回完整目录。
两种形式均为静态查询，忙碌时也可调用，不包含当前目标值。完整详情保留复杂输入约束、
颜色格式 `#rrggbb` 和全部已注册单位代码。用 `plotium_properties_inspect` 查询目标值、
默认值、范围和适用性；坐标轴只接受与其坐标单位兼容的代码。静态适用能力不授予连接修改权限。
