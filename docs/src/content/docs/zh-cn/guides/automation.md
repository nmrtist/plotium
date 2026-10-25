---
title: 自动化
description: 把同一个操作一次应用到多个数据集或图，运行只读的科学脚本，或运行已保存的工作流。
---

当同一个操作需要作用于一整个系列的实验时，自动化用一次可预检的批处理
取代逐个数据集的点击。

选择**文件 → Automation…**（命令面板中也可调用）。窗口分两个标签页：
**Current Project** 作用于已打开的内容，**External Inputs** 运行从磁盘文件开始的
已保存工作流。

如果想用日常语言让 AI 客户端完成这类工作，参见[与 AI 协作](/zh-cn/ai/working-with-ai/)。

## Current Project

搜索并勾选需要的数据集或图，或点击 **Current selection** 勾选工作区里当前选中的
内容。然后在列表下方选择两种模式之一：**Scientific Script** 或 **Registered Tool**。

### 对勾选的内容应用工具

在 **Registered Tool** 中从列表选择一个工具。窗口会显示工具的作用和它接受的参数；
在 **Parameters (JSON)** 中填写参数。点击 **Preflight** 预览会影响哪些目标、哪些会
被跳过，再点 **Confirm and execute**。整批操作合并为一次 **Undo automation** 撤销。

其中三个工具能改到对象检查器（**Object inspector**）、处理步骤和偏好设置里的设置，
于是一个等高线层级、一种颜色、一个线宽、一个字号或一种窗函数，都可以经一次批处理
应用到整个项目：

| 工具 | 作用 |
| --- | --- |
| **Inspect a property** | 读取当前值、默认值，以及该设置允许的范围 |
| **Set a property** | 写入一个值 |
| **Reset a property** | 按当前数据重新推导该值，与面板上的重置按钮一致 |

用 ID 指定设置。[设置 ID](/zh-cn/reference/settings-ids/#读取和写入设置) 列出各个 ID、
参数格式、每个设置位于哪类对象上，以及结果如何报告被跳过的目标。

超出范围的值在 **Preflight** 阶段、也就是确认之前就会被拒绝。过了这一步，列出的
目标要么全部生效，要么全部不变。已经是这个值的目标报告为跳过；如果所有目标都已是
这个值，就不写入任何内容，也不增加撤销步骤。

### 运行科学脚本

科学脚本读取 LC–MS 数据，返回自己的分析摘要，例如在每条色谱中找到的峰。它不能
修改项目，不能打开其他文件，也不能访问网络。

1. 勾选要分析的 LC–MS 数据集。勾选画布等于勾选它显示的数据集。
2. 选择 **Scientific Script**，点击 **Open script…** 并选择 `.plotiumscript` 文件。
   窗口会显示脚本将处理几个已勾选的 LC–MS 数据集。
3. 点击 **Run Scientific Script**。数据集逐个处理，并显示进度条。
4. 每个数据集得到一张结果卡片。脚本返回了 `summary` 时，其中各字段显示为表格；
   **Technical details** 显示完整结果。脚本处理失败的数据集显示错误信息，其余
   数据集照常运行。
5. 点击 **Save results…** 把全部结果写入一个 JSON 文件。

命令面板中的 **Run Scientific Script** 会打开这个模式，并对已勾选的数据集运行所选
脚本。只有 LC–MS 数据集（例如 mzML 和 Waters MassLynx 导入的数据）会交给脚本；
没有加载这类数据时该命令不可用。

脚本使用 [Rhai](https://rhai.rs/) 语言编写。`load_input()` 以 map 形式返回数据集：
`source` 和 `instrument`、文件中存储的 `chromatograms`、总离子流和基峰的
`stream_chromatograms`、`scans` 下每次扫描的摘要，Waters 数据还在 `lc_method` 中
给出液相方法。保留时间以分钟为单位。脚本最后一个表达式就是它的结果。可用的辅助
函数：

| 函数 | 返回 |
| --- | --- |
| `moving_average(values, width)` | 用 `width` 个点的居中窗口平滑后的数值 |
| `rolling_percentile(values, width, q)` | 居中窗口内的 `q` 分位数（0–1），可用作基线 |
| `estimate_noise(values)` | 该曲线的噪声估计 |
| `detect_peaks(x, y, options)` | 峰列表，每个峰含 `index`、`x`、`y` 和 `prominence`；`options` 可设 `min_height`、`min_prominence`、`min_spacing` 和 `max_count` |
| `format_number(value, decimals)` | 最多 12 位小数的数字文本 |

例如，下面的脚本报告第一条总离子流或基峰色谱中最显著的五个峰：

```rust
let input = load_input();
let trace = input.stream_chromatograms[0];
let peaks = detect_peaks(trace.time_min, trace.values, #{ min_prominence: 1000.0, max_count: 5 });
#{
    summary: #{ "Peaks found": peaks.len() },
    peaks: peaks
}
```

脚本运行过久、生成过大的数组或文本，或返回超过 10 MB 的结果时，会报错停止。
结果保留在窗口中，直到再次运行脚本；以后还需要的话请先保存。

## External Inputs

运行一个从磁盘文件开始的已保存工作流——例如：导入文件夹里的每个实验、
应用一个处理配方、逐个导出图形。点击 **Open workflow…** 载入，
**Validate** 校验，再点 **Confirm and run workflow**。进度会逐步显示，
较长的运行可以取消。

窗口只运行工作流文件，不创建它们。工作流是普通 JSON 文件，格式见
[工作流文件结构](/zh-cn/reference/cli/#工作流文件结构)。

## 什么会被记录下来

一次工作流运行会留下一份记录：工作流本身及其哈希、Plotium 版本，以及每一步
作用的目标、参数和结果。在这里运行，记录保存在项目中；用
[命令行](/zh-cn/reference/cli/)运行，同样的记录写成文件。**Current
Project** 里的批处理不会留下这种记录——它就是一次普通编辑，在文档里，也在
撤销栈上。科学脚本不修改任何内容，也不留下记录。

原生工作流中向项目之外写文件的操作（例如导出图形）还会列在 **Help → Operation and
Diagnostic History** 中，附带每个文件的路径、大小和 SHA-256 校验和。中途
失败的导出也会列出此前已经写出的文件，落到磁盘上的内容不会无从追查。

工作流文件也可以脱离桌面应用运行，见[命令行](/zh-cn/reference/cli/)。工作流
与运行记录文件本身的说明见[文件格式](/zh-cn/reference/file-formats/)。
