---
title: MCP 保存与导出
description: 经桌面端确认后保存项目、导出图形。
---

## 保存当前桌面项目

保存复用 **Save project** 的原生快照、后台 worker、备份和完成回调，保存当前
桌面实际状态，不另开 CLI 项目。先读 `plotium_get_state`；`save_state` 给出当前路径、
已存档案 revision、dirty、edit_generation 和最多 16 个保留的 `receipt_ids`。
逐个用 `plotium_save_status` 查询完整回执和恢复动作；仅有 ID 不代表提交成功或已经完成。

首次保存或另存时，不带 `ids`／`targets`：

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 46,
  "tool_id": "project.save",
  "parameters": {"path": "/absolute/path/figure.plotium"}
}
```

省略 `path` 保存当前文件；未命名项目必须给出路径。父目录须已存在，目标为普通、
非符号链接的 `.plotium` 文件或尚不存在的路径。预检解析父目录并冻结已观察的目标。
可选 `include_view_snapshots` 覆盖当前项目的原生保存配置。计划返回 `save_id`、
实际路径及 `overwrite_required`，不授予磁盘写权限。

用 `plotium_execute {"plan_id":"PLAN_ID"}` 提交一次性计划，结果为
`save.status: "awaiting_authorization"`。在 Plotium 核对准确路径，点击
**Allow save**；已有文件还必须勾选 **Replace the existing file at this exact path**。
**Deny** 不改目标。这与 **Read and modify** 独立，只授权本次保存，不能通过工具
参数代为批准。Read only 模式也可发起此授权请求。目标变化须重新规划并确认；
同时最多一个外部保存请求。

序列化使用独占创建的私有唯一同目录临时文件，不写固定 `<target>.tmp`。
已有目标在备份准备完成后、实际替换前再次复验预检冻结的文件元数据；发现变化即拒绝
安装，保留后来版本。一次性权限准入与文件复验是两个边界；复验加替换并非跨进程
compare-and-swap。非协作写入若发生在最后复验之后，或保留了所检查的元数据，
仍可能无法察觉；应避免并发保存到同一路径。

用 `plotium_save_status {"save_id":"SAVE_ID"}` 查询，不带项目 revision/context，
保存中也可读取且不使项目 dirty。`awaiting_authorization`、`queued`、`saving`
均未完成，`phase` 为 null；只有 `completed` 带最终阶段：

| phase | 文件状态与下一步 |
| --- | --- |
| `not_installed` | 本次未把快照安装到目标。处理拒绝或错误、检查实际状态；仍需保存时重新取得授权。 |
| `installed_unconfirmed` | 快照已替换／创建目标，但备份收尾或持久同步失败。保持项目打开，检查目标及存储后再明确授权重试；不能假定旧文件还在。 |
| `committed` | 捕获的快照完成原生持久提交；后来的编辑仍可能未保存。 |

Windows 替换部分失败时，即使新快照为 `not_installed`，旧目标仍可能已被移走或删除。
Plotium 保留已序列化的候选文件，并报告准确恢复路径。只有不覆盖现有目标时才尝试
从已知备份恢复原文件；无法完成时，保留并报告该备份及更早的暂存备份。如果 Windows
移走原文件却未返回其位置，错误会明确说明。保持项目打开，检查所列文件后再决定是否
重试；此类失败不会清除 dirty 状态。

回执包含原始 `project_context`、`snapshot.document_revision`、
`snapshot.edit_generation`、`persisted_revision`、准确路径、`state_applied`、
`dirty_after_completion`、错误及恢复说明。查询结果另给当前 context/path/revision/dirty
和 `matches_current_project`。快照在桌面批准后、后台工作前捕获；之后的新编辑
保持 dirty，完成不等于当前所有编辑已落盘。
已安装但未确认同步时仍保持 dirty，并记录已安装基线供原生恢复使用。
保存规划／提交和状态查询仅用临时桌面回执，不加审计记录；普通项目读取同样不追加
项目审计记录，也不改变 dirty。

切项目或停止连接撤销待写权限。worker 只持有原项目快照和路径，并在安装边界检查
授权；即便 document revision 相同，旧完成也不能装入另一项目或清它的 dirty。
worker 原子确认授权并进入安装后，停止不保证取消这次原文件写入，须查回执；
在此之前撤销则阻止安装。保存不加入 Undo，Undo 不删除或还原磁盘文件。

桌面进程保留最近 16 条回执，跨连接恢复和项目切换有效，退出应用后不保留。响应丢失后
先查询原 `save_id`，不要重放已提交调用。应用重启或 `unknown_save` 时，检查目标和
新打开的项目，不自行假定成功或失败。稳定 stdio 配置继续可用；重开／重启后仍须
重新读取状态、发现目标并规划。连接恢复不是保存成功的证据。

## 将捕获画布导出为 SVG、PDF 和 PNG

用 `figure.export` 指定一张准确的完整画布 TargetRef，不带顶层 `ids`／`targets`。
复用原生导出器，保留背景、完整页面、曲线、绝对 Y 误差幅度、原生文字和嵌入图片；
不裁切、不套主题、不改项目。成品之外仍须保留可编辑 `.plotium` 项目。

```json
{
  "expected_project_context": "PROJECT_CONTEXT",
  "expected_revision": 48,
  "tool_id": "figure.export",
  "parameters": {
    "canvas": {"resource":{"id":"CANVAS_ID","kind":"plotium.canvas"}},
    "size_mm": [180,90],
    "dpi": 300,
    "outputs": [
      {"format":"svg","path":"/absolute/path/figure.svg"},
      {"format":"pdf","path":"/absolute/path/figure.pdf"},
      {"format":"png","path":"/absolute/path/figure.png"}
    ]
  }
}
```

`size_mm` 默认原生页面尺寸；显式尺寸等比缩放整图，须保留宽高比（高度误差容许
0.001 mm），不改变可编辑页面。`dpi` 默认 300，范围 1–2400，仅控制 PNG。
每轴按原生物理 pt / 72 × DPI × 等比缩放系数四舍五入，最小 1 像素；
180 × 90 mm、300 DPI 得到 **2126 × 1063 像素**。SVG/PDF 保留物理页尺寸。
文字使用原生通用 sans-serif 字族；无可用系统字体时报错。SVG 在另一台机器的
字形取决于当地可用 sans-serif 字体。

每批 1–8 个输出，绝对路径须带匹配的小写扩展名，父目录已存在；目标必须是普通
非符号链接文件或尚不存在。重复的解析后输出路径拒绝。预检返回 `export_id`、
物理／像素尺寸，以及逐文件准确目标或错误；坏目录不会掩盖其他文件的结果。
`overwrite` 默认 false，已有目标报告 `overwrite_not_requested` 并保留原字节。
确需替换时显式设 `overwrite:true`，这仍不授予写入权限。

用 `plotium_execute` 提交一次性计划。有效文件等待 Plotium 的
**Allow AI to export this figure?** 提示，其中列出画布、revision、尺寸及路径。
核对后点击 **Allow export**；已有文件还必须勾选默认未勾选的
**Replace the listed existing output files**。**Deny** 不安装任何输出。
**Read and modify** 不等于导出授权；只读可请求新的独立导出授权。
全无效批次直接逐文件报错，不弹授权窗。

用 `plotium_export_status {"export_id":"EXPORT_ID"}` 查询，不带 context/revision，
渲染中也可查询且不生成审计。返回 `export`、当前 context/revision 和
`matches_current_project`。批次状态为 `awaiting_authorization`、`rendering`、
`completed`；`files[]` 每项包含格式、准确路径、覆盖要求、`status`、`error_code`、
`message` 和已安装字节数。文件状态为 `pending`、`succeeded`、`failed` 或
`unknown`（worker 失败，先检查该路径再决定下一步）。批次完成不等于全部成功。
缺失／损坏图片、字体、目标变化、权限拒绝和渲染／预算错误会准确报告且不替换
该输出；同批此前成功文件保留。

渲染在后台使用桌面批准后捕获的画布／资产，绑定原 `project_context`、`canvas`
和 `render_revision`，后续编辑不改变快照。切项目、停止连接或修改访问权限会撤销
待安装权限。逐文件原子权限准入先于实际安装：准入前撤销阻止安装；已准入文件
仍可能完成，须查回执。旧完成不改变新项目的文档、保存路径、dirty 或状态。
独占唯一同目录暂存及原子不覆盖创建保护未授权文件；已有目标在实际替换前复验
元数据，但并非跨进程 compare-and-swap，应避免并发写相同路径。
导出成功表示输出已安装，不等于原生项目保存的持久提交阶段；Undo 不删除或还原磁盘文件。

预算：一个运行批次，共享待执行计划最多 32 个；路径最多 4096 字节；PNG 单边
16,384、总计 16,777,216 像素；引用的嵌入资产 16 MiB、解码后 RGBA 64 MiB、
SVG 8 MiB、逐文件编码结果 64 MiB。不查找外部资产、不替换成占位图。
完整 schema 和限制可从 `limits` 与 `operation_constraints` 能力分区发现。

桌面进程保留最近 16 批导出回执，跨重连及项目切换有效，退出应用即丢失。
响应丢失后先查原 `export_id`，不重放导出。重启或 `unknown_export` 时先检查
准确输出文件。重开可编辑项目后重新读取状态、发现目标；仅为新意图重新规划。
