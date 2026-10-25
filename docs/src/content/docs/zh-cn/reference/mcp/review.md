---
title: MCP 预览、诊断与恢复
description: 渲染预览、诊断图面几何，以及出错后的恢复。
---

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
无需打开文件或自行渲染 SVG 文本。点数超过预览所能显示的曲线以最小/最大包络
绘制：每个像素列保留四对最小/最大值点，整个绘图区至少 2,048 对（标记则每个像素
一个）；`downsampled` 以 `{target, source_points, drawn_points}` 列出每条这样的
曲线，没有降采样时省略。放大后可见的数据点全部保留。
导出始终绘制全部数据点。MCP 客户端须将图像内容交给 agent；纯文本
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
`plotium_capabilities_get {"section":"limits","key":"image_preview"}` 也提供这些预算。

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
| `properties.set` / `properties.reset` / `properties.set_many` | 文档和图对象的显示属性及原生多目标批次；不含画布属性、应用偏好、数据处理，以及图表类型和堆叠方式的修改 |
| `figure.edit` 局部编辑 | `content_frame`、`image_replace`、`text_set`、`annotation_create`／`annotation_set`／`annotation_delete`（文字标注与参考线）及原生批次 |
| `figure.edit` 纯删除批次 | `content_delete`、原本为空的 `panel_delete`、显式列绑定 `series_delete`；原拒绝规则不变 |
| 其他操作，包括保存和导出 | 返回 `unsupported_operation`，不改变其正常 execute 合同 |

想让用户挑选样式（配色、线宽、图例位置、标签布局等）时，规划两三个备选计划，逐个预览，
把图并排给用户看。只执行用户选中的计划。其余计划从不提交，之后会返回 `stale_revision`，
但在提交或连接重置前仍占用 32 个待执行计划的名额。

受影响画布须使用普通折线／散点、原生文字、形状和栅格内容；预览不启动科学计算。
跨画布属性计划在隔离原生上下文应用完整批次，可逐个请求受影响画布，仍是一个计划、
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
  "checks": ["bounds","clipping","text_overflow","content_overlap","legend_occlusion","data_shape","layout_aesthetics","intent"],
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
`output`、`items`、`coverage`、`truncated` 和 `continuation`。默认的 compact 详情中，
`output` 只含 `size_mm` 和 `size_pt`，`coverage` 给出状态、扫描计数以及不可用或不支持的
条目数。`detail: "full"` 时 `output` 另含页面坐标说明，`coverage` 另含请求、支持和不支持的
检查列表，并附 `budget`、`preview_request` 和 `read_policy`。`summary`
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
- `axis_ticks_sparse` 是警告：在输出尺寸下，显示刻度标签的坐标轴画出的刻度少于两个，
  读不出刻度比例。增大面板，或调小 `object.figure.typography.tick_pt`。
- `right_axis_untitled` 是警告：有曲线使用右 y 轴，但右轴没有显示标题。设置
  `object.axes.y2_label`（或重新打开 `object.axes.y2_show_label`）。
- `legend_occlusion_potential` 表示潜在视觉冲突，不表示数据必然不可见。逐段／逐点
  检查实际投影、有效裁剪后的线与 marker，分批保留虚线相位，不使用整条曲线包围盒，
  不抽样。线条包络与图例布局矩形是保守边界；圆角、半透明和抗锯齿可能有所不同。

`data_shape`、`layout_aesthetics` 和 `intent` 检查报告 `severity` 为 `hint` 的问题：图可以表达得
更清楚，但没有错误。省略 `checks` 时包含这三项。提示从不修改文档。每条提示带
`suggestions`，即 `{action, property, target, reason}` 列表；`property` 是要在 `target`
上设置的属性，没有则两者都为 null。长度按请求的 `output.size_mm` 测量；阈值是暂定值。
限定到 Series 的查询只报告这些 Series 的提示。

| 代码 | 触发条件 |
| --- | --- |
| `small_values_invisible` | 某根柱的数值不到数值轴跨度的 1 %，或打印长度不到 0.5 mm，而旁边有至少 5 mm 长的柱 |
| `bars_too_narrow` | 某根柱打印宽度不到 0.6 mm |
| `few_samples_hidden` | 箱线、小提琴 Series，或每个 x 有多行的柱状 Series，每组都少于 10 个值；或每个 x 只有一行的柱状 Series 的 `series.replicates` 为 1–9；且图中没有散点 Series |
| `too_many_categories` | 类别轴显示标签，但省略了部分标签，或相邻标签相互重叠 |
| `log_scale_candidate` | 不含柱、箱线或小提琴的图，线性数值轴上的折线或散点值全为正，且跨越三个及以上数量级 |
| `bar_baseline_not_zero` | 数值轴范围不包含柱的基线 |
| `labels_overlap` | 画布上两个文字框在两个方向上都重叠超过 0.2 mm；证据给出两者和第二个的移动方向。同一图例的条目之间、同一坐标轴的刻度标签之间不互相比较 |
| `text_over_data` | 标注或数值标签压在柱、箱线、小提琴、折线或标记上 |
| `intent_mismatch` | 图型不适合这张图的 `object.intent.purpose`（见下文） |
| `panels_misaligned` | 同一列（左边缘相差 1 mm 以内）或同一行（下边缘相差 1 mm 以内）的两个框，绘图区左边缘或下边缘相差超过 0.5 mm；或同一行／列三个及以上框之间的间距相差超过 0.5 mm |

同类提示超过 50 条，或检查用完预算时，会增加一条 `unavailable` 覆盖行。

`intent` 检查对照作者声明的目的看每张按列绑定的图；`detail` 从不报告。没有声明目的的图
会得到一条 `not_applicable` 覆盖行 `no_purpose_stated`：这时请询问用户这张图要表达什么。
表格图表类型得到 `chart_type_not_covered`。`intent_mismatch` 的证据在
`interpretation` 中给出原因；`detail: "full"` 时还给出目的、claim 和读到的图表特征。
建议里只列 Plotium 能画出的图型；请转告用户，由用户选择。

| 目的 | 报告条件 |
| --- | --- |
| `compare` | 折线连接了各类别 |
| `trend` | 类别方向的轴是类别轴，或图中画了柱 |
| `distribution` | 类别轴上只有柱或折线，没有箱线、小提琴或散点 Series |
| `relationship` | 类别方向的轴是类别轴，或图中画了柱、箱线或小提琴 |
| `composition` | 图中有折线或点，但没有柱 |
| `change` | 两个及以上柱状 Series 并排，或正的柱值跨十倍及以上；或图中没有柱、箱线和小提琴时，左轴上为正的折线或散点值在线性轴上跨十倍及以上 |

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

用画布、上下文和 `basis_revision` 调用 `plotium_render_preview` 取得并实际查看原生 PNG。
X 坐标转像素为
`x * width_px / output.size_pt[0]`，Y 使用对应高度；核对其
`render_revision` 与 `basis_revision` 一致。编辑器 zoom 不影响上述坐标。直接 core
`figure.diagnose` 只返回一个有界 batch，不保留 cursor；MCP 查询提供继续读取。
诊断不会自动修图。

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

## 错误码

稳定错误码包括 `project_changed`、`unavailable`、`connection_lost`、`permission_denied`、`expired_plan`、`stale_revision`、`busy`、
`invalid_parameters`、`unsupported_operation`、`unsupported_property`、`plan_limit`、
`request_cancelled`、`connection_stopped`、`response_too_large` 和 `execution_failed`。
无效的能力分区或键会指向静态发现，并在 `error.details` 给出合法候选。
无效的规划参数指向该操作的 `operation_constraints`，而不是要求读取项目状态。
输入错误在已解析的请求边界提供 `error.details.field`，例如 `plan_id`、
`parameters` 或 `parameters.writes[0].key`。过期 revision 和项目切换仍须读取状态并重规划；
这些指引不允许重放。嵌套的 source 和目标错误给出完整路径，例如
`parameters.series[0].source.y`。

错误提供 `error.recovery`，异步保存、导出和任务回执提供 `recovery`。
`retryable: false` 表示不要重复失败的调用，应遵循下一步动作。`busy` 执行可能已经
消费计划。revision 冲突需读取状态并审核新计划；过期计划可能已经执行，必须先核查结果。
AI connections (MCP) 设置页面显示最近操作和错误，后台反馈不会自行打开设置页面。
