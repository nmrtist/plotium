---
title: 故障排查
description: 导入、分析、导出或更新不符合预期时的检查清单。
---

## 导入

**图片无法导入。** Plotium 检查文件实际内容，而不只看扩展名；可接受的格式以及
动画图片、多页图片的导入命令见[图中的图片](/zh-cn/guides/images/)。打开 **Operation and
Diagnostic History** 可查看文件名、检测到的格式、失败环节和原因。导入失败不会改动项目。

**ABF 文件打不开。** ABF1 文件和压缩的 ABF2 数据目前不支持——请在采集软件中
重新导出为未压缩的 ABF2。见[电生理（ABF2）](/zh-cn/guides/supported-formats/#电生理abf2)。

**数值列被导入成了文本。** Plotium 只对无歧义的值自动定型，因此区域性格式
（逗号小数、地区日期）和数字文本混排的列保持为文本。查看 **Review table import**
对话框的诊断信息，修正源文件，或导入后再转换该列。见
[列类型如何确定](/zh-cn/guides/importing-data/#列类型如何确定)。

**Excel 表格里的单元格导入后是空的。** Plotium 读取 Excel 为每个公式缓存
的值，但从不重新计算公式。在 Excel 里打开工作簿保存一次即可刷新缓存值。

## 处理与分析

**谱峰拟合提示组分过多而拒绝运行。** 单次拟合最多处理 24 个组分。缩小
拟合范围，或删除一些峰标记。

**DOSY 图谱构建被取消了。** 图谱构建期间更改数据集的处理或扩散输入会取消构建，
因为结果将不再与输入对应。完成修改后重新构建即可。
参见 [DOSY 扩散分析](/zh-cn/guides/dosy/)。

**统计或时间窗统计返回错误而不是数值。** Plotium 拒绝从空窗口、非有限采
样或被排除的单元格中编造数值；消息会说明问题所在。调整窗口，或确认它
提议的排除。

## 导出

**导出预检标记了我的图。** 字号或线宽违反所选期刊预设。在画板上修正
——**Figure Typography…** 可一次设定全部坐标轴文字尺寸——或改选符合
预期尺寸的预设。

## 项目与更新

**我不小心覆盖了项目。** 若 **Preferences → General → Project backup
copies** 已开启，历史保存以隐藏文件保留在项目文件旁。

**收不到更新。** 失败的后台检查（例如离线时）按设计保持静默。在
**Preferences → General** 中用 **Check now** 获得明确结果，并检查更新
通道——切换到更稳定的通道不会降级，而是等待该通道的下一个版本。

**macOS 应用无法安装更新。** 推出安装 DMG，从 Applications 或可写的本地
文件夹运行 Plotium。若系统询问，请允许更新应用。退出后的安装错误会在下次
启动的 **偏好设置 → 通用** 中显示，可用 **Check now** 重试，或从新
DMG 手动安装。见[更新](/zh-cn/reference/updates/)。

**打开项目后出现缺失图片占位项。** 嵌入资源缺失、损坏，或未通过完整性校验。
请查看 **Operation and Diagnostic History**，选中占位项并使用 **Replace Image…**。
Plotium 会保留页面供继续编辑，但阻止常规保存和导出，避免损坏状态被静默发布；若只需
审阅稿，可在导出对话框中明确生成带标签的占位图。见
[图片缺失或损坏](/zh-cn/guides/images/#图片缺失或损坏)。

## 报告问题

以上都无法解决时，请在
[GitHub issue 跟踪器](https://github.com/nmrtist/plotium/issues)报告，附上
Plotium 版本（见 **Preferences → General**）、平台，以及（可能时）能复现
问题的文件。崩溃后请附上 `crashes` 文件夹中最新的崩溃报告；其他问题请附上旁边
`logs` 文件夹中最新的会话日志。macOS 上两者都在 `~/Library/Application Support/plotium`
中；其他平台见[报告问题](/zh-cn/reference/reporting-problems/)。
