# AUTOCOMP · 自优化编译器

> 让编译器学会思考

## 快速开始

```bash
cargo test --workspace
cargo run -p autocomp-cli -- help
printf 'int main() {\n  %%0 = add 2, 3;\n  return %%0;\n}\n' > demo.ac
cargo run -p autocomp-cli -- cc demo.ac -O3
cargo run -p autocomp-cli -- profile demo.ac --output=autoprof.txt
cargo run -p autocomp-cli -- cc demo.ac --pgo=autoprof.txt --baseline
cargo run -p autocomp-cli -- report demo.ac --pgo=autoprof.txt --baseline --output=demo.report.html
cargo run -p autocomp-cli -- report demo.ac --pgo=autoprof.txt --baseline --format=json
cargo run -p autocomp-cli -- cc demo.ac --pgo=autoprof.txt --pgo=extra.autoprof.txt
cargo run -p autocomp-cli -- optimize demo.ac --workload='demo --quick' --output=demo.autoprof.txt
cargo run -p autocomp-cli -- verify demo.ac --pgo=demo.autoprof.txt --output=demo.verify.txt
cargo run -p autocomp-cli -- verify demo.ac --pgo=demo.autoprof.txt --strict-verify
cargo run -p autocomp-cli -- report demo.ac --pgo=demo.autoprof.txt --baseline --verify --output=demo.report.html
```

当前仓库包含 M1 可运行骨架：

- `autocomp-core`：IR 解析、Sub 指令、Profile 文本读写、优化 Pass、目标文件输出、HTML 报告
- `autocomp-cli`：`cc` / `c++` / `profile` / `optimize` / `report` / `bench`
- `optimize` 会从插桩产物收集函数采样，再执行 PGO 重编译
- IR 解析后会做定义/使用校验，重复定义或未定义值会直接报错
- `optimize` 默认执行自动 PGO 生成的样例程序，可用 `--no-run` 关闭
- `report --baseline` 会在 HTML 报告里加入 PGO 对照表
- `report --format=json` 可输出可机读的 JSON 报告，HTML 报告新增性能摘要
- Profile 会驱动分支布局和循环分块提示，随优化决策一起展示
- HTML 报告性能摘要展示 Profile 覆盖、覆盖度、热点函数与启发式评分，并注明评分不是实测性能提升
- `--pgo` 可重复传入多个 Profile，CLI 会合并后参与优化与报告
- `report --format=json` 默认输出 `*.report.json`，JSON 报告包含 Profile 覆盖度评分
- `report` 会生成带优化决策、热点函数、分支与循环 Profile 的 HTML 报告
- `optimize --output` 可指定自动 PGO 写出的 Profile 路径
- JSON 报告新增 `samples` 字段，空 Profile 会在 HTML 报告中标注回退到静态启发式
- 自动 PGO 会同时输出 workload 采样数、Profile 采样总数和函数/分支/循环覆盖数量
- 复制传播会把常量别名替换回使用点，随后可由常量折叠继续化简
- 调用图会输出候选内联提示，热点调用会优先展示
- 强度化简只处理常量乘零，避免把变量表达式重写成未定义语义的常量
- 热点函数排序复用公共排序逻辑，Pass 与报告的热点列表保持一致
- 插桩编译会额外写出 `*.instr.txt` 采样边车文件，供 Profile 收集读取
- `verify` 会把 IR 校验、Profile 合并、自动 PGO 和报告快照写成可追踪清单
- `report --verify` 会把验证清单直接嵌入 HTML/JSON 报告
- 验证清单区分 PASS / SKIP / FAIL，HTML 颜色标识，`--strict-verify` 可将 SKIP 视为失败
- 多 Profile 合并会统计采样数、重叠项和函数/分支/循环分布，并写入 HTML/JSON 报告
- 代数化简 Pass 会处理加零/减零恒等式，自动 PGO 输出分支与循环覆盖明细
- `verify --strict-check` 可把跳过的验证项视为失败，便于严格 CI 门禁

同一个世界，同一个互联网！
