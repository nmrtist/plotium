---
title: MCP 页面布局与删除
description: 通过 MCP 安排页面、面板、文字和图片，精确删除内容。
---

## 原子编辑页面、面板、文字与本地图片

以 `parameters: {canvas: 完整画布目标, edits: [...]}` 预检 `figure.edit`，不传顶层
`ids`／`targets`。完整带 `op` 标签的 schema 可用
`plotium_capabilities_get {"section":"operation_constraints","key":"figure.edit"}` 读取。
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
| `annotation_create` | `target: 完整图目标`、`text`、`at: [x,y]`（该图的数据坐标）、`font_pt`；可选 `kind: "text"`、`color: "#rrggbb"` |
| `annotation_create`（参考线） | `target: 完整图目标`、`kind: "reference_line"`、`orientation: vertical\|horizontal`、`value`；可选 `label`、`font_pt`、`line: {color, width_pt, style}` |
| `annotation_create`（箭头） | `target: 完整图目标`、`kind: "arrow"`、箭尾 `at: [x,y]`、箭头 `to: [x,y]`；可选 `head: none\|open\|filled\|double`、`line_width_pt`、`color: "#rrggbb"` |
| `annotation_set` | `target: 完整图目标`、`annotation_id`；文字标注至少提供 `text`、`at`、`font_pt`、`color`、`offset_pt`、`placement` 之一，参考线至少提供 `orientation`、`value`、`label`、`font_pt`、`line`、`placement` 之一，箭头至少提供 `at`、`to`、`head`、`line_width_pt`、`color` 之一；可选 `override_hand_edits` |
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
时，会把覆盖所有成员数据的 x 范围（按各自坐标单位）和最上面成员的 x 主刻度间隔
（按各自显示的单位）写入每个成员，因此各成员须先
使用相同的 x 刻度类型；所有成员绘制最下面成员的 x 方向，但不保存该方向。各图共享
x 视窗；之后在任一成员上修改 x 范围、x 刻度类型、x 显示单位、x 千位分隔符或
x 主刻度间隔，都会
写到所有成员，并合为一次 Undo；若任一成员不能接受该值则拒绝写入，`applied` 会
列出各成员的修改。同一批次若请求相互冲突的共享 x 设置，会整批拒绝且不留下修改；
按兼容坐标单位表达的等价范围可以接受。
`shared_x: false` 恢复为自由布局并保留各 frame。结果在 `panel_edits` 中列出
每个操作的 `layout`、`gap_pt`、`members_top_to_bottom` 和 `merged_panels`。
`plotium_document_inspect` 为每个面板返回 `layout`（`free`、`vertical_stack`、
`horizontal_stack`、`grid` 或 `shared_x_stack`）；堆叠面板另含
`shared_x_stack: {members_top_to_bottom, gap_pt}`。`plotium_figure_diagnose`
和计划预览使用同一套堆叠几何。

图内标注按数据坐标定位，而不是页面 pt：文字居中显示在 `at` 点上方，画布和所有
导出中都跟随该图的轴范围、缩放和平移移动；锚点在可见范围外时随图一起裁切。该位置
压到数据、图例、数值标签或其他标签时，文字挪到约三个字高范围内最近的空位；指向它的
箭头不算遮挡。`offset_pt: [dx, dy]`（pt，x 向右、y 向上）把文字固定在该偏移处，
`[0, 0]` 即正好在 `at`；`placement: "auto"` 恢复自动摆放。`color`（`#rrggbb`）设置
文字颜色，默认黑色。用户在画布上拖过的标签在修改 `text`、`font_pt` 时保持原位；对它
设置新的 `at`、`offset_pt` 或 `placement` 会被拒绝，除非以 `override_hand_edits: true`
表明是用户要求。`annotations` 中每个文字
标注报告 `placement`（`auto`、`fixed` 或 `hand`）和实际绘制的 `offset_pt`；
`plotium_get_state` 在 `hand_edits` 中以键 `annotation.label_position` 列出拖过的标签。
`annotation_id` 只在所属图内有效，删除后不会复用；可从结果的 `annotation_edits`
或 `plotium_document_inspect` 返回的图 `annotations` 列表读取。`at` 使用
`coordinate_units` 给出的源单位，而不是刻度标签所选的显示单位。`text` 中的 `\n`
另起一行。标注文字最多 1024
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
并以 `kind` 区分。提供另一类标注的字段会被拒绝，而不是被忽略；参考线的颜色用
`line.color` 设置，不用 `color`。

用户在画布上移动过的箭头或参考线报告 `hand_moved: true`，并在 `hand_edits` 中以键
`annotation.position` 列出。可以改它的样式，但给这样的箭头设置新的 `at`、`to`，或给
这样的参考线设置新的 `value`、`orientation` 会被拒绝，除非以
`override_hand_edits: true` 表明是用户要求；改后位置重新算作 agent 的设置。

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
