---
title: 连接 AI 客户端
description: 把 Codex、Claude Code 或 Claude Desktop 连接到 Plotium，让它处理你打开的项目。
---

连接到 Plotium 的 AI 客户端可以查看你打开的项目、制作和调整图形、运行支持的
分析，同时你仍可在桌面端继续操作。你在客户端（Codex、Claude Code 或 Claude
Desktop）里与它对话；Plotium 没有内置聊天窗口。连接只在你自己的电脑上运行，
使用 Model Context Protocol（MCP）。

## 开始之前

- 安装你要使用的 AI 客户端。
- 保持 Plotium 安装完整。macOS 应用已包含连接器；ZIP 安装请把 `plotium-mcp`
  （Windows 下为 `plotium-mcp.exe`）保留在主程序旁。

## 连接

1. 打开 Plotium。
2. 打开 **Preferences → AI connections (MCP)**。也可以点击 Ribbon 中操作历史旁的
   机器人图标，或使用菜单 **File → AI connections (MCP)…**。
3. 开启 **Enable MCP**。
4. 在 **Access** 中，选择 **Read only**（只读）让 AI 只查看不修改，或选择
   **Read and modify**（读取和修改）允许支持的编辑。默认是 **Read and modify**；
   两者的区别见[权限、审阅与撤销](/zh-cn/ai/control-and-review/)。
5. 按下表添加客户端：

| 客户端 | 在 Plotium 中 | 在客户端中 |
| --- | --- | --- |
| Codex | 在 Codex 卡片点击 **Add** | 如有需要，重载 MCP 配置或重启客户端 |
| Claude Code | 在 Claude Code 卡片点击 **Add** | 如有需要，重载 MCP 配置或重启客户端 |
| Claude Desktop（macOS 和 Windows） | 点击 **Add to Claude Desktop…** | 确认安装扩展；如有需要，重载或重启 |

每个客户端只需添加一次。Plotium 会在重启后记住 **Enable MCP** 和权限设置。

## 检查连接

请自行启动 AI 客户端，Plotium 不会代为启动。卡片上的 **Configured** 只表示已添加
连接配置。要确认客户端确实能访问 Plotium，请发送：

> 请使用 Plotium 列出当前项目中的数据集和图形，不要修改任何内容。

客户端应能描述你的项目；客户端活跃时，Plotium 中的机器人图标会变为绿色。仅打开
Claude Desktop 扩展安装包，并不代表连接已经成功。没有反应时，见
[连接不成功时](/zh-cn/ai/control-and-review/#连接不成功时)。

接下来可以看[与 AI 协作](/zh-cn/ai/working-with-ai/)，了解可以提什么要求。

## 其他 MCP 客户端

使用同一台电脑上的其他 MCP 客户端时，展开 **Manual configuration and
diagnostics**，点击 **Copy stable stdio configuration**，把复制的配置粘贴到客户端中。
此连接只适用于同一台电脑、同一用户账户下的客户端，不支持远程连接或 Windows 与 WSL
之间的访问。手动注册命令和技术参考见 [MCP 配置与权限](/zh-cn/reference/mcp/)。

## 更新 Plotium 之后

在 AI 客户端中重启 Plotium 连接（或重启客户端），以加载更新后的工具。不需要重新
添加客户端。
