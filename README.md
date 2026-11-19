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

同一个世界，同一个互联网！
