---
title: 安装
description: 在 Windows、macOS 和 Linux 上安装并首次启动 Plotium。
---

Plotium 是原生桌面应用，支持 Windows、macOS 和 Linux。

## 下载与安装

预编译安装包发布在
[GitHub releases 页面](https://github.com/nmrtist/plotium/releases)。
下载对应平台的压缩包，解压到任意位置，运行其中的 `plotium` 可执行文件即可。
无需安装程序，也不需要管理员权限。

| 平台 | 压缩包后缀 | 要求 |
| --- | --- | --- |
| Windows x86-64 | `windows-x86_64.zip` | 64 位 Windows |
| Linux x86-64 | `linux-x86_64.zip` | glibc 2.35+（例如 Ubuntu 22.04+），图形桌面 |
| macOS Intel | `macos-x86_64.zip` | macOS 13+ |
| macOS Apple Silicon | `macos-aarch64.zip` | macOS 13+ |

发行版的 `SHA256SUMS` 文件列出了所有压缩包的校验和。
这些包是未签名的便携可执行文件，macOS 包不包含 `.app` 应用包。

### 可选：连接 AI 客户端

如需在 Plotium 中使用 Codex、Claude Code 或 Claude Desktop，请将 `plotium-mcp`
（Windows 下为 `plotium-mcp.exe`）保留在主程序旁。macOS 和 Windows 包还包含
Claude Desktop 扩展 `plotium.mcpb`。

准备好后，按[连接 AI 客户端](/zh-cn/guides/automation/#连接-ai-客户端)中的步骤操作。
每个客户端只需添加一次。

## 首次启动

由于安装包从网络下载，操作系统可能在首次运行前给出警告：

- **Windows**——若 SmartScreen 提示"无法识别的应用"，选择
  **更多信息 → 仍要运行**。
- **macOS**——若 Gatekeeper 阻止运行，右键点击应用选择**打开**并确认；
  macOS 会记住这一选择。

首次启动后 Plotium 显示欢迎页。把数据文件拖进窗口，或使用
**File → Open File…** 开始——[界面速览](/zh-cn/getting-started/quick-tour/)
会带你认识界面。

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

Plotium 会在后台检查并自动更新——见[更新](/zh-cn/reference/updates/)。
卸载时删除应用文件夹即可；若想彻底清理，可一并删除上述配置文件夹。

## 从源码构建

开发者可从代码仓库构建 Plotium，步骤见
[仓库 README](https://github.com/nmrtist/plotium#build-from-source)。使用
发行版安装包的用户无需这样做。
