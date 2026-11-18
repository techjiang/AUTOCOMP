use crate::{CompileResult, Profile};

pub fn render_html(title: &str, result: &CompileResult, profile: Option<&Profile>) -> String {
    let escape = |s: &str| {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    };
    let products = result.products.iter().map(|p| format!("<li><code>{}</code></li>", escape(&p.display().to_string()))).collect::<Vec<_>>().join("");
    let decisions = result.decisions.iter().map(|d| format!("<li>{}</li>", escape(d))).collect::<Vec<_>>().join("");
    let hot = profile
        .map(|p| result.features.hot_functions(&p.function_counts))
        .unwrap_or_default();
    let hot_rows = hot.iter().map(|name| {
        let count = profile.and_then(|p| p.function_counts.get(name)).copied().unwrap_or(0);
        format!("<tr><td>{}</td><td>{}</td></tr>", escape(name), count)
    }).collect::<Vec<_>>().join("");
    let hot_section = if hot_rows.is_empty() {
        String::new()
    } else {
        format!("<h2>热点函数</h2><table><thead><tr><th>函数</th><th>采样数</th></tr></thead><tbody>{hot_rows}</tbody></table>")
    };

    format!(
        "<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n<title>{}</title>\n<style>\nbody{{font-family:system-ui,sans-serif;margin:2rem;color:#1f2937}}\nh1{{color:#2563eb}}\ncode{{background:#f3f4f6;padding:.15rem .35rem;border-radius:4px}}\nli{{margin:.35rem 0}}\ntable{{border-collapse:collapse;margin-top:.5rem}}td,th{{border:1px solid #d1d5db;padding:.4rem .7rem;text-align:left}}\n</style>\n</head>\n<body>\n<h1>{}</h1>\n<p>函数 {} · 指令 {} · 调用 {} · 常量 {}</p>\n<h2>产物</h2><ul>{}</ul>\n<h2>优化决策</h2><ol>{}</ol>\n{}\n</body>\n</html>\n",
        escape(title),
        escape(title),
        result.features.function_count,
        result.features.instruction_count,
        result.features.call_count,
        result.features.constant_count,
        products,
        decisions,
        hot_section,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn html_report_escapes_and_includes_decisions() {
        let result = CompileResult {
            products: vec![PathBuf::from("demo.o")],
            features: crate::FeatureSet::default(),
            decisions: vec!["backend: emitted <object>".into()],
        };
        let html = render_html("AUTOCOMP & Report", &result, None);
        assert!(html.contains("AUTOCOMP &amp; Report"));
        assert!(html.contains("&lt;object&gt;"));
        assert!(html.contains("demo.o"));
    }
}
