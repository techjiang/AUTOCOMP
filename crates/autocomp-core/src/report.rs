use crate::{CompileResult, Profile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportFormat {
    Html,
    Json,
}


pub fn render(title: &str, result: &CompileResult, profile: Option<&Profile>, baseline: Option<&CompileResult>, verify: Option<&crate::VerifyReport>, merge: Option<&crate::ProfileMergeReport>, format: ReportFormat) -> String {
    match format {
        ReportFormat::Html => render_html(title, result, profile, baseline, verify, merge),
        ReportFormat::Json => render_json(title, result, profile, baseline, verify, merge),
    }
}

fn profile_mix(profile: Option<&Profile>) -> f64 {
    let Some(profile) = profile else {
        return 0.0;
    };
    let function_samples = profile.function_counts.values().copied().sum::<u64>();
    let branch_samples = profile
        .branch_counts
        .values()
        .map(|(false_count, true_count)| false_count + true_count)
        .sum::<u64>();
    let loop_samples = profile.loop_counts.values().copied().sum::<u64>();
    let total = function_samples + branch_samples + loop_samples;
    if total == 0 {
        return 0.0;
    }
    let mixed = [function_samples, branch_samples, loop_samples].iter().filter(|count| **count > 0).count() as f64;
    mixed / 3.0
}

pub fn render_json(title: &str, result: &CompileResult, profile: Option<&Profile>, baseline: Option<&CompileResult>, verify: Option<&crate::VerifyReport>, merge: Option<&crate::ProfileMergeReport>) -> String {
    fn escape(value: &str) -> String {
        let mut out = String::new();
        for ch in value.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out
    }
    fn array(values: impl IntoIterator<Item = String>) -> String {
        format!("[{}]", values.into_iter().map(|v| format!("\"{v}\"")).collect::<Vec<_>>().join(","))
    }
    let products = array(result.products.iter().map(|p| escape(&p.display().to_string())));
    let decisions = array(result.decisions.iter().map(|d| escape(d)));
    let hot = profile.map(|p| result.features.hot_functions(&p.function_counts)).unwrap_or_default();
    let hot_counts = profile.map(|p| hot.iter().map(|name| format!("[\"{}\",{}]", escape(name), p.function_counts.get(name).copied().unwrap_or(0))).collect::<Vec<_>>().join(",")).unwrap_or_default();
    let baseline_json = baseline.map(|base| format!(
        "{{\"decisions\":{},\"products\":{}}}",
        base.decisions.len(), base.products.len()
    )).unwrap_or_else(|| "null".into());
    let verify_json = verify.map(|v| format!(
        "{{\"passed\":{},\"count\":{},\"checks\":[{}]}}",
        if v.all_passed() { "true" } else { "false" },
        v.checks.len(),
        v.checks.iter().map(|c| {
            let status = if let Some(skip) = &c.skipped {
                format!("SKIP:{}", escape(skip))
            } else if c.passed {
                "PASS".into()
            } else {
                "FAIL".into()
            };
            format!("{{\"name\":\"{}\",\"passed\":{},\"status\":\"{}\",\"detail\":\"{}\"}}", escape(&c.name), if c.passed { "true" } else { "false" }, status, escape(&c.detail))
        }).collect::<Vec<_>>().join(",")
    )).unwrap_or_else(|| "null".into());
    let merge_json = merge.map(|m| format!(
        "{{\"samples\":{},\"overlaps\":{},\"function_samples\":{},\"branch_samples\":{},\"loop_samples\":{}}}",
        m.sample_count(), m.overlap_count(), m.function_samples, m.branch_samples, m.loop_samples
    )).unwrap_or_else(|| "null".into());
    let profile_json = profile.map(|p| format!(
        "{{\"functions\":{},\"branches\":{},\"loops\":{},\"samples\":{},\"mix_score\":{:.2},\"hot\":[{}]}}",
        p.function_counts.len(), p.branch_counts.len(), p.loop_counts.len(),
        p.function_counts.values().copied().sum::<u64>()
            + p.branch_counts.values().map(|(a, b)| a + b).sum::<u64>()
            + p.loop_counts.values().copied().sum::<u64>(),
        profile_mix(Some(p)), hot_counts
    )).unwrap_or_else(|| "null".into());
    format!(
        "{{\"title\":\"{}\",\"features\":{{\"functions\":{},\"instructions\":{},\"calls\":{},\"constants\":{}}},\"products\":{},\"decisions\":{},\"profile\":{},\"baseline\":{},\"verify\":{},\"merge\":{}}}",
        escape(title), result.features.function_count, result.features.instruction_count,
        result.features.call_count, result.features.constant_count,
        products, decisions, profile_json, baseline_json, verify_json, merge_json
    )
}

pub fn render_html(title: &str, result: &CompileResult, profile: Option<&Profile>, baseline: Option<&CompileResult>, verify: Option<&crate::VerifyReport>, merge: Option<&crate::ProfileMergeReport>) -> String {
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
    let branch_rows = profile
        .map(|p| {
            let mut branches: Vec<_> = p.branch_counts.iter().collect();
            branches.sort_by(|a, b| (b.1.0 + b.1.1).cmp(&(a.1.0 + a.1.1)).then(a.0.cmp(b.0)));
            branches.into_iter().take(8).map(|(name, (false_count, true_count))| {
                let total = false_count + true_count;
                let percent = if total == 0 { 0.0 } else { *true_count as f64 * 100.0 / total as f64 };
                format!("<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.1}%</td></tr>", escape(name), false_count, true_count, percent)
            }).collect::<Vec<_>>().join("")
        })
        .unwrap_or_default();
    let branch_section = if branch_rows.is_empty() {
        String::new()
    } else {
        format!("<h2>分支 Profile</h2><table><thead><tr><th>分支</th><th>false</th><th>true</th><th>true 比例</th></tr></thead><tbody>{branch_rows}</tbody></table>")
    };
    let loop_rows = profile
        .map(|p| {
            let mut loops: Vec<_> = p.loop_counts.iter().collect();
            loops.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            loops.into_iter().take(8).map(|(name, count)| {
                format!("<tr><td>{}</td><td>{}</td></tr>", escape(name), count)
            }).collect::<Vec<_>>().join("")
        })
        .unwrap_or_default();
    let loop_section = if loop_rows.is_empty() {
        String::new()
    } else {
        format!("<h2>循环 Profile</h2><table><thead><tr><th>循环</th><th>采样数</th></tr></thead><tbody>{loop_rows}</tbody></table>")
    };
    let baseline_section = baseline.map(|base| {
        let decisions_delta = result.decisions.len() as i64 - base.decisions.len() as i64;
        let products_delta = result.products.len() as i64 - base.products.len() as i64;
        format!(
            "<h2>PGO 对照</h2><table><thead><tr><th>指标</th><th>基线</th><th>PGO</th><th>变化</th></tr></thead><tbody><tr><td>优化决策</td><td>{}</td><td>{}</td><td>{:+}</td></tr><tr><td>产物数量</td><td>{}</td><td>{}</td><td>{:+}</td></tr></tbody></table>",
            base.decisions.len(), result.decisions.len(), decisions_delta,
            base.products.len(), result.products.len(), products_delta,
        )
    }).unwrap_or_default();
    let profile_beats = profile.map(|p| p.function_counts.len()).unwrap_or(0);
    let profile_hot = profile
        .map(|p| result.features.hot_functions(&p.function_counts).len())
        .unwrap_or(0);
    let confidence = if profile_beats == 0 { 0.0 } else { 25.0 + 75.0 * profile_mix(profile) };
    let estimated_gain = if baseline.is_some() {
        let decisions_gain = result.decisions.len() as f64 * 1.5;
        let hot_gain = profile_hot as f64 * 1.5;
        (decisions_gain + hot_gain) * confidence / 100.0
    } else {
        0.0
    };
    let perf_section = baseline.map(|base| {
        let decisions_delta = result.decisions.len() as i64 - base.decisions.len() as i64;
        let kept_instructions = result.features.instruction_count as i64 - base.features.instruction_count as i64;
        format!(
            "<h2>性能摘要</h2><div class=\"panel\"><span class=\"metric\">决策 {}（{}）</span><span class=\"metric\">指令 {}（{}）</span><span class=\"metric\">Profile {} / 热点 {}</span><span class=\"metric\">覆盖度 {:.0}%</span><span class=\"metric gain\">启发式评分 +{:.1}%</span></div><p class=\"muted\">评分只表示决策置信度，不是实测性能提升；需以 bench 或基准测试为准。</p>",
            result.decisions.len(),
            if decisions_delta >= 0 { format!("+{decisions_delta}") } else { decisions_delta.to_string() },
            result.features.instruction_count,
            if kept_instructions >= 0 { format!("+{kept_instructions}") } else { kept_instructions.to_string() },
            profile_beats,
            profile_hot,
            confidence,
            estimated_gain,
        )
    }).unwrap_or_else(|| format!(
        "<h2>性能摘要</h2><div class=\"panel\"><span class=\"metric\">决策 {}</span><span class=\"metric\">指令 {}</span><span class=\"metric\">Profile {} / 热点 {}</span><span class=\"metric gain\">基线模式</span></div>",
        result.decisions.len(), result.features.instruction_count, profile_beats, profile_hot
    ));
    let coverage_section = profile.map(|p| {
        let total = p.function_counts.values().copied().sum::<u64>();
        let top = crate::analysis::ranked_profile_entries(&p.function_counts, 3);
        if top.is_empty() {
            return String::new();
        }
        let rows = top.into_iter().map(|(name, count)| {
            let share = if total == 0 { 0.0 } else { count as f64 * 100.0 / total as f64 };
            format!("<tr><td>{}</td><td>{}</td><td>{:.1}%</td></tr>", escape(&name), count, share)
        }).collect::<Vec<_>>().join("");
        format!("<h2>热点分布</h2><table><thead><tr><th>函数</th><th>采样</th><th>占比</th></tr></thead><tbody>{rows}</tbody></table>")
    }).unwrap_or_default();
    let profile_note = profile.map(|p| {
        let samples = p.function_counts.values().copied().sum::<u64>()
            + p.branch_counts.values().map(|(a, b)| a + b).sum::<u64>()
            + p.loop_counts.values().copied().sum::<u64>();
        if samples > 0 { String::new() } else { "<p class=\"muted\">当前 Profile 为空，优化决策回退到静态启发式。</p>".into() }
    }).unwrap_or_default();
    let merge_section = merge.map(|m| format!(
        "<h2>Profile 合并</h2><div class=\"panel\"><span class=\"metric\">合并采样 {}</span><span class=\"metric\">重叠项 {}</span><span class=\"metric\">函数 {}</span><span class=\"metric\">分支 {}</span><span class=\"metric\">循环 {}</span></div>",
        m.sample_count(), m.overlap_count(), m.function_samples, m.branch_samples, m.loop_samples
    )).unwrap_or_default();
    let verify_section = verify.map(|v| {
        let rows = v.checks.iter().map(|c| {
            let (status, cls) = if let Some(reason) = &c.skipped {
                (format!("SKIP · {}", escape(reason)), "skip")
            } else if c.passed {
                ("PASS".into(), "pass")
            } else {
                ("FAIL".into(), "fail")
            };
            format!("<tr><td class=\"{cls}\">{}</td><td>{}</td><td>{}</td></tr>", status, escape(&c.name), escape(&c.detail))
        }).collect::<Vec<_>>().join("");
        let passed = v.passed_count();
        let skipped = v.skipped_count();
        let failed = v.checks.len() - passed - skipped;
        let summary = format!("{} pass / {} skip / {} fail", passed, skipped, failed);
        format!("<h2>验证清单</h2><div class=\"panel\"><span class=\"metric\">{}</span></div><table class=\"checks\"><thead><tr><th>状态</th><th>项目</th><th>说明</th></tr></thead><tbody>{}</tbody></table>", summary, rows)
    }).unwrap_or_default();
    let hot_section = if hot_rows.is_empty() {
        String::new()
    } else {
        format!("<h2>热点函数</h2><table><thead><tr><th>函数</th><th>采样数</th></tr></thead><tbody>{hot_rows}</tbody></table>")
    };

    let html = format!(
        "<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n<title>{}</title>\n<style>\nbody{{font-family:system-ui,sans-serif;margin:2rem;color:#1f2937}}\nh1{{color:#2563eb}}\ncode{{background:#f3f4f6;padding:.15rem .35rem;border-radius:4px}}\nli{{margin:.35rem 0}}\ntable{{border-collapse:collapse;margin-top:.5rem}}td,th{{border:1px solid #d1d5db;padding:.4rem .7rem;text-align:left}}\n.panel{{background:#f8fafc;border-left:4px solid #2563eb;padding:.8rem 1rem;margin:1rem 0;display:flex;gap:1.2rem;flex-wrap:wrap;align-items:center}}\n.metric{{font-weight:600;color:#0f172a}}\n.gain{{color:#047857;background:#d1fae5;border-radius:999px;padding:.2rem .6rem;font-size:.9rem}}\n.muted{{color:#64748b;font-size:.9rem}}\n.checks td:first-child{{font-weight:700}}\n.checks .pass{{color:#047857;background:#d1fae5}}\n.checks .fail{{color:#b91c1c;background:#fee2e2}}\n.checks .skip{{color:#b45309;background:#fef3c7}}\n</style>\n</head>\n<body>\n<h1>{}</h1>\n<div class=\"panel\">函数 {} · 指令 {} · 调用 {} · 常量 {}</div>\n<h2>产物</h2><ul>{}</ul>\n<h2>优化决策</h2><ol>{}</ol>\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n</body>\n</html>\n",
        escape(title),
        escape(title),
        result.features.function_count,
        result.features.instruction_count,
        result.features.call_count,
        result.features.constant_count,
        products,
        decisions,
        perf_section,
        baseline_section,
        profile_note,
        merge_section,
        verify_section,
        hot_section,
        branch_section,
        loop_section,
        coverage_section,
    );
    html
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
        let html = render_html("AUTOCOMP & Report", &result, None, None, None, None);
        assert!(html.contains("AUTOCOMP &amp; Report"));
        assert!(html.contains("&lt;object&gt;"));
        assert!(html.contains("demo.o"));
    }

    #[test]
    fn html_report_includes_profile_sections() {
        let result = CompileResult {
            products: vec![PathBuf::from("demo.opt.o")],
            features: crate::FeatureSet::default(),
            decisions: vec!["pgo: hot function main".into()],
        };
        let mut profile = crate::Profile::default();
        profile.function_counts.insert("main".into(), 12);
        profile.branch_counts.insert("entry:main".into(), (4, 8));
        profile.loop_counts.insert("const-0".into(), 5);
        let html = render_html("PGO Report", &result, Some(&profile), None, None, None);
        assert!(html.contains("分支 Profile"));
        assert!(html.contains("循环 Profile"));
        assert!(html.contains("66.7%"));
    }

    #[test]
    fn json_report_includes_profile_and_baseline() {
        let result = CompileResult {
            products: vec![PathBuf::from("demo.opt.o")],
            features: crate::FeatureSet::default(),
            decisions: vec!["pgo: hot function main".into()],
        };
        let baseline = CompileResult {
            products: vec![PathBuf::from("demo.o")],
            features: crate::FeatureSet::default(),
            decisions: vec!["backend: emitted demo.o".into()],
        };
        let mut profile = crate::Profile::default();
        profile.function_counts.insert("main".into(), 9);
        let json = render_json("AUTOCOMP", &result, Some(&profile), Some(&baseline), None, None);
        assert!(json.contains("\"title\":\"AUTOCOMP\""));
        assert!(json.contains("\"profile\""));
        assert!(json.contains("\"baseline\""));
        assert!(json.contains("main"));
    }

    #[test]
    fn html_report_shows_performance_summary() {
        let result = CompileResult {
            products: vec![PathBuf::from("demo.opt.o")],
            features: crate::FeatureSet::default(),
            decisions: vec!["pgo: hot function main".into(), "branch-layout: layout 1 hot branches".into()],
        };
        let baseline = CompileResult {
            products: vec![PathBuf::from("demo.o")],
            features: crate::FeatureSet::default(),
            decisions: vec!["backend: emitted demo.o".into()],
        };
        let mut profile = crate::Profile::default();
        profile.function_counts.insert("main".into(), 9);
        let html = render_html("PGO Summary", &result, Some(&profile), Some(&baseline), None, None);
        assert!(html.contains("性能摘要"));
        assert!(html.contains("启发式评分"));
        assert!(html.contains("Profile 1 / 热点 1"));
    }

    #[test]
    fn json_report_exposes_profile_mix_score() {
        let result = CompileResult {
            products: vec![PathBuf::from("demo.opt.o")],
            features: crate::FeatureSet::default(),
            decisions: vec!["pgo: hot function main".into()],
        };
        let mut profile = crate::Profile::default();
        profile.function_counts.insert("main".into(), 9);
        profile.branch_counts.insert("entry:main".into(), (4, 5));
        profile.loop_counts.insert("const-0".into(), 7);
        let json = render_json("AUTOCOMP", &result, Some(&profile), None, None, None);
        assert!(json.contains("\"mix_score\":1.00"));
    }

    #[test]
    fn html_report_includes_baseline_comparison() {
        let result = CompileResult {
            products: vec![PathBuf::from("demo.opt.o")],
            features: crate::FeatureSet::default(),
            decisions: vec!["pgo: hot function main".into(), "backend: emitted optimized demo.opt.o".into()],
        };
        let baseline = CompileResult {
            products: vec![PathBuf::from("demo.o")],
            features: crate::FeatureSet::default(),
            decisions: vec!["backend: emitted demo.o".into()],
        };
        let html = render_html("PGO Compare", &result, None, Some(&baseline), None, None);
        assert!(html.contains("PGO 对照"));
        assert!(html.contains("+1"));
    }
}

#[cfg(test)]
mod extra_tests {
    use super::*;
    use std::path::PathBuf;

    fn result() -> CompileResult {
        CompileResult {
            products: vec![PathBuf::from("demo.opt.o")],
            features: crate::FeatureSet::default(),
            decisions: vec!["pgo: loaded 1 sampled functions".into()],
        }
    }

    #[test]
    fn json_report_exposes_profile_samples() {
        let mut profile = crate::Profile::default();
        profile.function_counts.insert("main".into(), 3);
        profile.branch_counts.insert("entry:main".into(), (1, 2));
        profile.loop_counts.insert("const-0".into(), 4);
        let json = render_json("AUTOCOMP", &result(), Some(&profile), None, None, None);
        assert!(json.contains("\"samples\":10"));
    }

    #[test]
    fn html_report_notes_empty_profile() {
        let profile = crate::Profile::default();
        let html = render_html("Empty PGO", &result(), Some(&profile), None, None, None);
        assert!(html.contains("当前 Profile 为空"));
    }
}

#[cfg(test)]
mod verify_tests {
    use super::*;

    #[test]
    fn report_includes_merge_section() {
        let merge = crate::ProfileMergeReport {
            function_overlap: 1,
            branch_overlap: 1,
            loop_overlap: 0,
            function_samples: 5,
            branch_samples: 4,
            loop_samples: 3,
        };
        let result = crate::CompileResult { products: vec![], features: crate::FeatureSet::default(), decisions: vec![] };
        let html = render_html("Merge", &result, None, None, None, Some(&merge));
        assert!(html.contains("Profile 合并"));
        assert!(html.contains("合并采样 12"));
        let json = render_json("Merge", &result, None, None, None, Some(&merge));
        assert!(json.contains("\"merge\""));
        assert!(json.contains("\"overlaps\":2"));
    }

    #[test]
    fn report_includes_verify_section() {
        let profile = crate::Profile::default();
        let verify = crate::VerifyReport {
            checks: vec![crate::VerifyCheck {
                name: "ir-verify".into(),
                passed: true,
                detail: "1 functions parsed".into(),
                skipped: None,
            }],
        };
        let result = crate::CompileResult {
            products: vec![std::path::PathBuf::from("demo.o")],
            features: crate::FeatureSet::default(),
            decisions: vec![],
        };
        let html = render_html("Verify", &result, Some(&profile), None, Some(&verify), None);
        assert!(html.contains("验证清单"));
        assert!(html.contains("1 pass / 0 skip / 0 fail"));
        let json = render_json("Verify", &result, Some(&profile), None, Some(&verify), None);
        assert!(json.contains("\"verify\""));
    }
}
