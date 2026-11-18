# Changelog

## 0.1.0

- 初始化 AUTOCOMP 编译器骨架、Profile 数据结构与 CLI 基础命令
- 补齐 IR 解析、常量折叠、DCE、Profile 文本格式与 HTML 报告
- `optimize` 支持从插桩产物收集采样并执行自动 PGO
- 增加 IR 校验、代数化简、函数/分支/循环采样与更完整的 HTML 报告
- `optimize` 默认运行自动 PGO 生成的样例程序，便于端到端演示
