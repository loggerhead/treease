---
summary: "Names and responsibilities of the primary product surfaces."
read_when:
  - Naming or describing the primary UI surfaces
  - Reviewing screenshots, product copy, or cross-surface interaction behavior
---

# Surface Glossary

This glossary uses a multi-level list to describe the containment relationships
among the primary product containers visible in the UI. Indented entries are contained by the entry above them. For example:

```md
- **<English name>** — <Chinese name>：<Page position>。<Responsibility>。
  - **<Child element>**...
```

- **Sidebar** — 侧边栏：编辑器页面最左侧，贯穿工作区高度。承载编辑器级的全局入口，并提供可扩展的侧向功能容器。
  - **File Operations Group** — 文件操作组：Sidebar 上部。承载文档文件操作入口，并作为文件相关功能的扩展容器。
  - **Settings Toggles** — 设置开关组：Sidebar 中部。承载影响 Editor 或 Graph 的全局开关按钮组。
  - **Global Auxiliary Area** — 全局辅助区：Sidebar 底部。承载不参与 Editor 或 Graph 主工作流的全局辅助入口，以及账户与应用级信息。
- **Editor Pane** — 编辑器面板：编辑器页面中部左侧。承载 Editor 及其编辑器级辅助区域。
  - **Function Bar** — 编辑器功能栏：Editor Pane 顶部。承载编辑器顶部的语言、命令和辅助操作入口，并作为编辑器操作的扩展容器。
    - **Command Palette** — 命令面板：Function Bar 下方的浮层。搜索并执行当前编辑器支持的命令。
  - **Editor** — 编辑器：Editor Pane 中央主体区域。承载当前文档的源文本编辑，以及与其他工作区表面的联动。
  - **Auxiliary Input Container** — 编辑器辅助输入容器：Editor Pane 底部、Tab Switcher 上方。为 Editor 的辅助输入模式提供统一的挂载和切换边界。
    - **AI Input Panel** — AI 输入面板：Auxiliary Input Container 内。承载面向当前文档的辅助输入交互。
    - **Yq Input Box** — yq 输入框：Auxiliary Input Container 内。承载结构化查询表达式的输入交互。
  - **Tab Switcher** — 标签切换器：Editor Pane 底部左侧。承载当前工作区文档标签的切换与管理交互。
- **Graph Pane** — 图谱面板：编辑器页面中部右侧。承载 Graph、Compare 和相关图谱交互区域。
  - **Graph Top Bar** — 图谱顶栏：Graph Pane 顶部，横跨顶部控制区域。承载 Graph Pane 顶部的工作区切换和视图控制容器。
    - **Surface Switcher** — 图谱视图切换器：Graph Pane 顶部左侧。承载右侧工作区表面之间的切换入口。
    - **Graph View Controls** — 图谱视图控制组：Graph Top Bar 右侧。承载当前 Graph 或 Compare 工作区的视图级控制入口，并作为视图控制的扩展容器。
      - **Graph Search Panel** — 图搜索面板：Graph View Controls 搜索入口下方的浮层。承载面向当前图谱的搜索输入、结果和导航交互。
  - **Graph** — 图谱：Graph Pane 中央主体区域。承载当前结构化文档的结构化可视化与交互工作区。
  - **Compare Surface** — 对比工作区：Graph Pane 中央主体区域，在对比模式下显示。承载双文档对照场景的交互内容与状态。
  - **Column Navigator** — 列导航器：Graph Pane 底部主体区域。承载沿结构化路径展开的嵌套内容浏览与编辑工作区。
  - **Tree Path Bar** — Tree Path 路径栏：Graph Pane 底部、Column Navigator 下方。承载当前结构路径的定位信息与路径级操作入口。
