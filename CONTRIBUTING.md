# 参与 Ashmactool

欢迎提交问题、功能建议和 Pull Request。

## 问题反馈

请提供 macOS 版本、芯片类型、Ashmactool 版本、复现步骤和实际表现。
指针问题请说明发生在哪个应用、哪种状态、所用主题及尺寸。截图或主题文件
请先检查是否包含个人信息；不要上传整个 Application Support 目录。

## 开发与验证

项目需要 macOS、Xcode Command Line Tools 和 Rust 1.85 或更新版本。
克隆与构建命令见 [README.md](README.md)。一般代码修改运行：

```sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
```

指针、菜单或电源功能修改时，再运行 README 中对应的原生验证命令。
指针自测会短暂替换系统指针，测试前先退出正在运行的 Ashmactool；不要与
其他指针工具同时测试。请在 PR 中说明测试环境、结果与尚未验证的范围。

## 提交修改

从自己的 Fork 创建分支，再向本仓库提交 Pull Request。说明问题和修改后的
行为，视觉修改附上预览。保持原生菜单栏工具的简洁和低开销，按修改范围
验证，避免无关改动。

提交的原创贡献使用本项目的 MIT 许可。新增依赖或外部素材时，保留来源和
许可说明；不要引入 Mousecape-swiftUI 中禁止再分发的代码或其他无授权材料。
