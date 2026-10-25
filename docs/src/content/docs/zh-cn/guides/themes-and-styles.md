---
title: 字体、主题与样式
description: 为所有图设定字体和字号，套用 Nature 等画布主题，并保存可复用的样式。
---

Plotium 自动按印刷习惯设定图形样式。新数据集默认使用 89 × 60 mm 单栏画布：单个图
占满画布时，屏幕上看到的字号就是期刊印出的字号；多分图的组合图之后再拼合到更宽的
页面上，各分图保持原始尺寸。

## 字体

图中文字默认在画布和所有导出格式中都使用 Arial。没有 Arial 时改用 Helvetica 或
Liberation Sans 等字宽相同的无衬线字体。SVG 文件写明字体，用矢量软件打开也保持
Nature 系列期刊要求的字体。

**Figure typography** 分区中的 **Font** 切换所有图内文字的字体——刻度标签、轴标题、
图标题、图例、色标和图内标注——可选 *Arial*（默认）、*Helvetica* 或
*Times New Roman*。Helvetica 在 macOS 上用 Helvetica 绘制；Windows 和 Linux 上改用
字宽相同的 Arial 或 Liberation Sans，SVG 文件仍把 Helvetica 写在首位。没有
Times New Roman 时改用 Liberation Serif 等字宽相同的衬线字体。高级设置中的
**Font weight** 把这些文字设为 *Regular*（常规）或 *Bold*（粗体）。画布上的文本框和
分图字母仍保持各自的 Arial 样式。这两项设置随主题和已保存的样式一起保存和套用；
内置主题使用 Arial Regular。

## 图形排印

Figure Ribbon 选项卡 **Style** 分组中的 **Figure typography** 会打开文档级排版分区；
命令面板中的 **Figure Typography…** 以独立窗口显示同一分区，并多出
**Reset to defaults** 按钮。两者都一次设定文档内所有图的文字尺寸（刻度标签、轴标题、
图标题与图例），单位为绝对磅值——这是文档级样式，缩放分图不会改变字号。各字号取值
1 到 72 pt；图例默认为 7 pt。

同一分区的高级设置还统一设定所有图的坐标轴线条和刻度，见
[刻度线与坐标轴线](/zh-cn/guides/axes/#刻度线与坐标轴线)。

对象检查器的 **Figure typography** 区域中的 **Tick-label size** 就是同一个刻度标签
字号，并提供 **Legend size** 与 **Legend text color**。在两处中任何一处修改，改的都是
文档里的同一个值。它属于文档而不属于某个图，因此无论当前选中什么（包括什么都没选中）
都会显示。

## 画布主题

要给当前画布套用主题，在 **Figure** 选项卡的 **Style** 组中选择 **Paper**、**Nature**、
**Dark** 或 **Vibrant**，或在 **View ▸ Canvas Theme** 中选择。套用主题是一个可撤销的步骤。

画布主题携带配套字号——例如 Presentation Dark 主题会放大坐标轴文字以适合投影演示。

**Nature** 主题遵循 Nature Portfolio 终稿作图规范：文字 7 pt，线宽按字号缩放——
坐标轴、刻度、柱描边以及新建的参考线和箭头为 0.3 pt，折线系列为 0.5 pt——刻度朝外
3 pt，符号 3 pt 且无描边，使用对色盲友好的 Okabe–Ito 配色。手动加上的符号描边会保留。
这些尺寸以 Nature 栏宽为准，套用到宽度不符合任一 Nature 尺寸预设（89、136 或
183 mm）的画布时，状态栏会附带提示列出这些宽度；画布尺寸保持不变。宽度已符合某个
Nature 尺寸、但尚未设定尺寸预设的画布，会随之采用该 Nature 预设，导出检查因此按
Nature 的下限（文字 5 pt、线条 0.25 pt）判断。

之后切换到不设定线宽和符号大小的主题时，这些线宽和符号大小会保留。

## 手动设置的值

手动设置的值（在对象检查器中设置、在控件上滚动调整或拖动图例）会替你保留。任何主题
都不会改动它们：曲线和符号颜色、线宽、符号大小和描边、文字和形状颜色以及图形排版。
状态栏会说明保留了几处。连接到 Plotium 的 AI 客户端也会跳过它们，除非你要求它修改；
AI 设置的值不算作你的设置。重置该值（或双击拖动过的图例）后，它重新跟随主题。

## 已保存的样式

**Save Canvas Style…**（View ▸ Canvas Theme，或 Figure Ribbon 选项卡 Style 组中的
**Save Style**）把当前画布的外观保存为命名样式：背景、文字颜色与字号、图形排版
（包括字体和字重）、坐标轴和刻度线宽、曲线颜色及其符号填充色、描边和空心设置、线宽
和符号大小。**Canvas Styles…** 列出已保存的样式，可套用到当前画布或删除。套用样式是
一次可撤销的操作。要修改某个样式，先套用它，调整画布后再用同一名称保存。

样式以 `.plotiumstyle` 文件保存在 Plotium 配置目录的 `styles` 文件夹中，可以复制到
其他电脑使用：macOS 上位于 `~/Library/Application Support/plotium/styles`；其他平台见
[Plotium 的设置存放在哪](/zh-cn/getting-started/installation/#plotium-的设置存放在哪)。

## 使用 AI 客户端

也可以让连接到 Plotium 的 AI 客户端替你完成，例如：

> 给这张画布套用 Nature 主题，先给我看预览。保留我手动设置的颜色。

> 把我保存的样式 Group meeting 套用到这个项目的所有画布上。

你手动设置过的值会保留，除非你明确让 AI 改；每次修改都是一步 Undo。同意之前请先看
预览。参见[权限、审阅与撤销](/zh-cn/ai/control-and-review/)。
