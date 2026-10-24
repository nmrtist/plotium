---
title: 偏好设置
description: 偏好设置窗口中的每一项设置，按类别列出。
---

用 `Ctrl` + `,`（macOS 为 `Cmd` + `,`）或从菜单打开偏好设置。修改会自动
保存，通常立即生效；**Graphics processor** 的更改在重启后生效。
**Reset to Defaults** 恢复已保存偏好设置的默认值，但保留最近文件列表。
AI 连接启用状态和权限会持久保存，详见下文。

## General（通用）

- **Remind me about results while following is paused**——暂停跟随期间出现新的 AI 成果时，
  显示短暂提醒。可以恢复跟随、忽略本次提醒或关闭后续提醒；关闭后成果计数和错误提示仍然保留。
- **Object snapping**——让图形和形状吸附到页面与对象参考线，并让整个页面和
  数据表吸附到邻近框架的边缘及框架间的标准间距。也可在 **Arrange** 页签中切换。按住
  `Alt` 可仅为本次拖动绕过吸附。
- **Equal scale for homonuclear 2D imports**——导入两个坐标轴均为同一种
  频率单位且核种相同的 2D 谱时，若较宽轴不超过较窄轴的 2 倍，则初始使用
  F1/F2 等比例坐标。即使之后更改此偏好设置，已导入的图仍会保留原设置；
  可在对象检查器的 **Axes → Equal x/y scale (1:1)** 中单独修改。
- **Keep source canvas when tiling its last object**——拖放平铺把页面上最后
  一个对象移走后，保留这个空页面，而不随本次拖放一起删除。默认关闭；释放
  鼠标时按住 `Alt` 可仅为本次拖放反转此选择。
- **Project backup copies**——在每个项目旁以隐藏文件保留指定数量的完整
  历史保存。每份副本可能与项目一样大；选 Off 关闭。
- **Automatic updates** 与 **Update channel**——见
  [更新](/zh-cn/reference/updates/)。此区域还显示已安装版本、
  **Check now** 按钮，以及更新就绪后的 **Restart now**。macOS `.app` 安装
  改为显示 DMG 下载链接；自动更新和通道控件不可用，保存的偏好设置保持不变。

## Appearance（外观）

- **Chrome theme**——浅色、深色或跟随系统。它设置的是应用窗口的外观；
  图形本身的外观由各画布的画布主题决定。
- **Canvas accent**——选择框与控制柄、布局网格、页边距参考线和拖放平铺
  预览的颜色。可自选颜色，也可用 **Follow theme** 跟随应用主题。吸附
  参考线始终使用另一种对比色以便区分；图形内容和导出配色不受影响。
- **UI scale**——界面文字和控件的大小，按显示器分别设置。自动模式根据
  显示器报告的像素密度选择物理上可读的尺寸；手动选项和
  `Ctrl` + `+` / `Ctrl` + `-` 快捷键只覆盖当前显示器。
- **Graphics processor**——决定 Plotium 启动时优先省电还是图形性能。默认选择
  **Power saving**，适合大多数用户；如果出现显示问题，可以尝试
  **High performance**。更改会在重启后生效。

## Processing（处理）

- **Default ILT regularization (λ)**——为没有以往 ILT 结果可参照的数据集
  构建 ILT DOSY 图时所用的 λ。允许范围 `0.000001` 到 `1000`，默认 `0.01`。
  在 **DOSY** 卡片中输入的 λ 只作用于该数据集，不会改动这里的
  默认值。参见 [DOSY 扩散分析](/zh-cn/guides/dosy/)。

## Export（导出）

- **Embed view snapshots**——把每个图的屏幕视图保存进 `.plotium` 文件。
- **Raster resolution**——位图导出的默认像素密度（72–1200 dpi）。在某次导出
  对话框里填写的 DPI 只作用于那一次导出，不会改动这里的默认值；在
  [命令面板](/zh-cn/reference/command-palette/)中搜索 `export DPI` 即可打开本页。

## Recent（最近）

最近打开的文件、文件夹和项目——与 **File → Open Recent** 及欢迎页相同
的列表——并提供 **Clear recent files** 按钮。

## AI connections (MCP)

连接外部 AI 客户端，让它查看项目并完成支持的编辑。点击 Ribbon 中的机器人图标
可直接打开此页。

| 控件 | 作用 |
| --- | --- |
| **Enable MCP** | 允许本机 AI 客户端连接。默认关闭，重启后记住选择 |
| **Access → Read and modify** | 允许支持的编辑，可正常撤销和重做。这是默认权限 |
| **Access → Read only** | 允许查看和预览，阻止后续编辑，包括已准备的修改 |
| Codex / Claude Code → **Add** | 将 Plotium 添加到已安装客户端的 MCP 配置中 |
| **Add to Claude Desktop…** | 在 macOS 和 Windows 上打开扩展安装包，请在 Claude Desktop 中确认安装 |
| **Manual configuration and diagnostics** | 提供其他客户端所需的配置和排查连接问题的信息 |

权限对所有已连接客户端和项目统一生效，重启 Plotium 后仍保留。关闭设置或切换
项目不会停止访问。关闭 MCP 会立即停止访问并丢弃待执行修改，同时保留客户端配置；
退出 Plotium 也会停止访问。

**Configured** 表示客户端配置已完成；最近的客户端活动则表明客户端是否实际访问
过 Plotium。页面也会显示最近操作和错误。连接步骤、示例请求、机器人状态和排障
方法见 [AI 客户端连接](/zh-cn/guides/automation/#本机-agent-连接mcp)。
