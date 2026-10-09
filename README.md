# Ashmactool

<img src="assets/AppIcon.png" width="160" alt="Ashmactool 图标">

Rust 编写的 macOS 菜单栏小工具箱。原生 AppKit 界面、单进程、没有 WebView，空闲时通过系统事件响应变化。

## 工具

| 工具 | 功能 |
| --- | --- |
| 指针样式 | 柔光 / 黑色 / 白色箭头，PNG 和 Mousecape `.cape` 导入，热点调整，恢复系统指针 |
| 保持屏幕唤醒 | 开关控制，防止闲置熄屏，关闭或退出时释放，重启应用默认关闭 |

指针尺寸有 **更小 / 小 / 中 / 大** 四档，画布分别为 32 / 40 / 52 / 64 点。“更小”比原来的“小”缩小 20%，实际箭头图形小于画布。自定义 `.cape` 保留主题原有尺寸。

屏幕唤醒使用 macOS 的 `PreventUserIdleDisplaySleep` 电源请求，不修改全局休眠配置，也不启动 `caffeinate` 子进程。它防止闲置熄屏；合盖、手动睡眠和锁屏仍由系统处理。

## 下载与使用

在 GitHub [Releases](https://github.com/ashlikesky/Ashmactool/releases) 下载 `Ashmactool-macOS-arm64.zip`，解压，将 `Ashmactool.app` 拖入 `/Applications` 后打开。点击菜单栏的工具图标操作。

当前发布包针对 Apple Silicon。最低构建目标是 macOS 14，实际运行验证环境见 [VALIDATION.md](VALIDATION.md)。本地包为 ad hoc 签名，尚未使用 Developer ID 签名和公证；若系统提示无法验证开发者，使用系统设置 → 隐私与安全性中的“仍要打开”。

退出应用会恢复本工具保存的原始指针并释放屏幕唤醒请求。登录启动需要将应用放入固定位置，再使用菜单中的“登录时启动”。

## 主题与恢复

- `.cape` 支持 1–24 帧纵向动画条带、最多 4 份分辨率表示、逻辑尺寸不超过 64×64 点。
- 导入前验证完整文件；无法安全备份的系统指针名称保留原状。
- 修改前将原始指针保存到磁盘；恢复只处理本工具保存的名称。
- 部分应用自行绘制指针，会覆盖系统主题。

设置、导入图片和恢复快照位于：

```text
~/Library/Application Support/Ashmactool/
```

应用异常退出后，下次启动会尝试恢复快照。也可以在关闭应用后执行：

```sh
/Applications/Ashmactool.app/Contents/MacOS/ashmactool --restore
```

## 构建与验证

需要 macOS、Xcode Command Line Tools 和 Rust 1.85 或更新版本。

```sh
cargo build --release --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo run --locked -- --doctor
cargo run --locked -- --power-self-test
cargo run --locked -- --self-test
cargo run --locked -- --ui-smoke-test
./scripts/package.sh
```

`--doctor`、`--inspect FILE.cape`、`--preview DIRECTORY` 不修改指针。`--self-test` 短暂注册样式，回读并立即恢复，检查原始像素。`--power-self-test` 创建电源请求，回读后立即释放。`--ui-smoke-test` 通过 AppKit 动作分发验证菜单，结束时恢复配置；不启用登录启动。指针测试需要先退出正在运行的本应用。

打包默认输出到项目上一级；可设置 `ASHMACTOOL_OUTPUT_DIR`。支持 `CARGO_TARGET_DIR`，无需将编译缓存放在源码目录。包内嵌入完整 `AppIcon.icns`；图标来源和提示词见 [assets/ICON.md](assets/ICON.md)。

## 代码结构

- `app.rs`：原生菜单、工具状态、导入和登录启动。
- `cursor.rs` / `graphics.rs`：指针注册、备份恢复和图像。
- `power.rs`：进程持有的 IOKit 电源请求，创建、回读和释放。
- `native.rs`：原生对象和 plist 文件操作。

## 参考与许可

参考 [Mousecape-swiftUI](https://github.com/sdmj76/Mousecape-swiftUI) 与原始 [Mousecape](https://github.com/alexzielenski/Mousecape) 的光标注册思路，Rust 实现独立编写，未打包上游 SwiftUI 代码、辅助程序或示例主题。参考版本和完整许可说明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

本项目采用 [个人非商业使用许可](LICENSE)。指针替换依赖 Apple 私有接口，系统升级后可能需要兼容性更新。屏幕唤醒使用 Apple 的公开 [IOPMAssertionCreateWithName](https://developer.apple.com/documentation/iokit/1557134-iopmassertioncreatewithname) 接口。
