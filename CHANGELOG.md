# Changelog

## 0.1.0

- 初始化 AUTOCOMP 编译器骨架、Profile 数据结构与 CLI 基础命令
- 补齐 IR 解析、常量折叠、DCE、Profile 文本格式与 HTML 报告
- `optimize` 支持从插桩产物收集采样并执行自动 PGO
- 增加 IR 校验、代数化简、函数/分支/循环采样与更完整的 HTML 报告
- `optimize` 默认运行自动 PGO 生成的样例程序，便于端到端演示
- `optimize --no-run` 仍会写出 Profile，便于离线 PGO
- `report --baseline` 展示 PGO 与基线编译的决策和产物对照
- Profile 驱动分支布局与循环分块提示进入默认 Pass，报告新增 JSON 格式与性能摘要
- HTML 报告性能摘要加入 Profile 覆盖、覆盖度、热点函数与启发式评分，并注明评分不是实测性能提升
- `--pgo` 支持重复传入并合并多个 Profile，JSON 报告新增 `mix_score` 字段
- `report --format=json` 默认写出 `*.report.json`，`cc --baseline` 输出精简对照
- `optimize --output` 可指定自动 PGO 的 Profile 输出路径
- JSON 报告新增 `samples` 字段；空 Profile 的 HTML 报告会提示回退到静态启发式
- 修复代数化简在变量加零场景下反复重写指令、DCE 可能误删指令的问题
- 强度化简收敛于常量乘零，避免变量表达式被错误折叠
- 自动 PGO 输出 workload/Profile 采样数与三类 Profile 覆盖统计
- 热点函数排序抽成公共函数，统一 Pass 与报告排序口径
- 插桩编译写出 `*.instr.txt` 边车文件，修复自动 PGO 无采样输入的问题
- 新增复制传播 Pass，常量别名在使用点替换后可继续折叠
- 新增调用图内联提示 Pass，按 Profile 热度排序候选调用边
- `verify` 输出可追踪验证清单，报告可通过 `--verify` 内嵌验证结果
- 验证清单区分 PASS / SKIP / FAIL，新增 `--strict-verify` 严格模式
- Profile 合并报告展示采样数、重叠项和三类 Profile 分布
