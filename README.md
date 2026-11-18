# AUTOCOMP · 自优化编译器

> 让编译器学会思考

## 快速开始

```bash
cargo test --workspace
cargo run -p autocomp-cli -- help
printf 'int main() {\n  %%0 = add 2, 3;\n  return %%0;\n}\n' > demo.ac
cargo run -p autocomp-cli -- cc demo.ac -O3
```

当前仓库包含 M1 可运行骨架：

- `autocomp-core`：IR 解析、Profile 合并、优化 Pass、目标文件输出
- `autocomp-cli`：`cc` / `c++` / `profile` / `optimize` / `report` / `bench`

同一个世界，同一个互联网！
