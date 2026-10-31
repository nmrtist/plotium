---
title: 支持的格式
description: Plotium 可导入的文件格式，按仪器和数据类型列出各导入器读取的内容与限制。
---

Plotium 直接读取厂商 NMR、LC–MS、XRD、XPS、XAS、AFM 与电生理格式，无需任何转换步骤。
如何打开文件、查看导入进度，见[导入数据](/zh-cn/guides/importing-data/)。

## 总览

| 格式 | 扩展名 | 说明 |
| --- | --- | --- |
| JEOL Delta | `.jdf` | 原始与已处理的 1D、2D 及参数系列；实验性支持 |
| Bruker TopSpin | `fid` / `ser` / `pdata` | 原始及已处理的 1D、2D；NUS 为实验性支持 |
| Varian/Agilent VnmrJ | `.fid` 目录 | 原始 1D、`phase=[1,2]` 的 2D，或单参数变化系列 |
| JCAMP-DX | `.dx` / `.jdx` / `.jcamp` | 1D 频域 NMR 谱 |
| Waters MassLynx RAW | `.raw` 目录 | 已验证的低分辨率数据，包括 SQD2 数据 |
| SCIEX legacy WIFF | `.wiff` + `.wiff.scan` | 支持单样本与多样本 legacy 数据；两个文件必须放在一起 |
| mzML | `.mzML` | 使用 32 位或 64 位数组的质心/轮廓谱图及 TIC、BPC、SIM 或 SRM 色谱图 |
| Rigaku 粉末 XRD | `.rasx`、FI `.raw`、RAS_RAW `.txt` | 衍射图样、采集元数据，以及文件提供的衰减系数 |
| XAS 扫描 | XDI、NSLS XDAC、带列名的线站列式文本（扩展名不限） | 能量、探测器通道、样品和吸收边元数据 |
| VAMAS XPS | `.vms` | ISO 14976 `NORM` / `REGULAR` XPS 区块，支持多个测量位置与谱区 |
| CasaXPS 文本 | `.txt` | 含原始谱、背景、包络、组件和拟合参数的结构化八行头导出 |
| Bruker NanoScope AFM | `.spm` / `.pfc` | 图像、力曲线、Force Volume 与 PeakForce Capture 数据立方体 |
| Axon Binary Format 2 | `.abf` | int16/float32、多通道、多 sweep，以及文件内 DAC/epoch 刺激 |
| 表格数据 | `.csv`、`.tsv`、`.txt`、`.xlsx` | 保留列类型与空单元格；每个 XLSX 工作表或 Source Data 面板导入为独立数据表 |
| Origin 项目（实验性） | `.opj`、`.opju` | 两种经验证的 OPJ 配置中的工作表；不导入图形，`.opju` 仅作识别 |
| Zip 压缩包 | `.zip` | 打包的数据文件夹 |
| Plotium 项目 | `.plotium` | 完整项目：数据、处理与排版；见 [Plotium 文件](/zh-cn/reference/file-formats/) |

## NMR

Plotium 支持一维和二维 NMR，包括带参数轴的系列。请根据要使用的数据选择输入：

- **Bruker：**选择实验目录可读取原始数据，选择 `pdata` 内的谱文件可读取已处理谱。
  若目录中有多个无法区分的候选，请指定具体文件或处理目录。支持 States 二维数据
  （`FnMODE=4`）。已处理谱保留文件中提供的实部和虚部分量。
- **JEOL：**选择含有受支持的原始或已处理采集数据的 `.jdf` 文件。
- **JCAMP-DX：**支持 5.00 或 5.01 版普通 XYDATA 频谱，每个坐标对应一个强度值，
  坐标单位可为 Hz 或 ppm。不支持 LINK、NTUPLES 和峰表。

缺失的核种、频率参考或数字滤波延迟信息保持未知。Hz 谱可以直接显示，但部分分析
需要额外校准。滤波延迟未知的原始数据首先显示为 FID；查看未校正频谱的方法见
[群延迟校正](/zh-cn/guides/processing/#群延迟校正)，选择分析功能前请核对
[校准条件](/zh-cn/guides/processing/#分析前检查校准条件)。坐标未知或数据排列不受支持时，
导入会给出错误或诊断信息。

JEOL 与 Bruker NUS 为实验性支持，未覆盖所有仪器和采集设置。使用不熟悉的采集类型时，
请检查导入诊断，并与可信参考结果比较后再使用分析结果。二维 NUS 数据缺少采样表时，
见[补录缺失的 NMR 采样表](/zh-cn/guides/importing-data/#补录缺失的-nmr-采样表)。

DOSY、T1、T2 实验会根据采集参数自动识别，并获得专属的分析工具；参见
[伪 2D 分析](/zh-cn/guides/pseudo-2d/)和 [DOSY 扩散分析](/zh-cn/guides/dosy/)。
缺失的扩散参数可在导入后通过 **DOSY** 任务卡片补充。

保存 `.plotium` 项目会保留导入的 NMR 分量、采样顺序、来源信息、警告和处理设置。
重开项目时，Plotium 根据保存的设置重新计算频谱，无需原始厂商文件。

### Varian/Agilent VnmrJ

选择 **Open Folder…** 并选中 `.fid` 目录，或选择 **Open File…** 并选中目录内的
`fid` 文件。请将 `fid` 和 `procpar` 保持在同一目录中。

支持原始一维数据、仅改变一个非分组参数的系列，以及 `phase=[1,2]` 的二维数据。
样本格式可为大端字节序的 16 位整数、32 位整数或 32 位浮点数。不支持分组或多个
参数数组、其他 phase 顺序、Varian NUS、超过二维的数据和已处理谱。记录的维度或
分量排列与实际数据不符时，导入会停止。

## 质谱

三种 LC–MS 格式都导入为同一种数据集，使用相同的图表工作流。导入后如何提取质谱
和离子色谱图，见[质谱](/zh-cn/guides/mass-spectrometry/)。

### Waters MassLynx RAW

请打开或拖入 `.raw` 目录本身；其中的内部文件不会被当作独立数据集。Plotium 会导入
其中受支持的 MS 功能和光学检测器通道。温度、压力及其他可读取的辅助通道会保留在
数据集中，但默认不绘图。

Plotium 支持已使用 SQD2 数据验证的低分辨率 MassLynx 编码。如果必需的 MS 功能
使用其他编码，导入会停止并指出相应功能和仪器。如果其余数据可读，不受支持的
可选功能或参考功能会产生导入警告。

### SCIEX legacy WIFF

请打开或拖入 `.wiff` 文件，并将文件名末尾追加 `.scan` 的配套文件放在同一
目录，例如 `sample.wiff` 与 `sample.wiff.scan`；`.wiff.scan` 不会作为独立数据集
导入。无需安装 SCIEX Analyst、SCIEX OS 或 ProteoWizard。

Plotium 会导入原始 scan ID、保留时间、m/z 与强度数组、可用的 precursor 信息、
极性、仪器与采集开始时间元数据，以及已验证的 11 个 experiment 槽位各自的 TIC。
谱图按样本与 experiment 分成独立的 acquisition stream，并保留 cycle 顺序，包括
零 TIC 和空 DDA 槽位；不会跨 experiment 合并时间或插值。重复样本名不会合并，
会稳定显示为 `yjs_10ppm #1`、`yjs_10ppm #2` 等后缀。

目前仅支持已验证的 legacy 布局。缺少配套文件、容器无样本或 WIFF 布局未识别时，
Plotium 会明确拒绝，不会创建不完整的数据集。加密或较新的 `.wiff2` 与
`.timeseries.data` 不受支持；请先将这些采集转换为 mzML，再在 Plotium 中打开。

### mzML

打开或拖入 `.mzML` 文件。谱图按 MS 级别和极性分组；每张 scan 仍会保留文件中
可用的采集功能元数据，例如 mzML instrument configuration、preset scan
configuration 和 filter string。以秒或分钟记录的扫描时间都会换算为分钟显示。
Plotium 也会导入文件自带的 TIC、BPC、SIM 与 SRM 色谱图，包括只有色谱图而没有
scan 谱图的采集。文件提供时，会保留 transition 的 precursor/product m/z、极性、
碰撞能量和活化方法。

对于 MS2 及更高级谱图，Plotium 会分别保留 precursor 谱图引用、selected-ion m/z、
selected-ion 强度与电荷、隔离窗目标与上下偏移、碰撞能量和活化方法。因此，即使
DIA 没有 selected ion，其隔离窗目标也不会被误写成 selected-ion m/z。Plotium
为每张谱图保存一个 precursor 和一个 selected ion；若文件包含更多值，导入警告会
指出对应谱图，并说明只保留了第一个值。

导入器支持小端 32 位和 64 位浮点 m/z、时间与强度数组，可不压缩或使用 zlib
压缩。Numpress、大端数组以及缺少必需数组的谱图或色谱图会使导入停止并显示错误。

## XAS 扫描

线站文件的扩展名常是运行编号，例如 `.000`、`.0001`，所以 Plotium 按内容识别：
XDI 或 XDAC 文件头，或者列名中既有能量，又有 μ，或有 I0 加 It 或荧光信号。其他文本
仍按表格处理，`.csv` 和 `.tsv` 文件也一样。扩展名不是 `.000` 的运行编号文件，请在
打开对话框里选择 **All files**。导入文件夹时按内容查找这些文件。能量统一换算为 eV，
各行按能量排序，重复的能量只保留第一行。

样品边跳不小于 0.1 时使用透射；否则使用荧光，再其次使用文件自带的吸收列。
导入报告会列出被丢弃的行和无法完成的步骤，Data 列表显示其中最新的一条。数据集提供归一化 μ(E)、μ(E)、压平 μ(E)、k 加权 χ(k)、|χ(R)|
和参考通道，用 [Larch](https://xraypy.github.io/xraylarch/) 的算法按 Athena 默认参数计算。
扫描终点低于 k = 4 Å⁻¹ 时只做到归一化。

选中扫描后，在 **Process** 选项卡中选择 **Processing**。**XAS Processing** 卡片提供
信号通道、能量平移、E0、归一化区间、背景和傅里叶变换，并列出扫描未能完成的步骤。
新扫描使用 **Athena defaults**：Hanning 窗，从 k = 3 Å⁻¹ 到数据末端前 2 Å⁻¹；
**Larch defaults** 的变换范围是 k 0–20 Å⁻¹，用 Kaiser-Bessel 窗。后边窗口不足 300 eV 的短扫描，归一化区间由 Larch 自动选择。能量按钮把参考箔的吸收边平移到表值；没有参考通道、
参考通道看不到吸收边，或其在吸收边处的步长超过 1 eV 时不可用。固定的 E0 会随能量平移移动。

**Process** 选项卡中的 **Summary Figure** 会新增一个双栏页面，包含 E0 附近的 XANES、
k 加权 χ(k) 和 |χ(R)|。每个面板都和扫描保持关联，但 XANES 范围在生成页面时按 E0 确定。
数据导出把 μ(E)、参考、归一化、压平、背景、χ(k) 和 |χ(R)| 曲线写成列。
处理方案和模板暂不支持 XAS。

## Rigaku 粉末 XRD

有 `.rasx` 时请优先打开该文件。Plotium 会同时读取实测 2theta、强度、衰减系数，
以及仪器名称、X 射线靶材、Kalpha1 波长、管电压/电流、扫描步长和扫描速度。
初始页面显示 XRD pattern。要处理它，先选中该图样，再在 **Process** 选项卡中选择
**Processing**。打开的 **XRD Processing** 卡片提供可选的 SNIP 背景扣除、
Savitzky-Golay 平滑，以及按最大强度或积分面积归一化；这些设置与原始观测值
都会保存在 Plotium 项目中。[数据处理](/zh-cn/guides/processing/)中的 NMR 管线步骤
不适用于 XRD。

采用 `FI` 布局的 Rigaku `.raw`，以及文件头标明 `RAS_RAW` 的 profile `.txt`，
也会作为 XRD 打开。二进制读取器会保留扫描坐标、强度、X 射线靶材、Kalpha1
波长、管电压/电流和扫描速度。由于不同厂商会使用互不兼容的 `.raw` 格式，
Plotium 会明确拒绝其他文件签名。普通 `.txt` 仍进入表格导入预览，不会被静默改判。

## XPS

VAMAS `.vms` 支持限定为按内容签名识别、具有规则能量标尺的 ISO 14976
`NORM` / `REGULAR` XPS 区块。一个文件始终是一个实验，并保留稳定的测量位置与
谱区。Plotium 保存原生能量、Counts、CPS、光子能量、dwell time、sweep、位置、
采集条件和自由元数据。只有区块边界可信时才会警告并跳过非 XPS 区块；XPS 数据
结构异常时拒绝整个文件。ordinate 极值属于元数据而不是数据点；ordinate 标签和
信号模式共同决定脉冲计数是否换算为 CPS。

CasaXPS `.txt` 按八行结构头识别，而不是仅按扩展名识别。其源数组与拟合参数
作为导入结果保存；其他 `.txt` 继续走表格导入。能量换算、处理与拟合见
[XPS](/zh-cn/guides/xps/)。

## Bruker NanoScope AFM

Plotium 可导入 NanoScope `.spm` 图像、力曲线与 Force Volume 网格，以及
PeakForce Capture `.pfc` 数据立方体。图像通道按文件记录的扫描尺寸和物理单位
绘制成地图，并锁定纵横比。力曲线按 approach 与 retract 两段分别绘制；文件中
记录了偏转灵敏度时，纵轴为以纳米计的偏转量，否则曲线保持文件中存储的单位。
Plotium 按采集原样显示数据——不会推断接触点、压痕或模量，也不会拟合接触力学
模型。

PeakForce Capture 文件旁通常保存着一个 AllImages `.spm` 导出文件。Plotium 会
找到这个配套文件，核对其图像网格与力网格一致后，把两者作为一个数据集导入；
打开文件夹时这样的文件对也只导入一次，不会成为两个数据集。默认画布会把通道
地图和网格中心像素的力曲线并排放置。找不到配套文件、或其网格不一致时，
`.pfc` 文件仍会导入，只是仅含力曲线。

PeakForce Capture 曲线是逐像素采集的原始信号。模量等派生 QNM 地图作为独立的
图像通道导入；Plotium 不会从曲线重新计算这些地图。

## 电生理（ABF2）

Plotium 读取 int16 或 float32 采样的 ABF 2.x 文件，支持多通道、多 sweep 以及
文件内的 DAC/epoch 刺激。不支持 ABF1 和压缩的 ABF2 数据；请在采集软件中重新
导出为未压缩的 ABF2。文件选择器可以一次选择多个 ABF，每个文件的直接父目录名
会成为可编辑的初始 cell ID。sweep、滤波、时间窗统计、刺激与 IV 分析见
[电生理](/zh-cn/guides/electrophysiology/)。

## 表格

CSV、TSV、带分隔符的 `.txt`、XLSX 以及从剪贴板粘贴的表格，都会经过表格导入
预览。列类型推断、Origin CSV 表头和 XLSX 工作表的处理见
[导入表格](/zh-cn/guides/importing-data/#导入表格)。

## Origin 项目（实验性）

Origin 的 `.opj` 与 `.opju` 文件可以从 **Open File…** 和 **Import Table…**
两个入口选择；两者都根据文件内容识别格式，而不是只看扩展名。受支持的 `.opj`
中的工作表会进入[表格导入预览](/zh-cn/guides/importing-data/#导入表格)。导入过程中
无需安装、启动或调用 Origin。

成功导入仅限两种通过文件内容精确识别的 OPJ 生成配置：

- Origin 7.0552（`CPYA 4.2673 build 552`）可导入经过验证的 `f64`、`f32`、
  有符号 `i32`、有符号 `i16`、定宽 ASCII 文本、数值与文本混合的单元格、
  空值和非零行偏移。项目参数与备注会作为来源元数据保留。
- Origin 9.51 build 195 W64（`CPYA 4.3268 build 195 W64`）可导入工作表名、
  列名、`f64` 数值、空值和经过验证的空列。该配置目前不导入文本单元格、
  项目参数或备注。

兼容性仅限上述经过测试的配置。不能因为扩展名相同或 Origin 主版本相同，就推定
其他文件也受支持。

Plotium 会保留经过验证的 Origin 窗口或分组名与列名。每个受支持的窗口以单个
数据表表示，并使用合成的工作表名 `Sheet1`；不解析原始工作表标签。Origin 7 的
混合列会保留为文本，长度不等的列会用空值补齐。长名称、单位、注释、列标识、
日期、分类值和未经验证的代码页不保证能够导入。

Plotium 会根据内容签名识别 `.opju` 文件，但本版本不能导入 `.opju`，也不会产生
任何部分结果。

不支持的内容包括图形、公式、脚本、分析重新计算、把已保存的分析结果恢复为
可执行分析、矩阵、嵌入对象、现代 OPJ 文本单元格、没有已验证代码页的非 ASCII
文本、加密或受保护项目、未经验证的 OPJ 版本或配置，以及未经验证的 OPJU
容器。对于受支持的 Origin 9.51 配置，Plotium 会在经过验证的窗口列表处停止
解析，并警告剩余项目对象没有被导入。

Plotium 绝不猜测导入内容。损坏或截断的文件、超过当前 128 MiB 输入上限的文件、
扩展名与签名不匹配的文件，以及结构异常或其他不受支持的文件，都会在提交任何
数据表之前给出明确错误。在其他方面受支持的 OPJ 中，只有在相应内容具有独立框架
且其外层边界可信时，才可以省略不受支持的工作表列，或跳过不受支持的非表格对象。
Plotium 会为每次此类省略显示警告，因此导入的工作表可能只含受支持的列。若框架
含糊，Plotium 会拒绝文件，而不会造成数据错位。

## Zip 压缩包

`.zip` 文件作为打包的数据文件夹打开。
