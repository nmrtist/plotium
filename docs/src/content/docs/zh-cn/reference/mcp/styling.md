---
title: MCP 样式修改
description: 通过 MCP 修改显示属性、图例和主题。
---

## 精确修改显示，不重建图对象

直接使用预检的 `creation.series_defaults[].target` 或执行的 `value.bindings[].target`，
或通过 `plotium_document_inspect`
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
  轴属性 reset 只清除指定 override；guide 原点/placement 按[精确放置单图图例](#精确放置单图图例)中的联动规则重置。

显示单位接受属性查询返回的兼容代码，例如 s→ms、mA→A。图对象仍保持 R4 坐标单位契约：
原始数据、手动范围存储、位移及误差幅度的含义不变；刻度及自动派生的单位文字改变，
人工轴标题保持原文。重绑、重排和删除首条曲线不重新定义坐标。隐藏轴或分类轴不能
设置数值手动范围，但可清除已保留的范围。属性查询中的 `unit_code.codes` 返回该目标的兼容选项；
`axis_range.input_schema` 描述输入，`view` 和 `unit` 表示当前显示窗口。

符号大小以 pt **直径**表示，显式设置后独立于线宽。没有覆盖时，原生点图仍沿用
现有基于 width 的半径，原线宽接口含义不变；reset 恢复原生符号/大小。
显式空图例文字使该图例项不显示。线型仅适用于折线，散点目标会按规则跳过。
符号适用于折线和散点，不适用于 stick、柱状、箱线和小提琴曲线；柱宽和描边宽度适用于柱状、箱线和小提琴曲线，描边颜色仅适用于柱状曲线。符号填充色和边框色作用于闭合符号，未设置时跟随
曲线颜色，reset 恢复跟随。空心符号不填充并保留边框，未设置边框宽度时为 0.5 pt。`marker_every` 从第一个源数据点起每 n 个点画一个符号，折线仍经过每个点。颜色由图型语义决定时，单曲线改色、改符号填充色或边框色按不适用跳过。

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
   显式稳定 ID；所有匹配理由相同（例如无筛选搜索）时，`value.reason` 只给出一次，
   不再返回 `value.reasons`。`plotium_resources_inspect` 用 `ids` 和 `expected_revision` 读取描述。
3. 用 `{}` 调用 `plotium_properties_list` 列出支持的属性 ID 和值类型。
   使用 `plotium_properties_inspect` 查询曲线地址：

   ```json
   {"ids":["CANVAS_UUID/OBJECT_ID"],"key":"series.line.stroke_width","expected_project_context":"PROJECT_CONTEXT","expected_revision":7}
   ```

   返回的 `value.readings` 为每条适用曲线提供一行；各行 schema 或默认值相同时，改为在
   `value.schema` 和 `value.default_value` 中给出一次。复制所选曲线的完整
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

一个批次包含 1–200 项写入，可使用 `plotium_properties_list` 返回的属性。只要有一项写入的目标不存在、属性不适用于该目标、值无效，
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
edit generation、automation revision 或 Undo 历史。能力发现见 `plotium_capabilities_get {"section":"guide_contract"}`。

## 为画布套用主题或已保存的风格

`figure.apply_theme` 用 Plotium 的内置主题（例如 `nature`）或在 Plotium 中保存的画布
风格整体重设画布样式。`theme_id` 可以填
`plotium_capabilities_get {"section":"operation_constraints","key":"figure.apply_theme"}`
返回的 `builtin_themes` 中的内置主题 id，也可以填
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
