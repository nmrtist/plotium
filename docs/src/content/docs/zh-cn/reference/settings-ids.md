---
title: 设置 ID
description: 自动化窗口、工作流、命令行和 AI 客户端使用的稳定设置标识。
---

对象检查器、处理步骤和偏好设置里的每项设置都有一个稳定 ID，例如
`series.contour.count`。只有在面板之外指定设置时才需要它：自动化窗口的属性工具、
工作流文件、[命令行](/zh-cn/reference/cli/)，或你自己编写的
[MCP 客户端](/zh-cn/reference/mcp/)。AI 客户端会自己查找这些 ID。

完整列表及取值类型和范围可用 `plotium-cli tool properties` 或 MCP 工具
`plotium_properties_list` 获取。下面各表列出最常用的设置。命令面板也能按 ID 匹配设置。

## 读取和写入设置

自动化窗口和工作流通过三个工具访问设置：

| 工具 | 作用 |
| --- | --- |
| **Inspect a property** | 读取当前值、默认值，以及该参数允许的范围 |
| **Set a property** | 写入一个值 |
| **Reset a property** | 按当前数据重新推导该值，与面板上的重置按钮一致 |

在工具参数中用 ID 指定设置：**Set a property** 用
`{"key": "series.contour.count", "value": 12}`，另外两个用
`{"key": "series.contour.count"}`。该指定哪个资源由设置本身决定：等高线和线条
设置在图对象上，切趾设置在数据集上，图形排印在文档上（列为
**Plotium document**），页面尺寸在画布上，偏好设置在应用上（列为
**Plotium application**）。

## 图形

这些 ID 对应[图形](/zh-cn/guides/plots-and-charts/)各指南中介绍的图和图形设置。图设置
（`object.*`）作用于一张图，序列设置（`series.*`）作用于一条曲线，排印设置
（`document.figure.typography.*`）作用于文档中的所有图。

### 坐标轴

| 设置 | ID |
| --- | --- |
| X / Y 轴标题 | `object.axes.x_label`、`object.axes.y_label` |
| X / Y 范围 | `object.axes.x_range`、`object.axes.y_range` |
| X / Y 显示单位 | `object.axes.x_display_unit`、`object.axes.y_display_unit` |
| 是否显示 X / Y 刻度标签 | `object.axes.x_show_tick_labels`、`object.axes.y_show_tick_labels` |
| 是否显示 X / Y 轴标题 | `object.axes.x_show_label`、`object.axes.y_show_label` |
| X / Y 刻度类型 | `object.axes.x_scale`、`object.axes.y_scale` |
| 等比例坐标（1:1） | `object.axes.equal_scale` |
| 边框 | `object.axes.frame` |
| X / Y 主刻度间隔 | `object.axes.x_major_step`、`object.axes.y_major_step` |
| X / Y 千位分隔符 | `object.axes.x_thousands_separator`、`object.axes.y_thousands_separator` |
| X / Y 断口 | `object.axes.x_break`、`object.axes.y_break` |
| X / Y 下段占比 | `object.axes.x_break_lower_fraction`、`object.axes.y_break_lower_fraction` |
| 右 y 轴标题、范围、显示单位 | `object.axes.y2_label`、`object.axes.y2_range`、`object.axes.y2_display_unit` |
| 是否显示右 y 轴刻度标签 / 标题 | `object.axes.y2_show_tick_labels`、`object.axes.y2_show_label` |
| 右 y 轴刻度类型、主刻度间隔 | `object.axes.y2_scale`、`object.axes.y2_major_step` |
| 曲线放在右 y 轴 | `series.y_axis` |

断口写作 `{"mode":"manual","min":…,"max":…,"unit":…}`，或 `{"mode":"auto"}` 表示
不断开。图形诊断会报告被断口隐藏的数据点。

### 图例、线条与符号

| 设置 | ID |
| --- | --- |
| 图例显示、位置、排列、标题 | `object.guides.visibility`、`object.guides.position`、`object.guides.layout`、`object.guides.title` |
| 图例摆放位置 | `object.guides.placement` |
| 图例文字 | `series.label` |
| 曲线是否可见 | `series.visible` |
| 线色、线宽、线型 | `series.line.color`、`series.line.stroke_width`、`series.line.style` |
| 符号及符号大小 | `series.line.marker`、`series.line.marker_size` |
| 符号填充色、边框色、边框宽度 | `series.line.marker_fill`、`series.line.marker_edge_color`、`series.line.marker_edge_width` |
| 空心符号、每 N 个点画一个符号 | `series.line.marker_hollow`、`series.line.marker_every` |
| 柱宽、柱边框色、柱边框宽度 | `series.bar.width`、`series.bar.edge_color`、`series.bar.edge_width` |
| 误差棒含义、重复次数 | `series.error_bars`（`unspecified`、`sd`、`sem`、`ci95`）、`series.replicates` |

### 排印

| 设置 | ID |
| --- | --- |
| 字体、字重 | `document.figure.typography.typeface`、`document.figure.typography.font_weight` |
| 刻度标签、轴标题、图标题和图例字号 | `document.figure.typography.tick_pt`、`document.figure.typography.label_pt`、`document.figure.typography.title_pt`、`document.figure.typography.legend_pt` |
| 图例文字颜色 | `document.figure.typography.legend_color` |
| 坐标轴线宽、刻度长度、刻度方向 | `document.figure.typography.axis_line_pt`、`document.figure.typography.tick_length_pt`、`document.figure.typography.tick_direction` |
| 子刻度 | `document.figure.typography.minor_ticks` |
| 单张图的字号 | `object.figure.typography.tick_pt`、`object.figure.typography.label_pt`、`object.figure.typography.legend_pt` |

### 图表与色图

| 设置 | ID |
| --- | --- |
| 图表类型 | `object.chart.type_id` |
| 自动分箱、箱数 | `object.chart.bins.auto`、`object.chart.bins.count` |
| 堆叠柱 | `object.chart.stacked` |
| 色图 | `object.chart.colormap` |
| 转置热图 | `object.chart.transpose` |
| 用途、结论 | `object.intent.purpose`、`object.intent.claim` |
| 热图颜色范围、范围中心 | `series.heatmap.range_span`、`series.heatmap.range_center` |

### 等高线层级

| 对象检查器中的设置 | id | 取值 |
| --- | --- | --- |
| **Lowest level** | `series.contour.base.magnitude` | 大于 0 的数；究竟以什么计量取决于 **Anchor** |
| **Anchor** | `series.contour.base.policy` | `absolute`、`noise_floor`、`background_scale`、`fraction_of_range` |
| **Levels** | `series.contour.count` | 1 到 256 |
| **Level ratio** | `series.contour.ratio` | 大于 1，最大 10 |
| **Negative contours** | `series.contour.negative.enabled` | `true` 或 `false` |
| **Positive colour** | `series.contour.positive_color` | `"#rrggbb"` |
| **Negative colour** | `series.contour.negative_color` | `"#rrggbb"` |
| **Line width** | `series.contour.line_width` | 0.05 到 10 pt；默认 0.5 |

各等高线设置的含义以及每种锚定方式需要什么数据，见[等高线层级](/zh-cn/guides/contour-levels/)。

## 处理设置

| 切趾步骤上的设置 | id | 取值 |
| --- | --- | --- |
| **Window** | `dataset.processing.apodization.kind` | `none`、`cosine_bell`、`exponential`、`gaussian` |
| **LB** | `dataset.processing.apodization.lb_hz` | −10000 到 10000 |
| **GB** | `dataset.processing.apodization.gb_hz` | 大于 0，最大 10000 |

各行的说明见[数据处理](/zh-cn/guides/processing/)。

## 应用偏好

偏好设置在 **Plotium application** 上使用同样三个工具。对于
`settings.appearance.accent.color`，**Set a property** 接受 `"#rrggbb"`，把画布
强调色固定为该颜色；**Reset a property** 清除它，强调色重新跟随主题。

## 批处理结果

### 勾选的目标会展开成什么

被勾选的资源往往并不是真正被写入的对象——展开成什么由参数决定：图对象展开
为它的各条序列，数据集展开为它的各个处理步骤，文档就是它自己。预检和结果列
表按展开后的部件逐行列出，因此一个含三条序列的图会给出三行，并指明是哪个图
对象里的哪条序列。

参数够不到的部件会标为 **Skipped** 并给出原因，其余部件照常应用：等高线下方
的热图、参数属于切趾时列表里的零填充步骤，或对一个窗函数为 *Exponential* 的
步骤索取 **GB**，都是这种情况。完全没有可寻址内容的资源（文本框、没有管线的
数据集）同样被跳过。

结果行和运行记录会在资源旁写出部件：

```json
{
  "resource": { "id": "…", "kind": "plotium.dataset" },
  "component": { "kind": "processing_step", "id": 3 }
}
```

序列部件写作 `{"kind": "series", "id": 2}`。两种 id 都只在它所属的资源内有
意义，换一个数据集，同一个步骤 id 就什么也不代表。

### 为什么某个目标被跳过

每一条被跳过的行都带有一句供人阅读的说明。由写入本身跳过的行还带有
`skip_reason`——一个稳定的标记，工作流据此分支即可，不必去匹配文字：

| `skip_reason` | 含义 |
| --- | --- |
| `already_at_value` | 目标本来就是这个值，没有写入任何内容 |
| `not_applicable` | 该参数不适用于这个目标 |
| `target_missing` | 这个地址在文档里已经指不到任何东西 |

在预检阶段就被排除的行只有那句说明。它们绝不会是「值本来就一样」这种情况，
因此没有这个标记本身就是区分。

如果 **Set a property** 的所有目标已存储所请求的值和覆盖状态，则不再写入：
每个目标报告跳过，文档版本号不变，也不增加撤销步骤。把继承值固定为显式覆盖，
仍会记录这项人工选择。

超出范围的值在 **Preflight** 阶段、也就是确认之前就会被拒绝，并同时给出你
填的值和拒绝它的边界。过了这一步，列出的部件要么全部生效，要么全部不变，
写入的改动合并为一次 **Undo automation** 撤销。

**Inspect a property** 把读数返回给调用方：作为工作流的一步运行时，读数会
进入运行记录，[命令行](/zh-cn/reference/cli/)写出的清单文件里也是同一份内容。
在自动化窗口里，数值显示在逐部件结果下方的 **Result value (JSON)** 区域。

## 读数与 schema

每条读数写出自己的 `target`，并带上当前值 `value`、参数有默认值时的
`default_value`、表示当前值是否偏离默认值的 `modified`、可用状态
`availability`，以及约束取值的 `schema`。

当前状态下不允许写入的参数照样会被读出：`availability` 为 `"disabled"`，
`disabled_reason` 说明要先改什么——例如设置 φ0 之前，先把相位模式切到
*Manual*。

schema 以 `type` 标记，取值为 `bool`、`text`、`int`、`stepped_int`、`float`、
`enum`、`color`、`unit_code` 或 `axis_range`。

- `int` 与 `stepped_int` 带 `min` 与 `max`，参数有单位时还带 `unit`。
  `stepped_int` 另有取值必须落在的格点 `step`——例如 Savitzky-Golay 窗口为
  3 到 201，步长为 2。
- `float` 带 `min`、`max` 与 `exclusive_min`；若参数拒绝某些取值，还带
  `excluded`（单个取值）或 `excluded_magnitude`（绝对值不超过该阈值的全部
  取值）。其 `display`（`linear`、`degrees` 或 `log10`）说明面板如何显示这个
  数；旁边的 `unit` 与 `log` 只是同一件事的另一种写法。
- `enum` 列出各变体，每个都带稳定 id 和标签。

无论 `display` 是什么，边界和取值始终使用参数自身的单位：`display` 为
`degrees` 的相位值，在线格式中仍是弧度。
