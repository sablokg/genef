//! Minimal dependency-free SVG writers so every figure can be exported.

use crate::figures::{shade_rgb, Heat, Layout};
use crate::parser::Tree;

/*
Gaurav Sablok
gsablok@proton.me
*/

pub const RED: &str = "#d73027";
pub const BLUE: &str = "#4575b4";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn header(w: f64, h: f64) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="Helvetica, Arial, sans-serif" font-size="11"><rect width="100%" height="100%" fill="white"/>"#
    )
}

/// Grouped bar chart. `series` = (legend name, css colour, values).
pub fn bars_svg(title: &str, labels: &[String], series: &[(&str, &str, &[f64])]) -> String {
    let n = labels.len().max(1) as f64;
    let k = series.len().max(1) as f64;
    let (ml, mr, mt, mb) = (60.0, 20.0, 40.0, 120.0);
    let slot = (k * 14.0 + 8.0).max(16.0);
    let w = (ml + mr + slot * n).max(600.0);
    let h = 420.0;
    let (pw, ph) = (w - ml - mr, h - mt - mb);
    let maxv = series
        .iter()
        .flat_map(|s| s.2.iter().cloned())
        .fold(0.0f64, f64::max)
        .max(1.0);

    let mut s = header(w, h);
    s += &format!(
        r#"<text x="{ml}" y="22" font-size="14" font-weight="bold">{}</text>"#,
        esc(title)
    );
    for t in 0..=5 {
        let v = maxv * t as f64 / 5.0;
        let y = mt + ph - ph * v / maxv;
        s += &format!(
            r##"<line x1="{ml}" x2="{}" y1="{y}" y2="{y}" stroke="#ddd"/><text x="{}" y="{}" text-anchor="end">{:.0}</text>"##,
            ml + pw,
            ml - 6.0,
            y + 4.0,
            v
        );
    }
    let slot = pw / n;
    let bw = slot * 0.8 / k;
    for (si, ser) in series.iter().enumerate() {
        for (i, v) in ser.2.iter().enumerate() {
            let bh = ph * v / maxv;
            let x = ml + i as f64 * slot + slot * 0.1 + si as f64 * bw;
            s += &format!(
                r#"<rect x="{x:.2}" y="{:.2}" width="{bw:.2}" height="{bh:.2}" fill="{}"/>"#,
                mt + ph - bh,
                ser.1
            );
        }
    }
    for (i, l) in labels.iter().enumerate() {
        let x = ml + (i as f64 + 0.5) * slot;
        let y = mt + ph + 12.0;
        s += &format!(
            r#"<text x="{x:.1}" y="{y}" text-anchor="end" transform="rotate(-45 {x:.1} {y})">{}</text>"#,
            esc(l)
        );
    }
    s += &format!(
        r##"<line x1="{ml}" x2="{}" y1="{y}" y2="{y}" stroke="black"/>"##,
        ml + pw,
        y = mt + ph
    );
    for (i, ser) in series.iter().enumerate() {
        let y = 20.0 + 16.0 * i as f64;
        s += &format!(
            r#"<rect x="{}" y="{}" width="10" height="10" fill="{}"/><text x="{}" y="{}">{}</text>"#,
            w - 140.0,
            y - 9.0,
            ser.1,
            w - 125.0,
            y,
            esc(ser.0)
        );
    }
    s + "</svg>"
}

/// Phylogram / cladogram annotated with expansions (+) and contractions (-) per branch.
pub fn tree_svg(tree: &Tree, lay: &Layout, change: &dyn Fn(usize) -> Option<(f64, f64)>) -> String {
    let (w, left, right) = (900.0, 30.0, 720.0);
    let leaves = lay.leaves.max(2);
    let h = leaves as f64 * 24.0 + 70.0;
    let px = |x: f64| left + (right - left) * x;
    let py = |y: f64| 30.0 + (h - 70.0) * y / (leaves - 1) as f64;

    let mut s = header(w, h);
    for (i, nd) in tree.nodes.iter().enumerate() {
        let (x, y) = (px(lay.x[i]), py(lay.y[i]));
        if let Some(p) = nd.parent {
            let xp = px(lay.x[p]);
            s += &format!(
                r##"<line x1="{xp:.1}" y1="{y:.1}" x2="{x:.1}" y2="{y:.1}" stroke="#222" stroke-width="1.4"/>"##
            );
            if let Some((inc, dec)) = change(i) {
                let mx = (xp + x) / 2.0;
                let (ya, yb) = (y - 3.0, y + 11.0);
                let (ti, td) = (format!("+{}", inc as i64), format!("-{}", dec as i64));
                // white copy underneath keeps labels legible where they cross connector lines
                for (yy, t) in [(ya, &ti), (yb, &td)] {
                    s += &format!(
                        r#"<text x="{mx:.1}" y="{yy:.1}" text-anchor="middle" fill="white" stroke="white" stroke-width="3" stroke-linejoin="round" font-size="10">{t}</text>"#
                    );
                }
                s += &format!(
                    r#"<text x="{mx:.1}" y="{ya:.1}" text-anchor="middle" fill="{RED}" font-size="10">{ti}</text><text x="{mx:.1}" y="{yb:.1}" text-anchor="middle" fill="{BLUE}" font-size="10">{td}</text>"#
                );
            }
        }
        if let (Some(&a), Some(&b)) = (nd.children.first(), nd.children.last()) {
            s += &format!(
                r##"<line x1="{x:.1}" y1="{:.1}" x2="{x:.1}" y2="{:.1}" stroke="#222" stroke-width="1.4"/>"##,
                py(lay.y[a]),
                py(lay.y[b])
            );
        } else {
            s += &format!(
                r#"<text x="{:.1}" y="{:.1}">{}</text>"#,
                x + 6.0,
                y + 4.0,
                esc(&nd.name)
            );
        }
    }
    s += &format!(
        r#"<text x="30" y="{}" fill="{RED}">+ expansions</text><text x="140" y="{}" fill="{BLUE}">- contractions</text>"#,
        h - 12.0,
        h - 12.0
    );
    s + "</svg>"
}

pub fn heatmap_svg(hm: &Heat) -> String {
    let (cw, ch, left, top) = (10.0, 16.0, 130.0, 20.0);
    let w = left + cw * hm.cols.len() as f64 + 20.0;
    let h = top + ch * hm.rows.len() as f64 + 44.0;
    let maxv = hm.m.iter().flatten().cloned().fold(1.0f64, f64::max);
    let mut s = header(w, h);
    for (r, row) in hm.m.iter().enumerate() {
        let y = top + r as f64 * ch;
        s += &format!(
            r#"<text x="{}" y="{}" text-anchor="end">{}</text>"#,
            left - 6.0,
            y + 12.0,
            esc(&hm.rows[r])
        );
        for (c, v) in row.iter().enumerate() {
            let (rr, gg, bb) = shade_rgb(v / maxv);
            s += &format!(
                r#"<rect x="{}" y="{y}" width="{cw}" height="{ch}" fill="rgb({rr},{gg},{bb})"><title>{} = {}</title></rect>"#,
                left + c as f64 * cw,
                esc(&hm.cols[c]),
                v
            );
        }
    }
    // colour scale legend: 0 .. max copies
    let ly = top + ch * hm.rows.len() as f64 + 14.0;
    for i in 0..=10 {
        let (rr, gg, bb) = shade_rgb(i as f64 / 10.0);
        s += &format!(
            r#"<rect x="{}" y="{ly}" width="12" height="10" fill="rgb({rr},{gg},{bb})"/>"#,
            left + i as f64 * 12.0
        );
    }
    s += &format!(
        r#"<text x="{}" y="{}" text-anchor="end">0</text><text x="{}" y="{}">{maxv} gene copies</text>"#,
        left - 4.0,
        ly + 9.0,
        left + 11.0 * 12.0 + 4.0,
        ly + 9.0
    );
    s + "</svg>"
}

/// Settings for [`export_all`].
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub phylo: bool,
    pub bins: usize,
    pub size_col: usize,
    pub change_col: usize,
    pub top_n: usize,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            phylo: true,
            bins: 20,
            size_col: 0,
            change_col: 0,
            top_n: 60,
        }
    }
}

/// Write every available figure as SVG into `dir`; returns the number of files written.
/// Shared by the GUI button and headless use.
pub fn export_all(
    d: &crate::parser::CafeData,
    dir: &std::path::Path,
    o: &ExportOptions,
) -> anyhow::Result<usize> {
    use crate::figures as fg;
    let mut n = 0;
    let mut write = |name: &str, s: String| -> anyhow::Result<()> {
        std::fs::write(dir.join(name), s)?;
        n += 1;
        Ok(())
    };
    if let Some(t) = &d.tree {
        let lay = fg::layout(t, o.phylo);
        write(
            "tree.svg",
            tree_svg(t, &lay, &|i| d.change_for(&t.nodes[i])),
        )?;
    }
    if !d.clades.is_empty() {
        let (l, inc, dec) = fg::branch_bars(d);
        write(
            "branch_changes.svg",
            bars_svg(
                "Expansions and contractions per branch",
                &l,
                &[
                    ("Expansions", RED, &inc[..]),
                    ("Contractions", BLUE, &dec[..]),
                ],
            ),
        )?;
    }
    if !d.families.is_empty() {
        let (l, all, sig) = fg::pvalue_hist(d, o.bins);
        write(
            "pvalue_hist.svg",
            bars_svg(
                "Family p-values",
                &l,
                &[("all", "#888888", &all[..]), ("significant", RED, &sig[..])],
            ),
        )?;
    }
    if let Some(ct) = &d.counts {
        let col = o.size_col.min(ct.columns.len().saturating_sub(1));
        let (l, c) = fg::size_dist(ct, col, 50);
        write(
            "size_distribution.svg",
            bars_svg(
                "Family size distribution",
                &l,
                &[("families", BLUE, &c[..])],
            ),
        )?;
        write("heatmap.svg", heatmap_svg(&fg::heatmap(ct, o.top_n)))?;
    }
    if let Some(ct) = &d.changes {
        let col = o.change_col.min(ct.columns.len().saturating_sub(1));
        let (l, c) = fg::change_hist(ct, col, 20);
        write(
            "change_distribution.svg",
            bars_svg("Family size change", &l, &[("families", RED, &c[..])]),
        )?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{heatmap, layout};
    use crate::parser::{parse_newick, CountTable};

    fn well_formed(s: &str) -> bool {
        s.starts_with("<svg") && s.ends_with("</svg>") && !s.contains("NaN") && !s.contains("inf")
    }

    #[test]
    fn bars_escape_and_finite() {
        let l = vec!["a<1>".to_string(), "b&c".to_string()];
        let s = bars_svg("t<>", &l, &[("x", RED, &[0.0, 0.0][..])]);
        assert!(well_formed(&s));
        assert!(s.contains("a&lt;1&gt;") && s.contains("b&amp;c"));
    }

    #[test]
    fn tree_and_heatmap_svg() {
        let t = parse_newick("((a<1>:1,b<2>:1)<3>:1,c<4>:2)<0>;").unwrap();
        let lay = layout(&t, true);
        let s = tree_svg(&t, &lay, &|i| {
            if t.nodes[i].parent.is_some() {
                Some((2.0, 1.0))
            } else {
                None
            }
        });
        assert!(well_formed(&s));
        assert!(s.contains("+2") && s.contains("-1"));
        let ct = CountTable {
            columns: vec!["a<1>".into(), "b<2>".into()],
            families: vec!["F".into()],
            values: vec![vec![1.0, 3.0]],
        };
        assert!(well_formed(&heatmap_svg(&heatmap(&ct, 5))));
    }

    #[test]
    fn export_all_writes_files() {
        use crate::parser::{CafeData, CladeRow};
        let dir = std::env::temp_dir().join(format!("cafe_gui_export_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let d = CafeData {
            clades: vec![CladeRow {
                id: "<1>".into(),
                name: "a".into(),
                inc: 1.0,
                dec: 2.0,
            }],
            ..Default::default()
        };
        assert_eq!(export_all(&d, &dir, &ExportOptions::default()).unwrap(), 1);
        assert!(dir.join("branch_changes.svg").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
