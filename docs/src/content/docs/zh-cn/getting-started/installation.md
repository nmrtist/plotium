---
title: 安装
description: 在 Windows、macOS 和 Linux 上安装并首次启动 Plotium。
---

Plotium 是原生桌面应用，支持 Windows、macOS 和 Linux。

## 下载与安装

预编译安装包发布在
[GitHub releases 页面](https://github.com/nmrtist/plotium/releases)。
macOS 用户下载对应处理器架构的 DMG，打开后将左侧的 **Plotium.app**
大图标拖到右侧的 **Applications**。推出磁盘映像，再从 Finder 的 Applications 中打开 Plotium。
安装过程不需要运行终端命令。

Windows 和 Linux 用户解压 ZIP 到任意位置，运行其中的 `plotium` 即可。
这些便携包无需安装程序。

| 平台 | 下载包后缀 | 要求 |
| --- | --- | --- |
| Windows x86-64 | `windows-x86_64.zip` | 64 位 Windows |
| Linux x86-64 | `linux-x86_64.zip` | glibc 2.35+（例如 Ubuntu 22.04+），图形桌面 |
| macOS Intel | `macos-x86_64.dmg` | macOS 13+ |
| macOS Apple Silicon | `macos-aarch64.dmg` | macOS 13+ |

发行版的 `SHA256SUMS` 文件覆盖所有 ZIP 和 DMG。macOS 便携 ZIP 保留原有
名称和内容，包含裸可执行文件及 MCP 配套文件，不包含应用包。

当前 DMG 内的应用采用 **ad-hoc 签名，未公证**。这能验证本地应用包的完整性，
不代表经过 Apple 认证的开发者身份或互联网分发许可。Gatekeeper 可能阻止
下载的副本；参见下方“首次启动”。

### 可选：连接 AI 客户端

macOS 应用包已包含 MCP 连接器和 Claude Desktop 扩展，请保持应用包完整，
不要移出 Contents 内的文件。使用便携 ZIP 时，请将 `plotium-mcp`
（Windows 下为 `plotium-mcp.exe`）保留在主程序旁。macOS 和 Windows ZIP
也包含 Claude Desktop 扩展 `plotium.mcpb`。

准备好后，按[连接 AI 客户端](/zh-cn/guides/automation/#连接-ai-客户端)中的步骤操作。
每个客户端只需添加一次。

## 首次启动

由于安装包从网络下载，操作系统可能在首次运行前给出警告：

- **Windows**——若 SmartScreen 提示"无法识别的应用"，选择
  **更多信息 → 仍要运行**。
- **macOS**——以 Gatekeeper 的实际提示为准。当前 DMG 未采用 Developer ID
  签名，也未公证，本地自建包能启动不能证明下载的副本会被接受。决定是否
  允许应用前，请阅读 [Apple 安全说明](https://support.apple.com/zh-cn/102445)。
  不要忽视恶意软件或应用损坏警告。

首次启动后 Plotium 显示欢迎页。把数据文件拖进窗口，或使用
**File → Open File…** 开始——[界面速览](/zh-cn/getting-started/quick-tour/)
会带你认识界面。

在 Finder 中双击 `.plotium` 工程可用 Plotium 打开。若安装了多个 Plotium
副本，使用**打开方式**选择目标副本。`.plotiumproc` 处理配方需要在 Plotium
内应用到数据集，不注册为可独立打开的 Finder 文档。

## Plotium 的设置存放在哪

偏好设置、已保存的处理模板和自定义拟合模型存放在一个较小的按用户配置
文件夹中：

| 平台 | 位置 |
| --- | --- |
| Windows | `%APPDATA%\plotium\config` |
| macOS | `~/Library/Application Support/plotium` |
| Linux | `~/.config/plotium` |

你的数据和项目从不存放在这里——它们保存在你选择的位置。删除该文件夹会
把 Plotium 恢复为默认设置。

## 更新与卸载

macOS `.app` 用户先用 **Command-Q** 退出 Plotium，再下载新 DMG，替换
Applications 中的 Plotium.app。工程和偏好设置会保留。应用包不使用便携包的
二进制更新器；便携 ZIP 仍保留后台更新，详见[更新](/zh-cn/reference/updates/)。
卸载时删除 Plotium.app 或便携应用文件夹即可；若想彻底清理，可一并删除
上述配置文件夹。

## 从源码构建

开发者可从代码仓库构建 Plotium，步骤见
[仓库 README](https://github.com/nmrtist/plotium#build-from-source)。使用
发行版安装包的用户无需这样做。
