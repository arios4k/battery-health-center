<div align="center">

[English](README.md) · [Español](README.es.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · **简体中文**

<img src="docs/icon.svg" width="128" alt="Battery Health Center 图标：一块蓝色电池，带有青绿色电芯和白色健康脉冲">

# Battery Health Center

**笔记本电脑电池的一切，尽在一处。**
电池健康、循环次数、充电上限与计划，以及哪些应用在耗电，专为 GNOME 打造。

[![最新版本](https://img.shields.io/github/v/release/arios4k/battery-health-center?label=%E7%89%88%E6%9C%AC&color=26a269)](https://github.com/arios4k/battery-health-center/releases/latest)
[![CI](https://github.com/arios4k/battery-health-center/actions/workflows/ci.yml/badge.svg)](https://github.com/arios4k/battery-health-center/actions/workflows/ci.yml)
[![手册](https://img.shields.io/badge/%E6%89%8B%E5%86%8C-%E9%98%85%E8%AF%BB-2ea44f)](https://arios4k.github.io/battery-health-center/manual/)
[![下载量](https://img.shields.io/github/downloads/arios4k/battery-health-center/total?color=26a269)](https://github.com/arios4k/battery-health-center/releases)
[![使用 apt 安装](https://img.shields.io/badge/install-apt-orange)](#在-ubuntu-和-debian-上安装)
[![在 Fedora 上安装](https://img.shields.io/badge/install-Fedora%20COPR-51a2da)](#在-fedora-上安装)
[![COPR 构建](https://copr.fedorainfracloud.org/coprs/arios/battery-health-center/package/battery-health-center/status_image/last_build.png)](https://copr.fedorainfracloud.org/coprs/arios/battery-health-center/builds/)
[![许可证：GPL-3.0-or-later](https://img.shields.io/badge/%E8%AE%B8%E5%8F%AF%E8%AF%81-GPL--3.0--or--later-blue)](LICENSE)
[![语言](https://img.shields.io/badge/%E8%AF%AD%E8%A8%80-EN%20%C2%B7%20ES%20%C2%B7%20DE%20%C2%B7%20FR%20%C2%B7%20ZH-lightgrey)](#翻译)
[![使用 Rust 和 GTK4 开发](https://img.shields.io/badge/made%20with-Rust%20%2B%20GTK4-informational)](#从源码构建)
[![通过 PayPal 支持](https://img.shields.io/badge/PayPal-Support-00457C?logo=paypal&logoColor=white)](https://paypal.me/AlexisRios)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/v0.5.0/overview-dark.png">
  <img src="docs/screenshots/v0.5.0/overview-light.png" alt="Battery Health Center 0.5.0：侧边栏导航，在自适应仪表盘中显示电池状态与健康">
</picture>

</div>

## 一个应用，取代一堆脚本

Linux 能提供电池数据，但它们分散各处：满屏 `upower` 和 `/sys` 文件的终端、用来设置充电上限的 Shell 扩展、
靠 `powertop` 猜测哪些应用耗电。Battery Health Center 把它们汇集到一个原生 GNOME 应用中，有点像 macOS
的电池设置：

| 原本需要… | Battery Health Center 提供 |
|---|---|
| 手动读取 `/sys/class/power_supply` 和 `upower -i` | **健康**：最大容量与设计容量对比、循环次数、状态，以及预计降至 80% 的日期趋势 |
| 用脚本或扩展设置充电上限 | **充电**：一键切换配置、自定义上限、充电计划和优化充电 |
| 猜测哪个应用在耗电 | **使用情况**：按能耗排序的应用，附估算功率和逐小时视图 |
| 在“设置”、“优化”和各种扩展中寻找电源选项 | **电源**：电源模式、自动电源模式和 GNOME 的节能设置集中在一个页面 |
| 一张记录读数的电子表格 | **报告**：HTML 电池报告和历史数据的 CSV 导出 |

## 手册

**要了解如何使用 Battery Health Center，请阅读 [Battery Health Center 手册](https://arios4k.github.io/battery-health-center/manual/)**（英文）。它逐步讲解每个页面：电池健康与校准、充电上限与充电计划、应用使用情况、电源模式、通知、报告与您的数据，还包括故障排除和每个版本的更新内容。也可以在应用中按 **F1** 或点击帮助按钮打开。

## 为什么选择 Battery Health Center

- **原生且轻量。** 使用 Rust、GTK 4 和 libadwaita：程序仅 1.2 MB（软件包 0.5 MB），支持浅色和深色模式，并跟随
  您的 GNOME 主题。
- **默认注重隐私。** 一切都留在您的电脑上：应用从不联网，也没有遥测。历史记录只是您主目录中的几个 CSV 文件。
- **谨慎使用 root。** 只有一个小型辅助程序以 root 身份运行。它只接受经过验证的值，并且只写入电池的充电上限
  文件和它自己的配置。
- **说您的语言。** 英语、西班牙语、德语、法语和简体中文，跟随您的系统语言。
- **随系统更新。** 从已签名的 apt 仓库（或 Fedora 的 COPR）安装，日常系统更新就会让它保持最新。

更多问题的答案（例如充电上限能否在您的笔记本电脑上使用）请见手册中的[故障排除与常见问题](https://arios4k.github.io/battery-health-center/manual/troubleshooting.html#faq)。

<sub>截图在 Zorin OS 18 上使用其 ZorinBlue 主题拍摄。可用 `docs/screenshots/capture.sh` 重新生成。</sub>

## 在 Ubuntu 和 Debian 上安装

适用于 Ubuntu 24.04+、Debian 13+、Linux Mint 22、Zorin OS 18 及其他衍生版，仅限 64 位 Intel/AMD：

```bash
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://arios4k.github.io/apt/key.asc | sudo tee /etc/apt/keyrings/battery-health-center.asc > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/battery-health-center.asc] https://arios4k.github.io/apt stable main" | sudo tee /etc/apt/sources.list.d/battery-health-center.list
sudo apt update && sudo apt install battery-health-center
```

然后在应用程序列表中打开 **Battery Health Center**。更新通过 `sudo apt upgrade` 获取。签名密钥的指纹是
`F090 D01D 74F7 14C8 5A95 23CA 4773 367E BD97 6CF5`，仓库（含故障排除提示）位于
[arios4k/apt](https://github.com/arios4k/apt)。

## 在 Fedora 上安装

适用于 Fedora 43 及更新版本，仅限 64 位 Intel/AMD，来自 [COPR 仓库](https://copr.fedorainfracloud.org/coprs/arios/battery-health-center/)：

```bash
sudo dnf copr enable arios/battery-health-center
sudo dnf install battery-health-center
```

然后在应用程序列表中打开 **Battery Health Center**。更新通过 `sudo dnf upgrade` 获取。充电上限和计划服务会在
安装时自动启用，无需其他设置。

## 从 snap 迁移

如果您安装过旧版本的 snap，请先用 `sudo snap remove battery-health-center` 将其移除。这会删除其历史记录，所以
请在此之前把 `~/snap/battery-health-center/current/.local/share/battery-health-center/*.csv` 复制到
`~/.local/share/battery-health-center/`。

## 从源码构建

依赖：Rust、GTK ≥ 4.14、libadwaita ≥ 1.5，以及用于翻译的 `gettext`。

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev pkg-config gettext   # Debian/Ubuntu
cargo run --release
```

更改充电上限会使用 `pkexec` 从构建目录运行 `bhc-helper`，因此在应用安装之前，每次都会要求输入密码。如需系统级
安装，请运行 `./install.sh`（用 `./install.sh --uninstall` 卸载），或用 `scripts/build-locale.sh && cargo deb`
构建软件包。

开发时的实用命令：

```bash
# 试用某个翻译
scripts/build-locale.sh && LANGUAGE=zh_CN BHC_LOCALEDIR=$PWD/locale cargo run --release

# 试用充电计划界面而不影响您的真实设置：让应用指向一个包含模拟 battery-health-center.conf 的文件夹
# （例如一行 "BAT0 75 80"），还可以选择性地加入 battery-health-center.schedule 和 unplug-log
BHC_HELPER_FILES=/path/to/folder cargo run --release

# 为每个页面保存一张截图
BHC_SCREENSHOTS=/path/to/folder cargo run --release
```

## 翻译

应用提供英语、西班牙语（`es`）、德语（`de`）、法语（`fr`）和简体中文（`zh_CN`）版本。它会使用您的系统语言，
未翻译的内容则回退为英语。导出的 HTML 电池报告目前仍然只有英文。本 README 和[工作原理](docs/HOW-IT-WORKS.zh-CN.md)
也提供
[English](README.md)、[Español](README.es.md)、[Deutsch](README.de.md) 和 [Français](README.fr.md) 版本。

要添加一种语言，例如意大利语：

```bash
msginit --locale=it --input=po/battery-health-center.pot --output=po/it.po
```

翻译 `po/it.po` 中的字符串（Poedit 或 GNOME 的 Gtranslator 等 PO 编辑器会有帮助），然后把 `it` 添加到
`po/LINGUAS`，并在 `Cargo.toml` 的 `[package.metadata.deb]` 中仿照其他语言添加一行（`.mo` 文件），再用上面的
命令检查。非常欢迎提交新增语言的 Pull Request。`data/io.github.BatteryHealthCenter.desktop` 中的
`Comment[it]` 和 `Keywords[it]`，以及 metainfo 文件中的 `<summary xml:lang="it">`，用于翻译应用在软件列表中的
显示。

当应用中的文字发生变化时，请更新模板和翻译（尚未翻译的字符串会显示为空，并以英语显示）：

```bash
python3 scripts/update-pot.py
msgmerge --update --backup=none po/zh_CN.po po/battery-health-center.pot   # 以及其他语言
```

代码中的字符串通过 `tr("…")` 处理；带参数的文本用 `trf("… {name}", &[("name", value)])`；复数用
`trn`/`trnf`，详见 `src/i18n.rs`。

## 更多文档

- [用户手册](https://arios4k.github.io/battery-health-center/manual/)（英文）：详细介绍每项功能的指南，附截图。可在应用中通过帮助按钮或 `F1` 打开。
- [工作原理](docs/HOW-IT-WORKS.zh-CN.md)：数据从何而来，以及充电上限、充电计划和应用能耗估算是如何工作的。
- [维护与打包](docs/MAINTAINING.md)（英文）：如何发布新版本，以及给希望收录本应用的发行版的说明。欢迎收录，
  我将深感荣幸。
- [apt 仓库](https://github.com/arios4k/apt)：安装步骤、故障排除以及构建方式。

## 参与贡献

```bash
cargo test
cargo clippy
```

特别欢迎错误报告、翻译，以及在其他笔记本电脑和发行版上的测试：请[提交 issue](https://github.com/arios4k/battery-health-center/issues)，
并附上您的笔记本电脑型号和发行版。

## 作者

由 **Alexis Rios-Negron** 创建。

## 许可证

GPL-3.0-or-later © Alexis Rios-Negron。详见 [LICENSE](LICENSE)。
