# AUTOCOMP · 自优化编译器

> 让编译器学会思考

## 快速开始

```bash
cargo test --workspace
cargo run -p autocomp-cli -- help
printf 'int main() {\n  %%0 = add 2, 3;\n  return %%0;\n}\n' > demo.ac
cargo run -p autocomp-cli -- cc demo.ac -O3
cargo run -p autocomp-cli -- profile demo.ac --output=autoprof.txt
cargo run -p autocomp-cli -- cc demo.ac --pgo=autoprof.txt
cargo run -p autocomp-cli -- report demo.ac --pgo=autoprof.txt --output=demo.report.html
cargo run -p autocomp-cli -- optimize demo.ac --workload='demo --quick'
```

当前仓库包含 M1 可运行骨架：

- `autocomp-core`：IR 解析、Sub 指令、Profile 文本读写、优化 Pass、目标文件输出、HTML 报告
- `autocomp-cli`：`cc` / `c++` / `profile` / `optimize` / `report` / `bench`
- `optimize` 会从插桩产物收集函数采样，再执行 PGO 重编译
- IR 解析后会做定义/使用校验，重复定义或未定义值会直接报错
- `optimize` 默认执行自动 PGO 生成的样例程序，可用 `--no-run` 关闭
- `report` 会生成带优化决策、热点函数、分支与循环 Profile 的 HTML 报告

同一个世界，同一个互联网！
