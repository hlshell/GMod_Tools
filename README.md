# GMod_Tools

> 面向 Garry's Mod 模组开发者的一站式 GUI 工具箱

[English / 英文](README_en.md)

[![GitHub Stars](https://img.shields.io/github/stars/hlshell/GMod_Tools?style=flat-square)](https://github.com/hlshell/GMod_Tools/stargazers)
[![GitHub Forks](https://img.shields.io/github/forks/hlshell/GMod_Tools?style=flat-square)](https://github.com/hlshell/GMod_Tools/forks)
[![GitHub Watchers](https://img.shields.io/github/watchers/hlshell/GMod_Tools?style=flat-square)](https://github.com/hlshell/GMod_Tools/watchers)
[![GitHub License](https://img.shields.io/github/license/hlshell/GMod_Tools?style=flat-square)](LICENSE)
[![Release](https://img.shields.io/github/v/release/hlshell/GMod_Tools?style=flat-square)](https://github.com/hlshell/GMod_Tools/releases)
[![Platform](https://img.shields.io/badge/Platform-Windows-blue?style=flat-square)]()
[![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square)]()

---

## 📖 简介

**GMod_Tools** 是专为 Garry's Mod 模组创作者打造的桌面 GUI 工具箱，集成模组开发中常用的资源处理功能，让打包、校验、资源管理更加高效。

- ✅ 项目**开源**，源码托管于本仓库
- ✅ 仅支持 **Windows** 平台，暂不提供 Linux 版本
- ✅ Release 提供打包好的 ZIP 成品包，开箱即用

---

## ✨ 功能特性

- 📦 GMA 打包 / 解包
- 🖼️ VTF / VMT 预览、导出与快速编辑
- 🔍 模型、材质、音效资源目录合法性校验
- 💻 Lua 代码片段测试、语法检查与格式化
- 🗂️ 模组目录一键规范化整理
- 🧩 VPK 打包 / 解包 / 列表
- 🦴 MDL 解包（.vvd / .vtx → .smd）与骨架、网格查看
- 🌓 深色 / 浅色主题切换
- 💾 用户配置持久化保存
- 📖 内置文档查看面板

---

## 📥 快速开始

### 方式一：直接使用发行包（推荐）

1. 前往 [Releases](https://github.com/hlshell/GMod_Tools/releases) 下载最新 `GMod_Tools-win64.zip`
2. 解压至**不含中文、特殊字符**的路径
3. 运行主程序，无需编译部署

### 方式二：源码编译

> 环境依赖：Rust 工具链

```bash
git clone https://github.com/hlshell/GMod_Tools.git
cd GMod_Tools
cargo build --release
```

---

## ⚠️ 重要说明

- 仅支持 **Windows**，暂不提供 Linux 版本
- 资源处理（GMA / VPK / VTF / MDL）全部内置自研，**无需任何第三方工具**
- 创意工坊上传依赖 GMod 官方 `gmpublish.exe`，MDL 打包依赖官方 `studiomdl.exe`（均可自动探测或手动配置）
- 工具仅用于 Garry's Mod 模组开发与学习
- 本项目遵循仓库根目录 `LICENSE` 协议

---

## 🤝 贡献

欢迎提交 Issue 反馈 Bug、提出功能建议；提交 PR 前请先开 Issue 沟通方案。

---

## ❤️ 支持项目

- ⭐ **Star** 本仓库，关注项目更新
- 🍴 **Fork** 自由修改（遵守许可证）
- 📢 **分享** 给其他 GMod 开发者

---

## 📮 反馈渠道

- GitHub Issues：https://github.com/hlshell/GMod_Tools/issues

---

## 📜 许可证

本项目遵循仓库根目录 `LICENSE` 文件（GNU General Public License v3.0）。
