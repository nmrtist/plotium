---
title: 自动化
description: 把同一个操作一次应用到多个数据集或图，或运行已保存的工作流。
---

当同一个操作需要作用于一整个系列的实验时，自动化用一次可预检的批处理
取代逐个数据集的点击。

选择**文件 → Automation…**（命令面板中也可调用）。窗口分两个标签页：

## Current Project

作用于已打开的内容。搜索并勾选需要的数据集或图——或点击 **Current
selection** 载入当前选择——再选择一个工具，点击 **Preflight** 预览会影响
哪些目标、哪些会被跳过。**Confirm and execute** 执行，整批操作合并为
一次 **Undo automation** 撤销。

### 参数设置

有三个工具能改到对象检查器（**Object inspector**）和处理步骤里的设置，于是
一个层级、一种颜色、一个线宽、一个字号或一种窗函数，都可以经一次可预检的批
处理应用到整个项目：

| 工具 | 作用 |
| --- | --- |
| **Inspect a property** | 读取当前值、默认值，以及该参数允许的范围 |
| **Set a property** | 写入一个值 |
| **Reset a property** | 按当前数据重新推导该值，与面板上的重置按钮一致 |

在 **Parameters (JSON)** 中用 id 指定参数：**Set a property** 用
`{"key": "series.contour.count", "value": 12}`，另外两个用
`{"key": "series.contour.count"}`。

这三个工具够得到各面板能编辑的一切设置：对象与序列的样式、处理步骤的参数、
文档与画布的设置，以及应用偏好。该指定哪个资源由参数本身决定——等高线和线条
参数在图对象上，切趾参数在数据集上，图形排印在文档上（列为
**Plotium document**），页面尺寸在画布上，偏好设置在应用上（列为
**Plotium application**）。

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
| **Stroke width**（**Line** 区域） | `series.line.stroke_width` | 0.05 到 10 pt；默认 0.5 |
| **Tick-label size**（**Figure typography** 区域） | `document.figure.typography.tick_pt` | 1 到 72 |

应用偏好设置同样用这三个工具。对于
`settings.appearance.accent.color`，**Set a property** 接受 `"#rrggbb"`，把画布
强调色固定为该颜色；**Reset a property** 清除它，强调色重新跟随主题。

| 切趾步骤上的设置 | id | 取值 |
| --- | --- | --- |
| **Window** | `dataset.processing.apodization.kind` | `none`、`cosine_bell`、`exponential`、`gaussian` |
| **LB** | `dataset.processing.apodization.lb_hz` | −10000 到 10000 |
| **GB** | `dataset.processing.apodization.gb_hz` | 大于 0，最大 10000 |

各等高线参数的含义、以及每种锚定方式需要什么样的数据，见
[等高线层级](/zh-cn/guides/contour-levels/)；切趾各行的说明见
[数据处理](/zh-cn/guides/processing/)。

#### 勾选的目标会展开成什么

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

#### 为什么某个目标被跳过

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
进入运行记录，[命令行](/zh-cn/reference/cli/)写出的清单文件里也是同一份
内容。在窗口里，数值显示在逐部件结果下方的 **Result value (JSON)** 区域：
每个部件一份读数，包含当前值、默认值和它接受的范围。

#### 一条读数包含什么

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

## External Inputs

运行一个从磁盘文件开始的已保存工作流——例如：导入文件夹里的每个实验、
应用一个处理配方、逐个导出图形。点击 **Open workflow…** 载入，
**Validate** 校验，再点 **Confirm and run workflow**。进度会逐步显示，
较长的运行可以取消。

## 什么会被记录下来

一次工作流运行会留下一份记录：工作流本身及其哈希、Plotium 版本，以及每一步
作用的目标、参数和结果。在这里运行，记录保存在项目中；用
[命令行](/zh-cn/reference/cli/)运行，同样的记录写成文件。**Current
Project** 里的批处理不会留下这种记录——它就是一次普通编辑，在文档里，也在
撤销栈上。

原生工作流中向项目之外写文件的操作（例如导出图形）还会列在 **Help → Operation and
Diagnostic History** 中，附带每个文件的路径、大小和 SHA-256 校验和。中途
失败的导出也会列出此前已经写出的文件，落到磁盘上的内容不会无从追查。

工作流文件是纯 JSON，也可以脱离桌面应用运行——见
[命令行](/zh-cn/reference/cli/)，它以无界面方式执行同样的工作流。工作流
与运行记录文件本身的说明见[文件格式](/zh-cn/reference/file-formats/)。

## 本机 agent 连接（MCP）

连接 AI 客户端后，你可以让它查看当前项目、检查图形，或在 Plotium 中完成支持的
编辑操作。MCP 让客户端能够调用 Plotium 的工具；你仍在 Codex、Claude Code 或
Claude Desktop 中与 AI 对话。

### 连接 AI 客户端

请保留完整的 Plotium 发行目录，包括 `plotium-mcp`（Windows 下为
`plotium-mcp.exe`）。接入前，先安装你要使用的 AI 客户端。

1. 在 Plotium 中打开或新建项目。
2. 打开 **Preferences → AI connections (MCP)**。也可以点击 Ribbon 中操作历史
   旁的机器人图标，或使用菜单 **File → AI connections (MCP)…**。
3. 开启 **Enable MCP**。在 **Access** 中，选择 **Read only**（只读）可先查看项目，
   选择 **Read and modify**（读取和修改）则允许支持的编辑。默认允许读取和修改。
4. 按下表添加客户端。

| 客户端 | 在 Plotium 中操作 | 在客户端中操作 |
| --- | --- | --- |
| Codex | 在 Codex 卡片点击 **Add** | 如有需要，重载 MCP 配置或重启客户端 |
| Claude Code | 在 Claude Code 卡片点击 **Add** | 如有需要，重载 MCP 配置或重启客户端 |
| Claude Desktop（macOS 和 Windows） | 点击 **Add to Claude Desktop…** | 确认安装扩展；如有需要，重载或重启 |

请自行启动 AI 客户端，Plotium 不会代为启动。Codex 和 Claude Code 卡片上的
**Configured** 表示已添加连接配置。要确认客户端确实能访问 Plotium，请发送：

> 请使用 Plotium 列出当前项目中的数据集和图形，不要修改任何内容。

客户端应能描述你的项目，Plotium 中也应显示最近的客户端活动。连接活跃时，机器人
图标会变为绿色。仅打开 Claude Desktop 扩展安装包，并不代表连接已经成功。

### 试着完成一个任务

在 AI 客户端中直接用自然语言描述任务，例如：

- **检查图形：**“请使用 Plotium 预览我的画布，描述曲线和标签，不要修改内容。”
  客户端需要支持接收图像。
- **只诊断不修改：**“按 180 × 90 mm 定位文字溢出和图例潜在遮挡，说明覆盖限制并实际查看原生 PNG；保留有意重叠与手动轴范围。”见[图面诊断](/zh-cn/reference/mcp/#按输出尺寸诊断图面几何)。
- **调整线宽：**“找到名为 Current 的曲线，查看当前线宽，再仅将这条曲线改为
  1.5 pt。”需要 **Read and modify** 权限。
- **编排图形：**“将页面设为 180 × 90 mm，这两个面板横排，边距 4 mm、间距
  5 mm，标 a/b，加入说明文字和本地 PNG/JPEG，再显示预览；其他面板不动。”
  明确目标面板与图片绝对路径。图、文字和图片仍可在 Plotium 分别编辑，Undo
  可恢复整批版式修改。
- **更新数据：**“只改源表这个单元格，显示哪些派生表已失效，再刷新这张表的已存配方；保留我的颜色、轴范围和布局。”补丁与刷新可分别 Undo。
- **创建图形：**“导入 `/absolute/path/experiment.csv`，用时间列作 X、电流列作 Y，
  单位分别为 s 和 mA，创建可编辑折线图并显示预览。”请换成实际文件的绝对路径，
  并说明要使用的列和单位。

连接支持导入 CSV/TSV 表格、读取表格数据、查看项目结构、预览画布、创建折线图或
散点图及多面板、设置页面／面板几何、对齐／分布／网格排列明确面板、添加可编辑文字和本地 PNG/JPEG、更换曲线的数据列、使用受支持的原生计划派生表格、修改已有单元格、显式刷新已存配方，以及重命名数据集、画布或对象。显示设置可精确修改曲线颜色、线宽/线型、符号、可见性和图例文字；轴标题、显示单位及自动/手动范围；单图刻度、轴标题和图例字号，以及已有等高线和文档字号控制。
处理参数、受支持表格处理之外的拟合等计算、其他格式导入尚未开放给此连接，请在 Plotium 界面
中完成。Plotium 不提供内置聊天窗口。

保存时可说：“把当前桌面项目保存到 `/absolute/path/figure.plotium`。”
在 Plotium 的 **Allow AI to save this project?** 提示中核对路径；已有文件还须
明确确认替换。保存使用独立的一次性授权，不能 Undo；Agent 收到完成回执才算
保存结束，之后的编辑和读取仍可能未保存。

### 只删除明确指定的内容

可以要求：“删除这段重复说明和这条明确的冗余 Series，保留另一条同名曲线、单位和我的手动
范围。先列出删除清单，再执行并显示实际 PNG。”Agent 必须发现稳定 ID，不按名称或当前选择
定位。一次 Undo 恢复整批；空父容器和来源数据保留，删除末条 Series 会被拒绝。
空 panel 可显式删除，非空 panel 不能经 MCP 级联删除。
详见[精确删除及拒绝规则](/zh-cn/reference/mcp/#精确删除内容或指定-series)。

### 看过候选后再应用局部修正

可以要求：“自行发现明确对象，读取诊断和实际 PNG，提出最小局部修正。**每个**计划都
先显示并实际读取候选 PNG，再明确 execute 同一计划；执行后读取真实 PNG 和 fresh 诊断。
保留数据、单位、手动范围、其他同名对象及有意裁切／重叠，不要缩字或隐藏数据消除告警。”

候选支持原生 guide 编辑、内容几何、原位换图、文字更新和精确删除。它需要 **Read and
modify**，但不改 authored state、Undo/Redo、dirty 或 edit generation，也不追加项目审计记录。如果你在
Agent 审阅候选时编辑项目，旧计划必须重做。一次 execute 仍是一个原生 Undo 步骤。
详见[支持矩阵与候选元数据](/zh-cn/reference/mcp/#提交前读取计划候选)。

### 交付可编辑项目和成品

先让 Agent 导入 CSV/TSV、检查明确单位，再创建折线加误差／散点双面板，添加
文字和 PNG/JPEG。请它实际看预览后只修正指定局部；需要派生曲线时，先计算再
追加到已有图。你可在 GUI 改颜色、轴范围和面板位置，再让 Agent 精确补丁数据、
检查派生失效并显式刷新；用 Undo/Redo 核对补丁和刷新各自保留人工设计。

保存后在 GUI 重开 `.plotium`，确认曲线、文字、图片仍可编辑。再说：“将这张
180 × 90 mm 画布导出为 `/absolute/path/figure.svg`、同名 PDF 和 300 DPI PNG，
不要覆盖已有文件。”在 **Allow AI to export this figure?** 核对路径并批准；
替换已有文件另需勾选确认。Agent 应逐文件查完成回执、实际打开成品检查，而不把
排队当成功。PNG 应为 2126 × 1063 像素。导出不替代项目，也不使项目变为已保存。
MCP 导出结果保存在当前进程的导出回执中；重连先查回执，不重放未知调用。

### 只修改图例，保留数据

可以说：“只把左面板的图例移到可用左下角，保持纵向，标题为 Samples。保留全部
曲线、单位和手动范围，显示修改前后预览；放不下时如实报告。”Agent 应按完整对象
ID 区分同名 plot，并实际查看原生 PNG。之后仍可在 GUI 拖动图例，双击恢复自动位置；
保存前检查 Undo/Redo。[坐标与重置规则](/zh-cn/reference/mcp/#精确放置单图图例)见接口参考。

### 控制权限与检查修改

**Access** 对使用此 Plotium 连接的所有客户端生效，也适用于之后打开的项目。
Plotium 会在重启后记住权限和 **Enable MCP** 的选择。改为 **Read only** 会阻止
后续编辑，包括已准备但尚未应用的修改；仍可查看数据和预览图形。

支持的修改会立即显示在桌面中，可使用 Plotium 的 **Undo**（撤销）和 **Redo**
（重做）。普通查询、预览和诊断不改变项目审计记录、dirty、edit generation 或 automation revision。

关闭 **Enable MCP** 会立即停止访问，并保留客户端配置。关闭设置或切换项目不会
断开连接；退出 Plotium 则停止访问。重启 Plotium 或重新开启 MCP 后，可继续使用
原来的 AI 会话，无需再次添加客户端。切换项目后，请先让 AI 重新查看当前项目，
再继续编辑。

MCP 工具注解描述工具副作用，也可供客户端审批策略参考；它们不授予 Plotium 权限，
也不取消桌面文件确认。混合的规划／执行入口采用保守提示。客户端审批应限定于需要的
工具，并遵循[注解与授权指引](/zh-cn/reference/mcp/#工具-annotations-与客户端审批)。

### 看懂机器人图标

将鼠标停在机器人图标上，可查看项目、权限和最近活动；点击可打开连接设置。
Ribbon 折叠后仍可使用此图标。

| 外观 | 状态 | 你可以做什么 |
| --- | --- | --- |
| 灰色、短斜线 | Off（已关闭） | 需要连接时开启 **Enable MCP** |
| 蓝色、空心点 | Starting / Ready（启动中／就绪） | 等待启动，或从客户端发送请求 |
| 绿色、实心点 | Active（活跃） | 最近 30 秒有客户端活动；不代表任务已完成 |
| 橙色、感叹号 | Needs attention（需要处理） | 打开连接设置，查看错误并按提示处理 |

### 连接不成功时

| 遇到的情况 | 建议操作 |
| --- | --- |
| Plotium 找不到 Codex 或 Claude Code | 通过 **Installation help** 安装，或用 **Choose executable…** 选择客户端程序，再点击 **Add** |
| 错误提示缺少 Node.js | 检查客户端的 Node.js 安装，或选择独立的客户端可执行文件后重试 |
| 已有另一个名为 `plotium` 的服务器 | 先检查现有配置；确定要替换时点击 **Back up and replace this entry**。Plotium 会备份原条目，不改动其他服务器 |
| 显示 **Configured**，但没有客户端活动 | 重载客户端的 MCP 配置或重启客户端，再发送上面的首次请求；检查客户端是否禁用或阻止了该服务器 |
| 客户端提示先打开项目 | 在 Plotium 中打开或新建项目，再让客户端重新查看 |
| 提示连接由另一个 Plotium 实例提供 | 使用提供连接的实例，或退出该实例，让当前实例接入。切换窗口焦点不会切换 AI 访问的项目 |
| Windows 更新时提示连接器被占用 | 退出正在使用 Plotium 的 AI 客户端，再在连接设置中点击 **Retry** |
| 缺少连接器或 Claude Desktop 扩展 | 重新解压完整 Plotium 发行包，将其中的文件保留在一起 |

启动失败后会自动重试。连接设置会显示最近操作和错误；解决原因后，如页面提供
**Retry**，可点击重试。编辑过程中断线时，先检查项目再让 AI 重试：修改可能已经
完成，再次执行可能产生重复内容。

### 其他客户端与技术说明

使用其他本机 MCP 客户端时，展开 **Manual configuration and diagnostics**，点击
**Copy stable stdio configuration**，将复制的配置用于客户端。此连接适用于同一台
电脑、同一用户账户下的客户端，不支持远程连接或 Windows 与 WSL 之间的跨环境访问。

手动注册命令、工具参数、调用示例和限制见 [MCP 接口参考](/zh-cn/reference/mcp/)。
