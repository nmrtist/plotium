---
title: 质谱
description: 浏览 LC–MS 数据，提取质谱和离子色谱图，选择色谱通道。
---

Waters MassLynx RAW、SCIEX legacy WIFF 和 mzML 都会打开为同一种 LC–MS 数据集。
打开 Waters 数据时，选择 **Open Folder…** 并选中 `.raw` 目录，或把该目录拖到窗口中。
本页讲导入之后的操作；各导入器读取哪些内容，见
[支持的格式](/zh-cn/guides/supported-formats/#质谱)。

Plotium 不提供 LC–MS 处理流程。导入的数据、当前采集流、检测器通道、提取质谱、
提取离子色谱图和页面排版都会保存在 `.plotium` 项目中。

## 初始页面

初始页面显示当前采集流的总离子流图（TIC）。Waters 数据含光学检测器数据时，
UV 通道会放在 TIC 上方，并共享保留时间轴。多个 UV 通道会叠加显示，图例使用
文件中存储的波长，例如 `214 nm`。若要隐藏、移动或调整该图例的排版，请选中
UV 图并使用对象检查器的 **Legend & scales**。

## 预览和提取质谱

选中 LC–MS 数据集，然后在 **Analyze（分析）** 标签中选择 **Extract Mass
Spectrum**。Plotium 会在右侧栏打开 **Dataset tools → Mass spectrometry**，并启用
保留时间范围选择。

单击 TIC 或 UV 色谱图，会在 **Scan preview** 中显示保留时间最近的 MS 扫描。
预览会标明保留时间和原始扫描编号，但不会加入页面，也不会保存为结果；重新打开
项目后会被清除。选择 **Extract current scan** 可把该扫描作为棒状谱加入页面。

若要从时间窗提取，请选择 **Method**，启用 **Select range**，然后在 TIC 或 UV
色谱图上拖出范围。选择 **Extract spectrum**，即可把峰顶扫描、最近扫描、平均谱
或求和谱加入页面。每张提取谱都会记录功能、时间范围和提取方式；移动预览游标
不会改变它，并且它会列在数据浏览器的 **Analysis** 下。

## 提取离子色谱图（XIC）

要创建提取离子色谱图（XIC，也称 EIC），请打开 **Dataset tools → Mass
spectrometry** 并选择 **Select m/z range**。在当前质谱图上拖出 m/z 区间，
确认显示的 m/z 区间和采集流，然后选择 **Extract ion chromatogram**。

生成的折线图会在每次扫描的保留时间（分钟）处显示一个点。每个点的值是
所选 m/z 区间内的强度之和，并包含两个端点。该结果会被保存；移动扫描预览或
选择其他采集流都不会改变它。XIC 提取不进行色谱峰积分，也不计算峰面积。

## 切换采集流

数据中含多个受支持的 MS 功能或 experiment 时，请使用 **Dataset tools → Mass
spectrometry** 中的 **Acquisition stream**。Waters 数据的初始采集流是第一个
非参考 MS 功能。切换采集流、提取质谱和提取 XIC 都可通过标准的 Edit 命令撤销
和重做。

## 浏览色谱通道

对于包含大量色谱通道的数据，请通过 **Extract Mass Spectrum** 或命令面板打开
**Dataset tools → Mass spectrometry**。**Chromatogram channels** 浏览器会显示稳定的
数量和顺序，其中 TIC、BPC 排在 SIM/SRM transition 之前。搜索可匹配通道名称和
native ID；对于结构化 transition，还可按 precursor m/z、product m/z、极性、
碰撞能量和活化方法筛选。数值输入支持精确值、`>=400` 之类的比较式，以及
`400..500` 之类的区间。

选择一行会把当前 LC–MS 色谱图中的曲线替换为该单一通道；质谱图绝不会被重新绑定。
该操作可以撤销，并随页面保存到项目中。只有色谱图的采集也使用同一浏览器。没有
可绘制通道、没有结构化 transition 信息或当前筛选无结果时，面板都会给出明确提示。
大型 scheduled-MRM 数据中列表也能保持流畅。

## TIC 与 BPC 的来源

文件直接提供的 TIC 或 BPC 色谱图会标记为源色谱图，并在其所属采集流中优先使用。
缺少该通道时，Plotium 会先使用每张谱图在文件中提供的 TIC 或 base-peak 摘要；
没有摘要时，才从强度数组派生 TIC，或从最大的非负峰派生 BPC。因此同一次采集中
可能同时存在文件摘要与数组派生点。多采集流数据中，未归属任何采集流的源色谱图
会作为独立通道保留，不会被猜测成某个采集流的 TIC 或 BPC。field 元数据和
Scientific Summary 会显示实际使用的来源。
