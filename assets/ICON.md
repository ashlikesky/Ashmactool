# Ashmactool 图标

本版采用深蓝底色和一个白色 A 形几何标记，减少细节，保留清晰轮廓和充足留白。借鉴 [Apple App icons 指南](https://developer.apple.com/design/human-interface-guidelines/app-icons) 中简洁、可辨识、以核心图形表达身份的原则。

使用内置 imagegen 工具生成。`icon-source.png` 是原始透明图片；`AppIcon.png` 是用于打包的 1024×1024 版本。打包脚本使用 macOS 的 `sips` / `iconutil` 生成完整 `.icns` 分辨率组。

生成提示词：

> Use case: logo-brand. Asset type: new replacement macOS app icon for Ashmactool, a lightweight native utility toolbox. Design inspired by Apple's principles of simplicity and recognition, without copying any Apple app. One single bold white abstract geometric A-shaped arrow mark, made of two smooth tapered strokes and a small negative-space crossbar, centered on a calm deep slate-blue square tile. The mark must be an original clean app identity with an upward-pointing apex, visible instantly at 16 and 32 pixels. Very minimal, precise curves, generous empty space, restrained soft surface lighting, almost flat. No toolbox object, no handles, no locks, no extra symbols, no text or wordmark, no tiny details, no chrome or metallic realism, no busy gradients, no outer shadow. Rounded-square macOS icon silhouette taking 88 percent of the canvas, genuinely transparent outside the tile. Square 1024 by 1024 composition. This is one finished icon, not a presentation mockup.
