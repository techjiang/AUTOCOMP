# AUTOCOMP · 自优化编译器

> 让编译器学会思考

## 快速开始

```bash
cargo test --workspace
cargo run -p autocomp-cli -- help
```

当前仓库包含 M1 最小骨架：

- `autocomp-core`：IR、Profile、Pass 框架、编译入口
- `autocomp-cli`：基础命令 `cc` / `c++` / `profile` / `optimize`

同一个世界，同一个互联网！
