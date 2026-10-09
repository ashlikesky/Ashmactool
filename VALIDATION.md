# Ashmactool 0.2.0 验证记录

日期：2026-10-09。环境：Apple Silicon / arm64，macOS 27.0（26A428）；Rust 1.97.1。

## 已通过

- `cargo test --locked`：4 项测试通过，覆盖热点/非有限值拒绝、主题字段类型、帧数与图片数量限制，以及备份 plist 的像素和热点往返。
- `cargo clippy --all-targets --locked -- -D warnings`：通过。
- release 构建完成；最终 `.app` 的 ad hoc 签名严格验证、Info.plist 检查通过。
- 最终 `.app` 的 `--self-test`：Arrow / ArrowCtx / ArrowS 注册回读正确；重新加载磁盘备份后恢复，逻辑尺寸、热点、帧数和归一化 RGBA 像素与原始值一致，备份清理完成。
- 最终 `.app` 的 `--power-self-test`：`PreventUserIdleDisplaySleep` 请求类型和 255 启用级别回读正确；关闭后请求消失，重复关闭安全。
- 最终 `.app` 的 `--ui-smoke-test`：经 AppKit `sendAction` 分发实际原生菜单回调，样式切换、32 点更小尺寸、开启/关闭屏幕唤醒、恢复动作通过；32 点热点回读为 (6.5, 5)，测试后恢复配置。
- 图标：1024×1024 透明 PNG，打包为含 1x/2x 分辨率的 AppIcon.icns，Info.plist 正确引用。
- 应用实际启动后，另一进程回读 Arrow / ArrowCtx / ArrowS 为 32×32 点；屏幕唤醒默认关闭，未出现 Ashmactool 的电源请求。

## 包大小与校验

- release 可执行文件：420,368 字节，约 410.5 KiB。
- `.app` 磁盘占用：约 2.0 MiB，主要新增体积来自应用图标。
- arm64 ZIP：1,876,124 字节，约 1.79 MiB。
- 可执行文件 SHA-256：`2c064a656ae7d430f767669714b27c0b2513613a810e5ddd157e3a90d88c7233`。
- ZIP SHA-256：`eae7961fb08f6fa63b01944ac22a75f50fb548be64ee376a50491894eaa8e738`。

## 证据范围

菜单验证使用应用内部 AppKit 动作分发，没有宣称完成外部鼠标点击验收。电源功能验证了真实系统请求的创建和释放，没有等待整段闲置熄屏超时。登录启动、注销后重新登录、Intel、macOS 14–26 未完成实机验证。最低部署目标 14.0 不是对全部版本的兼容性保证。应用尚未 Developer ID 签名或公证。

本实现仍依赖私有指针接口，不保证所有软件和后续系统版本使用相同机制。保持屏幕唤醒使用公开 IOKit 接口，不阻止合盖、手动睡眠等系统行为。
