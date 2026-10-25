---
title: Plotium 文件
description: Plotium 自己写出的文件：项目、处理配方、样式、拟合模型、工作流、运行记录和表格 schema 配套文件。
---

本页介绍 Plotium 自己创建的文件。Plotium 能读取的仪器与表格格式见
[支持的格式](/zh-cn/guides/supported-formats/)。

## `.plotium` 项目

`.plotium` 文件把整个会话保存在一个文件里：导入的数据、处理管线、分析结
果、画板排版、嵌入的图片、已保存的工作流，以及每次自动化运行的记录。因此它是
自包含的——复制到另一台机器或发给同事，打开时与你保存时完全一致，不需要记着
带任何附属文件。与使用任何分析软件一样，保留原始仪器数据仍是好习惯。

**Preferences → General → Project backup copies** 会在项目旁以隐藏文件
保留历史保存，误覆盖后可以恢复。
自动崩溃恢复检查点是单独的内部文件；它们只在出现新编辑后更新，并不是可供
分享的项目副本。见[保存项目](/zh-cn/guides/organizing-data/#保存项目)。

对于同核 2D 审核，项目会保存交叉峰标记、互为对应峰的配对关系，以及每个
标记的审核状态。固定的光标位置与对称审核结果不会保存。

对于伪 2D 的 DOSY，项目还会保存逐列图和 ILT 图本身、各自构建时所用的 ILT
设置、**Show** 与 **DOSY method** 的选择，以及关于各图如何得出的足够信息——
重新打开项目时，若某张图与项目重建出的数据不再一致，Plotium 就据此发出警告。
等高线绘图本身不保存；项目打开时，Plotium 会根据保存的图重新绘制。

添加到页面上的图片保存在项目内部。复制图片不会重复保存源图片。项目打开时，
Plotium 会校验每张嵌入图片；缺失或损坏的图片会变成可替换的占位项。见
[图中的图片](/zh-cn/guides/images/)。

## `.plotiumproc` 处理配方

`.plotiumproc` 文件保存一条处理管线，不含任何数据——配方保存一次即可在
任何机器上应用到一整个系列的同类实验。见
[配方与模板](/zh-cn/guides/templates/)。

命名的处理模板保存在 Plotium [配置目录](/zh-cn/getting-started/installation/#plotium-的设置存放在哪)的
`templates` 文件夹中。

## `.plotiumstyle` 画布样式

**Save Canvas Style…** 会把当前画布的外观写成配置目录 `styles` 文件夹中的一个
`.plotiumstyle` 文件。把该文件复制到另一台电脑的同一文件夹，即可在那里使用这个
样式。见[字体、主题与样式](/zh-cn/guides/themes-and-styles/)。

## 自定义拟合模型

自定义拟合模型在 Curve Fit 编辑器中用 `.plotiumfit` 模型语言编写，统一保存在
模型库中，即配置目录里的 `fit-models.json`。模型只是纯数据，不会运行代码。见
[自定义拟合模型](/zh-cn/guides/custom-models/)。

## 工作流与运行记录文件

[自动化](/zh-cn/guides/automation/)工作流文件是一次批处理运行的 JSON
描述——导入哪些文件、应用什么、导出什么。运行后会生成一份运行记录
（manifest）文件，写明每个数据集实际发生了什么，批处理结果因此始终可
追溯。二者都是纯 JSON，均见[命令行](/zh-cn/reference/cli/)。

工作流不是配方：配方保存一条处理管线，而工作流描述一整次运行，可以把
配方作为其中一个步骤引用。

## 表格 schema 配套文件

Plotium 把表格导出为 CSV 或 TSV 时，会在旁边写入一个 `.plotium-schema.json`
文件，记录列类型、单位和误差棒关系。导出 XLSX 时同样的信息保存在隐藏工作表中，
复制 TSV 时（Windows 上）则放到剪贴板。再次导入该表格时会恢复这些信息；见
[Plotium 导出的表格](/zh-cn/guides/importing-data/#plotium-导出的表格)。
