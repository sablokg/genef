//! Parsers for CAFE5 output files and Newick / Nexus trees.

use anyhow::{bail, Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/*
Gaurav Sablok
gsablok@proton.me
*/

/// Result-file suffixes. Longer / more specific suffixes come first so that
/// prefix detection works (`Base_clade_results.txt` must not match `_results.txt`).
pub const SUFFIXES: [&str; 9] = [
    "_clade_results.txt",
    "_family_results.txt",
    "_branch_probabilities.tab",
    "_count.tab",
    "_change.tab",
    "_asr.tre",
    "_results.txt",
    "_report.cafe",
    ".cafe",
];

/// Split a CAFE label such as `cat<0>` or `<3>_12` into (name, id).
/// `id` is the `<n>` part when present, otherwise the whole label.
pub fn split_label(label: &str) -> (String, String) {
    if let (Some(a), Some(b)) = (label.find('<'), label.find('>')) {
        if a < b {
            let name = label[..a].trim_end_matches('_').trim().to_string();
            let id = label[a..=b].to_string();
            return (name, id);
        }
    }
    let l = label.trim().to_string();
    (l.clone(), l)
}

#[derive(Debug, Clone, Default)]
pub struct Node {
    pub name: String,
    pub id: String,
    pub length: f64,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: usize,
}

impl Tree {
    pub fn is_leaf(&self, i: usize) -> bool {
        self.nodes[i].children.is_empty()
    }
}

/// Extract the Newick string from plain Newick or Nexus text.
fn extract_newick(text: &str) -> Result<String> {
    // drop [ ... ] comments
    let mut clean = String::with_capacity(text.len());
    let mut depth = 0;
    for ch in text.chars() {
        match ch {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => clean.push(ch),
            _ => {}
        }
    }
    let start = clean.find('(').context("no '(' found: not a Newick tree")?;
    let end = clean[start..]
        .find(';')
        .map(|e| start + e)
        .unwrap_or(clean.len());
    Ok(clean[start..end]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect())
}

pub fn parse_newick(text: &str) -> Result<Tree> {
    let s = extract_newick(text)?;
    let b = s.as_bytes();
    let mut pos = 0usize;
    let mut nodes = Vec::new();
    let root = parse_sub(b, &mut pos, &mut nodes, None)?;
    Ok(Tree { nodes, root })
}

fn parse_sub(
    b: &[u8],
    pos: &mut usize,
    nodes: &mut Vec<Node>,
    parent: Option<usize>,
) -> Result<usize> {
    let idx = nodes.len();
    nodes.push(Node {
        parent,
        ..Default::default()
    });
    if b.get(*pos) == Some(&b'(') {
        *pos += 1;
        loop {
            let c = parse_sub(b, pos, nodes, Some(idx))?;
            nodes[idx].children.push(c);
            match b.get(*pos) {
                Some(b',') => *pos += 1,
                Some(b')') => {
                    *pos += 1;
                    break;
                }
                _ => bail!("malformed Newick near byte {}", *pos),
            }
        }
    }
    let start = *pos;
    while *pos < b.len() && !matches!(b[*pos], b':' | b',' | b')' | b';' | b'(') {
        *pos += 1;
    }
    let label = String::from_utf8_lossy(&b[start..*pos]).to_string();
    let (name, id) = split_label(&label);
    nodes[idx].name = name;
    nodes[idx].id = id;
    if b.get(*pos) == Some(&b':') {
        *pos += 1;
        let s2 = *pos;
        while *pos < b.len() && !matches!(b[*pos], b',' | b')' | b';') {
            *pos += 1;
        }
        nodes[idx].length = String::from_utf8_lossy(&b[s2..*pos]).parse().unwrap_or(0.0);
    }
    Ok(idx)
}

pub fn read_tree(path: &Path) -> Result<Tree> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_newick(&text)
}

// ---------------------------------------------------------------- tables

pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn read_table(path: &Path) -> Result<Table> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let first = lines.next().context("empty file")?;
    let tabbed = first.contains('\t');
    let split = |l: &str| -> Vec<String> {
        if tabbed {
            l.split('\t').map(|s| s.trim().to_string()).collect()
        } else {
            l.split_whitespace().map(String::from).collect()
        }
    };
    let header = split(first.trim_start_matches('#'));
    let rows = lines.map(split).collect();
    Ok(Table { header, rows })
}

#[derive(Debug, Clone)]
pub struct CladeRow {
    pub id: String,
    pub name: String,
    pub inc: f64,
    pub dec: f64,
}

impl CladeRow {
    pub fn display(&self) -> String {
        if self.name.is_empty() {
            self.id.clone()
        } else {
            format!("{}{}", self.name, self.id)
        }
    }
}

#[derive(Debug, Clone)]
pub struct FamilyRow {
    pub id: String,
    pub pvalue: f64,
    pub significant: bool,
}

/// Family x column numeric table (`*_count.tab`, `*_change.tab`).
#[derive(Debug, Clone, Default)]
pub struct CountTable {
    pub columns: Vec<String>,
    pub families: Vec<String>,
    pub values: Vec<Vec<f64>>, // values[family][column]
}

fn to_counts(t: &Table) -> CountTable {
    let columns: Vec<String> = t.header.iter().skip(1).cloned().collect();
    let mut ct = CountTable {
        columns,
        ..Default::default()
    };
    for r in &t.rows {
        if r.is_empty() {
            continue;
        }
        ct.families.push(r[0].clone());
        let mut v: Vec<f64> = r.iter().skip(1).map(|s| s.parse().unwrap_or(0.0)).collect();
        v.resize(ct.columns.len(), 0.0);
        ct.values.push(v);
    }
    ct
}

#[derive(Default)]
pub struct CafeData {
    pub tree: Option<Tree>,
    pub clades: Vec<CladeRow>,
    pub families: Vec<FamilyRow>,
    pub counts: Option<CountTable>,
    pub changes: Option<CountTable>,
    pub results_text: String,
    pub loaded: Vec<String>,
}

impl CafeData {
    pub fn load(dir: &Path, prefix: &str) -> Result<Self> {
        let f = |suf: &str| dir.join(format!("{prefix}{suf}"));
        let mut d = CafeData::default();

        let p = f("_clade_results.txt");
        if p.exists() {
            for r in read_table(&p)?.rows {
                if r.len() >= 3 {
                    let (name, id) = split_label(&r[0]);
                    d.clades.push(CladeRow {
                        id,
                        name,
                        inc: r[1].parse().unwrap_or(0.0),
                        dec: r[2].parse().unwrap_or(0.0),
                    });
                }
            }
            d.loaded.push("clade results".into());
        }
        let p = f("_family_results.txt");
        if p.exists() {
            for r in read_table(&p)?.rows {
                if r.len() >= 2 {
                    d.families.push(FamilyRow {
                        id: r[0].clone(),
                        pvalue: r[1].parse().unwrap_or(f64::NAN),
                        significant: r
                            .get(2)
                            .map(|s| s.to_lowercase().starts_with('y'))
                            .unwrap_or(false),
                    });
                }
            }
            d.loaded.push("family results".into());
        }
        let p = f("_count.tab");
        if p.exists() {
            d.counts = Some(to_counts(&read_table(&p)?));
            d.loaded.push("counts".into());
        }
        let p = f("_change.tab");
        if p.exists() {
            d.changes = Some(to_counts(&read_table(&p)?));
            d.loaded.push("changes".into());
        }
        let p = f("_asr.tre");
        if p.exists() {
            d.tree = Some(read_tree(&p)?);
            d.loaded.push("tree".into());
        }
        let p = f("_results.txt");
        if p.exists() {
            d.results_text = fs::read_to_string(&p).unwrap_or_default();
            d.loaded.push("model results".into());
        }
        if d.loaded.is_empty() {
            // no per-file CAFE5 output: fall back to a single report (`Base_report.cafe` or `name.cafe`)
            for suf in ["_report.cafe", ".cafe"] {
                let cafe = f(suf);
                if cafe.is_file() {
                    return Self::load_cafe4(&cafe);
                }
            }
            bail!(
                "no CAFE result files with prefix '{prefix}' in {}",
                dir.display()
            );
        }
        Ok(d)
    }

    /// (expansions, contractions) for a tree node, matched by `<id>` or by name.
    pub fn change_for(&self, n: &Node) -> Option<(f64, f64)> {
        self.clades
            .iter()
            .find(|c| c.id == n.id || (!n.name.is_empty() && c.name == n.name))
            .map(|c| (c.inc, c.dec))
    }
}

// ------------------------------------------------- CAFE (v4) `.cafe` report

fn pairs_after_colon(line: &str) -> Vec<(f64, f64)> {
    let body = line.split_once(':').map(|x| x.1).unwrap_or("");
    body.split('\t')
        .filter_map(|t| {
            let t = t.trim().trim_start_matches('(').trim_end_matches(')');
            let (a, b) = t.split_once(',')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .collect()
}

fn node_pairs(line: &str) -> Vec<(usize, usize)> {
    let body = line.split_once("): ").map(|x| x.1).unwrap_or("");
    body.split_whitespace()
        .filter_map(|t| {
            let (a, b) = t.trim_matches(|c| c == '(' || c == ')').split_once(',')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect()
}

/// Family size encoded in a CAFE report node label (`hsa_25` -> 25, `_34` -> 34).
fn label_size(name: &str) -> f64 {
    name.rsplit_once('_')
        .and_then(|(_, s)| s.parse().ok())
        .unwrap_or(0.0)
}

impl CafeData {
    /// Load a single CAFE (v4-style) `.cafe` report: species tree with node IDs,
    /// per-branch expansion / decrease counts, and one Newick line per family with
    /// the family size at every node plus a family-wide p-value.
    /// Family p < 0.05 is flagged significant (the file does not store the threshold).
    pub fn load_cafe4(path: &Path) -> Result<Self> {
        let text =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let lines: Vec<&str> = text.lines().collect();
        let find = |pre: &str| lines.iter().find(|l| l.starts_with(pre)).copied();

        let ids_line =
            find("# IDs of nodes:").context("not a .cafe report: no '# IDs of nodes:' line")?;
        let mut tree = parse_newick(ids_line)?;
        if let Some(l) = find("Tree:") {
            if let Ok(t2) = parse_newick(l) {
                if t2.nodes.len() == tree.nodes.len() {
                    for (a, b) in tree.nodes.iter_mut().zip(&t2.nodes) {
                        a.length = b.length;
                    }
                }
            }
        }
        let num = |id: &str| {
            id.trim_matches(|c| c == '<' || c == '>')
                .parse::<usize>()
                .ok()
        };
        let by_num: std::collections::HashMap<usize, usize> = tree
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| num(&n.id).map(|k| (k, i)))
            .collect();

        let mut d = CafeData::default();
        let pairs = find("# Output format for: ")
            .map(node_pairs)
            .unwrap_or_default();
        let exp = find("Expansion").map(pairs_after_colon).unwrap_or_default();
        let dec = find("Decrease").map(pairs_after_colon).unwrap_or_default();
        for (k, &(a, b)) in pairs.iter().enumerate() {
            for (side, n) in [a, b].into_iter().enumerate() {
                let (Some(&ni), Some(e), Some(c)) = (by_num.get(&n), exp.get(k), dec.get(k)) else {
                    continue;
                };
                let nd = &tree.nodes[ni];
                let pick = |t: &(f64, f64)| if side == 0 { t.0 } else { t.1 };
                d.clades.push(CladeRow {
                    id: nd.id.clone(),
                    name: nd.name.clone(),
                    inc: pick(e),
                    dec: pick(c),
                });
            }
        }
        if !d.clades.is_empty() {
            d.loaded.push("clade results".into());
        }

        let label = |i: usize| {
            let n = &tree.nodes[i];
            if tree.is_leaf(i) {
                format!("{}{}", n.name, n.id)
            } else {
                n.id.clone()
            }
        };
        let cols: Vec<String> = (0..tree.nodes.len()).map(label).collect();
        let nonroot: Vec<usize> = (0..tree.nodes.len()).filter(|&i| i != tree.root).collect();
        let mut counts = CountTable {
            columns: cols.clone(),
            ..Default::default()
        };
        let mut changes = CountTable {
            columns: nonroot.iter().map(|&i| cols[i].clone()).collect(),
            ..Default::default()
        };
        let mut skipped = 0usize;
        // CAFE5 writes the `'ID'\t'Newick'` header without a newline, so the first family can
        // sit on the same line; older reports have a full 4-column header instead.
        let hdr = lines.iter().position(|l| l.starts_with("'ID'"));
        let glued = hdr
            .and_then(|h| lines[h].split_once("'Newick'"))
            .map(|x| x.1)
            .filter(|r| !r.trim().is_empty() && !r.trim_start().starts_with('\''));
        let body = glued.into_iter().chain(
            lines
                .iter()
                .skip(hdr.map(|h| h + 1).unwrap_or(lines.len()))
                .copied(),
        );
        for l in body {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 3 || f[0].trim().is_empty() {
                continue;
            }
            let Ok(ft) = parse_newick(f[1]) else {
                skipped += 1;
                continue;
            };
            if ft.nodes.len() != tree.nodes.len() {
                skipped += 1;
                continue;
            }
            let sizes: Vec<f64> = ft.nodes.iter().map(|n| label_size(&n.name)).collect();
            let p: f64 = f[2].trim().parse().unwrap_or(f64::NAN);
            d.families.push(FamilyRow {
                id: f[0].trim().to_string(),
                pvalue: p,
                significant: p < 0.05,
            });
            counts.families.push(f[0].trim().to_string());
            counts.values.push(sizes.clone());
            changes.families.push(f[0].trim().to_string());
            changes.values.push(
                nonroot
                    .iter()
                    .map(|&i| sizes[i] - tree.nodes[i].parent.map(|q| sizes[q]).unwrap_or(0.0))
                    .collect(),
            );
        }
        if !d.families.is_empty() {
            d.loaded.push(if skipped > 0 {
                format!("families ({skipped} skipped)")
            } else {
                "families".into()
            });
            d.counts = Some(counts);
            d.changes = Some(changes);
            d.loaded.push("counts".into());
            d.loaded.push("changes".into());
        }
        let only_sig = !d.families.is_empty() && d.families.iter().all(|f| f.significant);
        d.results_text = format!(
            "CAFE report: {}\nFamilies: {}   (p < 0.05 flagged significant){}\n{}\n{}\n",
            path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            d.families.len(),
            if only_sig {
                "\nNOTE: every family in this report is significant (CAFE5 lists only families below its p-value cut-off); use the per-file CAFE5 output for all families."
            } else {
                ""
            },
            find("Lambda:").unwrap_or(""),
            find("Lambda tree:").unwrap_or("")
        );
        d.loaded.push("model results".into());
        d.tree = Some(tree);
        d.loaded.push("tree".into());
        Ok(d)
    }
}

/// Distinct run prefixes (e.g. `Base`, `Gamma`) found in a results folder.
pub fn list_prefixes(dir: &Path) -> Vec<String> {
    let mut set = BTreeSet::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            for s in SUFFIXES {
                if let Some(p) = n.strip_suffix(s) {
                    if !p.is_empty() {
                        set.insert(p.to_string());
                    }
                    break;
                }
            }
        }
    }
    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cafe_tree() {
        let t = parse_newick("((cat<1>:68.7,horse<3>:68.7)<2>:4.5,cow<5>:73.2)<0>;").unwrap();
        assert_eq!(t.nodes.iter().filter(|n| n.children.is_empty()).count(), 3);
        assert_eq!(t.nodes[t.root].id, "<0>");
        let cat = t.nodes.iter().find(|n| n.name == "cat").unwrap();
        assert_eq!(cat.id, "<1>");
        assert!((cat.length - 68.7).abs() < 1e-9);
        assert_eq!(t.nodes[cat.parent.unwrap()].id, "<2>");
    }

    #[test]
    fn parses_asr_labels_with_family_size() {
        // CAFE5 *_asr.tre nexus: labels carry the ancestral family size after '_'
        let txt =
            "#nexus\nBEGIN TREES;\n TREE fam1 = ((a<1>_3:1,b<2>_5:1)<3>_4:2,c<4>_2:3)<0>_4;\nEND;";
        let t = parse_newick(txt).unwrap();
        assert_eq!(t.nodes.len(), 5);
        assert_eq!(t.nodes[t.root].id, "<0>");
        assert!(t.nodes.iter().any(|n| n.name == "b" && n.id == "<2>"));
    }

    #[test]
    fn rejects_malformed_newick() {
        assert!(parse_newick("(a:1,b:2").is_err());
        assert!(parse_newick("no tree here").is_err());
    }

    #[test]
    fn parses_nexus() {
        let t = parse_newick("#nexus\nBEGIN TREES;\n TREE 1 = [&R] (a:1,b:2);\nEND;").unwrap();
        assert_eq!(t.nodes.len(), 3);
    }

    #[test]
    fn splits_labels() {
        assert_eq!(split_label("cat<0>"), ("cat".into(), "<0>".into()));
        assert_eq!(split_label("<3>_12"), ("".into(), "<3>".into()));
    }

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("cafe_gui_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn loads_run_and_lists_prefixes() {
        let d = tmpdir("load");
        let w = |n: &str, s: &str| std::fs::write(d.join(n), s).unwrap();
        w(
            "Base_clade_results.txt",
            "#Taxon_ID\tIncrease\tDecrease\ncat<1>\t3\t1\n<0>\t0\t2\n",
        );
        w(
            "Base_family_results.txt",
            "#FamilyID\tpvalue\tSignificant at 0.05\nF1\t0.01\ty\nF2\t0.6\tn\n",
        );
        w(
            "Base_count.tab",
            "FamilyID\tcat<1>\thorse<3>\t<0>\nF1\t5\t1\t3\nF2\t2\t2\t2\n",
        );
        w(
            "Base_change.tab",
            "FamilyID\tcat<1>\thorse<3>\nF1\t+2\t-1\nF2\t0\t0\n",
        );
        w(
            "Base_results.txt",
            "Model Base Final Likelihood (-lnL): 10\n",
        );
        w("Gamma_results.txt", "x");
        assert_eq!(
            list_prefixes(&d),
            vec!["Base".to_string(), "Gamma".to_string()]
        );
        let c = CafeData::load(&d, "Base").unwrap();
        assert_eq!(c.clades.len(), 2);
        assert_eq!(c.clades[0].name, "cat");
        assert_eq!(c.families.iter().filter(|f| f.significant).count(), 1);
        assert_eq!(c.changes.as_ref().unwrap().values[0], vec![2.0, -1.0]);
        assert!(CafeData::load(&d, "Nope").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn loads_real_cafe_report() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("example_data/example_result_small.cafe");
        let d = CafeData::load_cafe4(&path).unwrap();
        let t = d.tree.as_ref().unwrap();
        assert_eq!(t.nodes.len(), 37);
        assert_eq!(t.nodes.iter().filter(|n| n.children.is_empty()).count(), 19);
        assert!(t
            .nodes
            .iter()
            .any(|n| n.name == "hsa" && (n.length - 133.0).abs() < 1e-9));
        assert_eq!(d.clades.len(), 36); // every non-root node
        let eda = d.clades.iter().find(|c| c.name == "eda").unwrap();
        assert_eq!((eda.inc, eda.dec), (1590.0, 273.0));
        assert_eq!(d.families.len(), 50);
        let ct = d.counts.as_ref().unwrap();
        assert_eq!(ct.columns.len(), 37);
        let eda_col = ct.columns.iter().position(|c| c == "eda<0>").unwrap();
        assert_eq!(ct.values[0][eda_col], 51.0); // C_595143: eda_51
        let root_col = ct.columns.iter().position(|c| c == "<1>").unwrap();
        assert_eq!(ct.values[0][root_col], 37.0); // inferred size at root
        let ch = d.changes.as_ref().unwrap();
        assert_eq!(ch.columns.len(), 36);
        assert!(d
            .change_for(&t.nodes[t.nodes.iter().position(|n| n.name == "hsa").unwrap()])
            .is_some());
    }

    #[test]
    fn cafe_report_found_by_prefix() {
        let d = tmpdir("cafe4");
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("example_data/example_result_small.cafe"),
            d.join("run1.cafe"),
        )
        .unwrap();
        assert_eq!(list_prefixes(&d), vec!["run1".to_string()]);
        assert!(CafeData::load(&d, "run1").unwrap().tree.is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    fn example_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example_data")
    }

    /// Real CAFE5 v1.1 output (12 primates, Base model, 40-family excerpt).
    #[test]
    fn loads_native_cafe5_output() {
        let d = CafeData::load(&example_dir(), "Base").unwrap();
        assert_eq!(d.loaded.len(), 6);
        let t = d.tree.as_ref().unwrap();
        assert_eq!(t.nodes.len(), 23);
        assert_eq!(d.clades.len(), 22);
        assert_eq!(d.families.len(), 40);
        assert_eq!(d.families.iter().filter(|f| f.significant).count(), 25);
        let ct = d.counts.as_ref().unwrap();
        assert_eq!(ct.columns.len(), 23);
        assert_eq!(ct.columns[0], "Ptrog<1>");
        assert!(d
            .change_for(&t.nodes[t.nodes.iter().position(|n| n.name == "Hsapi").unwrap()])
            .is_some());
    }

    /// `Base_report.cafe` written by CAFE5 has its header and first family on one line.
    #[test]
    fn native_report_matches_per_file_output() {
        let r = CafeData::load_cafe4(&example_dir().join("Base_report.cafe")).unwrap();
        let n = CafeData::load(&example_dir(), "Base").unwrap();
        assert_eq!(r.families.len(), 25); // CAFE5 lists only families below its p-value cut-off
        assert_eq!(r.families[0].id, "13"); // first family is on the header line
        assert!(r.families.iter().all(|f| f.significant));
        assert_eq!(r.clades.len(), n.clades.len());
        for c in &r.clades {
            let m = n.clades.iter().find(|x| x.id == c.id).unwrap();
            assert_eq!((m.inc, m.dec), (c.inc, c.dec), "{}", c.id);
        }
        let (rc, nc) = (r.counts.as_ref().unwrap(), n.counts.as_ref().unwrap());
        for (k, fid) in rc.families.iter().enumerate() {
            let row = nc.families.iter().position(|x| x == fid).unwrap();
            for (j, col) in rc.columns.iter().enumerate() {
                let nj = nc.columns.iter().position(|x| x == col).unwrap();
                assert_eq!(
                    rc.values[k][j], nc.values[row][nj],
                    "family {fid} column {col}"
                );
            }
        }
    }

    #[test]
    fn report_only_folder_is_found() {
        let d = tmpdir("reportonly");
        std::fs::copy(
            example_dir().join("Base_report.cafe"),
            d.join("Base_report.cafe"),
        )
        .unwrap();
        assert_eq!(list_prefixes(&d), vec!["Base".to_string()]);
        assert_eq!(CafeData::load(&d, "Base").unwrap().families.len(), 25);
        let _ = std::fs::remove_dir_all(&d);
    }
}
