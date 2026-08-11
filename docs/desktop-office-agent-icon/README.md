# Desktop Office Agent — Icon Pack

黑白色、几何极简的桌面办公 Agent 应用图标。

## 设计概念

- **圆环**：带缺口的智能体环，象征处理中 / 持续运行的 AI 智能体
- **桌面线**：白色横条，代表办公工作台
- **文档线**：桌面下方三条长短不一的短线，代表任务与文档
- 整体以 -6° 轻微倾斜构成动态势，黑白色为主，圆角方形为现代桌面应用标准外形

## 文件说明

| 路径 | 内容 |
| --- | --- |
| `svg/desktop-office-agent-dark.svg` | 深色主版本（矢量源文件，可缩放到任意尺寸） |
| `svg/desktop-office-agent-light.svg` | 浅色版本（用于浅色任务栏 / 浅色主题） |
| `png/dark/…-1024.png` ~ `…-16.png` | 深色版全套 PNG（1024/512/256/128/64/48/32/16） |
| `png/light/…-1024.png` ~ `…-16.png` | 浅色版全套 PNG（同上） |
| `ico/desktop-office-agent.ico` | Windows 图标（内置 256/128/64/48/32/16 多尺寸） |
| `preview.png` | 展示图 |

## 使用方法

- **Windows**：直接使用 `ico/desktop-office-agent.ico`，或用 `png/dark/desktop-office-agent-256.png` 作为快捷方式 / 窗口图标。
- **macOS**：将 `svg/desktop-office-agent-dark.svg` 导入 Xcode Assets 生成 AppIcon（或转为 1024px PNG 后使用 `iconutil` 生成 `.icns`）。
- **Linux / Web**：使用 `png/dark` 下任意尺寸，或直接引用 SVG。

## 设计参数

- 画布：1024 × 1024，圆角方形（圆角 232）
- 主色：#0A0A0C（近黑），辅色：#F5F5F7（近白），过渡灰仅在渐变中使用
- 圆环半径 138、线宽 36；桌面线高 28；文档线高 16
