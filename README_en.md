# GMod_Tools

> One-stop GUI toolkit for Garry's Mod addon developers

[中文 / Chinese ](README.md)

[![GitHub Stars](https://img.shields.io/github/stars/hlshell/GMod_Tools?style=flat-square)](https://github.com/hlshell/GMod_Tools/stargazers)
[![GitHub Forks](https://img.shields.io/github/forks/hlshell/GMod_Tools?style=flat-square)](https://github.com/hlshell/GMod_Tools/forks)
[![GitHub Watchers](https://img.shields.io/github/watchers/hlshell/GMod_Tools?style=flat-square)](https://github.com/hlshell/GMod_Tools/watchers)
[![GitHub License](https://img.shields.io/github/license/hlshell/GMod_Tools?style=flat-square)](LICENSE)
[![Release](https://img.shields.io/github/v/release/hlshell/GMod_Tools?style=flat-square)](https://github.com/hlshell/GMod_Tools/releases)
[![Platform](https://img.shields.io/badge/Platform-Windows-blue?style=flat-square)]()
[![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square)]()

---

## 📖 Introduction

**GMod_Tools** is a desktop GUI toolkit built for Garry's Mod addon developers, integrating common asset-processing workflows to make packing, validation and resource management easier.

- ✅ **Open-source** project, source code hosted in this repository
- ✅ **Windows only**, no Linux build yet
- ✅ Pre-built ZIP packages available in Releases

---

## ✨ Features

- 📦 GMA pack / unpack
- 🖼️ VTF / VMT preview, export & quick edit
- 🔍 Asset directory validation (models, textures, sounds)
- 💻 Lua snippet runner, linter & formatter
- 🗂️ One-click addon folder structure standardization
- 🧩 VPK pack / unpack / listing
- 🦴 MDL unpack (.vvd / .vtx → .smd), skeleton & mesh viewer
- 🌓 Light / Dark theme switch
- 💾 Persistent user config
- 📖 Built-in document viewer

---

## 📥 Getting Started

### Option A: Download Release (Recommended)

1. Download the latest `GMod_Tools-win64.zip` from [Releases](https://github.com/hlshell/GMod_Tools/releases)
2. Extract to a path **without non-ASCII or special characters**
3. Run the executable directly, no build required

### Option B: Build from Source

> Dependency: Rust toolchain

```bash
git clone https://github.com/hlshell/GMod_Tools.git
cd GMod_Tools
cargo build --release
```

---

## ⚠️ Notes

- **Windows only**, no Linux support yet
- Asset processing (GMA / VPK / VTF / MDL) is fully built-in, **no third-party tools needed**
- Workshop upload uses GMod official `gmpublish.exe`; MDL compile uses official `studiomdl.exe` (auto-detect or manual config)
- For Garry's Mod addon development & learning purposes
- Licensed under the `LICENSE` file in the repository root

---

## 🤝 Contribute

Issues are welcome for bug reports and feature requests. Please open an issue before submitting a pull request.

---

## ❤️ Support

- ⭐ **Star** this repo to follow updates
- 🍴 **Fork** for modification (follow license)
- 📢 **Share** with other Garry's Mod developers

---

## 📮 Contact

- GitHub Issues: https://github.com/hlshell/GMod_Tools/issues

---

## 📜 License

This project is licensed under the GNU General Public License v3.0 (see the `LICENSE` file in the repository root).
