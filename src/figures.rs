//! Turns parsed CAFE data into plot-ready data. Used by both the GUI and SVG export.

use crate::parser::*;

// ------------------------------------------------------------ tree layout

/*
Gaurav Sablok
gsablok@proton.me
*/

pub struct Layout {
    pub x: Vec<f64>, // 0..1, root = 0
    pub y: Vec<f64>, // leaf index units
    pub leaves: usize,
}

pub fn layout(tree: &Tree, phylo: bool) -> Layout {
    let n = tree.nodes.len();
    let phylo = phylo && tree.nodes.iter().any(|nd| nd.length > 0.0);
    let mut x = vec![0.0; n];
    let mut y = vec![0.0; n];
    let mut next = 0.0;
    assign_y(tree, tree.root, &mut y, &mut next);
    if phylo {
        assign_x(tree, tree.root, 0.0, &mut x);
        let m = x.iter().cloned().fold(0.0f64, f64::max);
        if m > 0.0 {
            x.iter_mut().for_each(|v| *v /= m);
        }
    } else {
        let mut h = vec![0.0; n];
        heights(tree, tree.root, &mut h);
        let m = h[tree.root].max(1.0);
        for i in 0..n {
            x[i] = (m - h[i]) / m;
        }
    }
    Layout {
        x,
        y,
        leaves: next as usize,
    }
}

fn assign_y(t: &Tree, i: usize, y: &mut Vec<f64>, next: &mut f64) {
    let ch = &t.nodes[i].children;
    if ch.is_empty() {
        y[i] = *next;
        *next += 1.0;
    } else {
        for &c in ch {
            assign_y(t, c, y, next);
        }
        y[i] = (y[ch[0]] + y[*ch.last().unwrap()]) / 2.0;
    }
}

fn assign_x(t: &Tree, i: usize, acc: f64, x: &mut Vec<f64>) {
    x[i] = acc;
    for &c in &t.nodes[i].children {
        assign_x(t, c, acc + t.nodes[c].length, x);
    }
}

fn heights(t: &Tree, i: usize, h: &mut Vec<f64>) -> f64 {
    let mut m = 0.0f64;
    for &c in &t.nodes[i].children {
        m = m.max(heights(t, c, h) + 1.0);
    }
    h[i] = m;
    m
}

// ------------------------------------------------------------ bar figures

/// Per-branch expansions / contractions, sorted by total change (descending).
pub fn branch_bars(d: &CafeData) -> (Vec<String>, Vec<f64>, Vec<f64>) {
    let mut rows: Vec<&CladeRow> = d.clades.iter().collect();
    rows.sort_by(|a, b| {
        (b.inc + b.dec)
            .partial_cmp(&(a.inc + a.dec))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    (
        rows.iter().map(|r| r.display()).collect(),
        rows.iter().map(|r| r.inc).collect(),
        rows.iter().map(|r| r.dec).collect(),
    )
}

/// Histogram of family p-values: (bin labels, all families, significant families).
pub fn pvalue_hist(d: &CafeData, bins: usize) -> (Vec<String>, Vec<f64>, Vec<f64>) {
    let bins = bins.max(2);
    let mut all = vec![0.0; bins];
    let mut sig = vec![0.0; bins];
    for f in d.families.iter().filter(|f| f.pvalue.is_finite()) {
        let b = ((f.pvalue.clamp(0.0, 1.0) * bins as f64) as usize).min(bins - 1);
        all[b] += 1.0;
        if f.significant {
            sig[b] += 1.0;
        }
    }
    let labels = (0..bins)
        .map(|i| format!("{:.2}", i as f64 / bins as f64))
        .collect();
    (labels, all, sig)
}

/// Distribution of family sizes for one column (species / node).
pub fn size_dist(ct: &CountTable, col: usize, cap: usize) -> (Vec<String>, Vec<f64>) {
    let mut counts = vec![0.0; cap + 1];
    for row in &ct.values {
        if let Some(&v) = row.get(col) {
            counts[(v.round().max(0.0) as usize).min(cap)] += 1.0;
        }
    }
    let mut labels: Vec<String> = (0..cap).map(|i| i.to_string()).collect();
    labels.push(format!("{cap}+"));
    (labels, counts)
}

/// Distribution of signed family-size change for one column.
pub fn change_hist(ct: &CountTable, col: usize, lim: i64) -> (Vec<String>, Vec<f64>) {
    let mut counts = vec![0.0; (2 * lim + 1) as usize];
    for row in &ct.values {
        if let Some(&v) = row.get(col) {
            let k = (v.round() as i64).clamp(-lim, lim) + lim;
            counts[k as usize] += 1.0;
        }
    }
    let labels = (-lim..=lim).map(|i| i.to_string()).collect();
    (labels, counts)
}

// ------------------------------------------------------------ heat map

pub struct Heat {
    pub rows: Vec<String>, // species
    pub cols: Vec<String>, // families
    pub m: Vec<Vec<f64>>,  // m[species][family]
}

/// Heat map of the `top_n` families with the highest size variance across species.
pub fn heatmap(ct: &CountTable, top_n: usize) -> Heat {
    let mut idx: Vec<usize> = (0..ct.columns.len())
        .filter(|&i| !ct.columns[i].starts_with('<'))
        .collect();
    if idx.is_empty() {
        idx = (0..ct.columns.len()).collect();
    }
    let var = |row: &Vec<f64>| {
        let n = idx.len().max(1) as f64;
        let mean = idx.iter().map(|&i| row[i]).sum::<f64>() / n;
        idx.iter().map(|&i| (row[i] - mean).powi(2)).sum::<f64>() / n
    };
    let mut order: Vec<(usize, f64)> = ct
        .values
        .iter()
        .enumerate()
        .map(|(i, r)| (i, var(r)))
        .collect();
    order.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    order.truncate(top_n);

    let rows = idx
        .iter()
        .map(|&i| {
            let n = split_label(&ct.columns[i]).0;
            if n.is_empty() {
                ct.columns[i].clone()
            } else {
                n
            }
        })
        .collect();
    let cols = order.iter().map(|&(f, _)| ct.families[f].clone()).collect();
    let m = idx
        .iter()
        .map(|&s| order.iter().map(|&(f, _)| ct.values[f][s]).collect())
        .collect();
    Heat { rows, cols, m }
}

/// White -> dark blue colour ramp, t in 0..1.
pub fn shade_rgb(t: f64) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (255.0 - 247.0 * t) as u8,
        (255.0 - 207.0 * t) as u8,
        (255.0 - 148.0 * t) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ct() -> CountTable {
        CountTable {
            columns: vec!["a<1>".into(), "b<2>".into(), "<0>".into()],
            families: vec!["F1".into(), "F2".into(), "F3".into()],
            values: vec![
                vec![10.0, 0.0, 5.0],
                vec![2.0, 2.0, 2.0],
                vec![60.0, 1.0, 9.0],
            ],
        }
    }

    #[test]
    fn size_dist_caps_and_counts() {
        let (l, c) = size_dist(&ct(), 0, 50);
        assert_eq!(l.len(), c.len());
        assert_eq!(c[10], 1.0);
        assert_eq!(c[2], 1.0);
        assert_eq!(*c.last().unwrap(), 1.0); // 60 -> "50+"
        assert_eq!(l.last().unwrap(), "50+");
    }

    #[test]
    fn change_hist_clamps() {
        let t = CountTable {
            columns: vec!["x".into()],
            families: vec!["a".into(), "b".into()],
            values: vec![vec![-99.0], vec![3.0]],
        };
        let (l, c) = change_hist(&t, 0, 20);
        assert_eq!(l.len(), 41);
        assert_eq!(c[0], 1.0);
        assert_eq!(c[23], 1.0);
    }

    #[test]
    fn heatmap_ranks_by_variance_and_drops_internal_nodes() {
        let h = heatmap(&ct(), 2);
        assert_eq!(h.rows, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(h.cols, vec!["F3".to_string(), "F1".to_string()]);
        assert_eq!(h.m[0], vec![60.0, 10.0]);
    }

    #[test]
    fn pvalue_hist_bins() {
        let d = CafeData {
            families: vec![
                FamilyRow {
                    id: "a".into(),
                    pvalue: 0.0,
                    significant: true,
                },
                FamilyRow {
                    id: "b".into(),
                    pvalue: 1.0,
                    significant: false,
                },
                FamilyRow {
                    id: "c".into(),
                    pvalue: f64::NAN,
                    significant: false,
                },
            ],
            ..Default::default()
        };
        let (_, all, sig) = pvalue_hist(&d, 10);
        assert_eq!(all[0], 1.0);
        assert_eq!(all[9], 1.0);
        assert_eq!(sig.iter().sum::<f64>(), 1.0);
    }

    #[test]
    fn layout_cladogram_and_phylogram() {
        let t = parse_newick("((a:1,b:1)<2>:1,c:2)<0>;").unwrap();
        let cl = layout(&t, false);
        assert_eq!(cl.leaves, 3);
        assert!((cl.x[t.root] - 0.0).abs() < 1e-12);
        let leaf = t.nodes.iter().position(|n| n.name == "c").unwrap();
        assert!((cl.x[leaf] - 1.0).abs() < 1e-12);
        let ph = layout(&t, true);
        assert!((ph.x[leaf] - 1.0).abs() < 1e-12);
        let inner = t.nodes.iter().position(|n| n.id == "<2>").unwrap();
        assert!((ph.x[inner] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn branch_bars_sorted() {
        let d = CafeData {
            clades: vec![
                CladeRow {
                    id: "<1>".into(),
                    name: "a".into(),
                    inc: 1.0,
                    dec: 1.0,
                },
                CladeRow {
                    id: "<2>".into(),
                    name: "b".into(),
                    inc: 5.0,
                    dec: 2.0,
                },
            ],
            ..Default::default()
        };
        let (l, inc, _) = branch_bars(&d);
        assert_eq!(l[0], "b<2>");
        assert_eq!(inc, vec![5.0, 1.0]);
    }
}
