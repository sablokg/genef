use crate::figures::{self, shade_rgb};
use crate::parser::{list_prefixes, read_tree, CafeData};
use crate::svg;
use anyhow::Result;
use eframe::egui::{self, Color32};
use egui_plot::{Bar, BarChart, Legend, Plot};
use std::path::{Path, PathBuf};

const RED: Color32 = Color32::from_rgb(215, 48, 39);
const BLUE: Color32 = Color32::from_rgb(69, 117, 180);
const GREY: Color32 = Color32::from_rgb(140, 140, 140);

/*
Gaurav Sablok
gsablok@proton.me
*/

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Summary,
    Tree,
    Branches,
    PValues,
    Sizes,
    Heatmap,
    Changes,
}

pub struct App {
    data: CafeData,
    dir: Option<PathBuf>,
    prefixes: Vec<String>,
    prefix: String,
    tab: Tab,
    status: String,
    phylo: bool,
    size_col: usize,
    change_col: usize,
    top_n: usize,
    bins: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            data: CafeData::default(),
            dir: None,
            prefixes: vec![],
            prefix: String::new(),
            tab: Tab::Summary,
            status: "Open a CAFE5 results folder to begin.".into(),
            phylo: true,
            size_col: 0,
            change_col: 0,
            top_n: 60,
            bins: 20,
        }
    }
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    fn open_dir(&mut self, dir: PathBuf) {
        self.prefixes = list_prefixes(&dir);
        self.prefix = self
            .prefixes
            .iter()
            .find(|p| p.to_lowercase().contains("gamma"))
            .or(self.prefixes.first())
            .cloned()
            .unwrap_or_default();
        self.dir = Some(dir);
        self.reload();
    }

    /// Open a single `.cafe` report; sibling runs in the same folder stay selectable.
    fn open_file(&mut self, path: PathBuf) {
        let Some(dir) = path.parent().map(|d| d.to_path_buf()) else {
            return;
        };
        self.prefixes = list_prefixes(&dir);
        // `Base_report.cafe` -> "Base", `run.cafe` -> "run"
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        self.prefix = ["_report.cafe", ".cafe"]
            .iter()
            .find_map(|s| name.strip_suffix(s))
            .unwrap_or(&name)
            .to_string();
        self.dir = Some(dir);
        self.reload();
    }

    fn reload(&mut self) {
        let Some(dir) = self.dir.clone() else { return };
        match CafeData::load(&dir, &self.prefix) {
            Ok(d) => {
                self.status = format!("Loaded {}: {}", self.prefix, d.loaded.join(", "));
                self.data = d;
                self.size_col = 0;
                self.change_col = 0;
            }
            Err(e) => self.status = format!("Error: {e:#}"),
        }
    }

    fn export_all(&self, dir: &Path) -> Result<usize> {
        svg::export_all(
            &self.data,
            dir,
            &svg::ExportOptions {
                phylo: self.phylo,
                bins: self.bins,
                size_col: self.size_col,
                change_col: self.change_col,
                top_n: self.top_n,
            },
        )
    }
}

// ------------------------------------------------------------------ helpers

fn col_combo(ui: &mut egui::Ui, label: &str, cols: &[String], sel: &mut usize) {
    if cols.is_empty() {
        return;
    }
    *sel = (*sel).min(cols.len() - 1);
    egui::ComboBox::from_label(label)
        .selected_text(cols[*sel].clone())
        .show_ui(ui, |ui| {
            for (i, c) in cols.iter().enumerate() {
                ui.selectable_value(sel, i, c.clone());
            }
        });
}

/// Grouped bar chart with category labels on the x axis.
fn bar_plot(
    ui: &mut egui::Ui,
    id: &str,
    labels: &[String],
    series: &[(&str, Color32, &[f64])],
    xl: &str,
    yl: &str,
) {
    let k = series.len().max(1) as f64;
    let w = 0.8 / k;
    let labels = labels.to_vec();
    Plot::new(id.to_string())
        .legend(Legend::default())
        .x_axis_label(xl.to_string())
        .y_axis_label(yl.to_string())
        .height(ui.available_height().max(250.0))
        .x_axis_formatter(move |m, _| {
            let v = m.value;
            let i = v.round();
            if (v - i).abs() < 1e-6 && i >= 0.0 && (i as usize) < labels.len() {
                labels[i as usize].clone()
            } else {
                String::new()
            }
        })
        .show(ui, |pu| {
            for (si, (name, color, vals)) in series.iter().enumerate() {
                let off = (si as f64 - (k - 1.0) / 2.0) * w;
                let bars: Vec<Bar> = vals
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Bar::new(i as f64 + off, *v).width(w * 0.95))
                    .collect();
                pu.bar_chart(BarChart::new(bars).name(*name).color(*color));
            }
        });
}

// ------------------------------------------------------------------ app UI

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Open results folder…").clicked() {
                    if let Some(d) = rfd::FileDialog::new().pick_folder() {
                        self.open_dir(d);
                    }
                }
                if ui.button("Open .cafe file…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("CAFE report", &["cafe"])
                        .pick_file()
                    {
                        self.open_file(p);
                    }
                }
                if !self.prefixes.is_empty() {
                    let prefixes = self.prefixes.clone();
                    let before = self.prefix.clone();
                    egui::ComboBox::from_label("Run")
                        .selected_text(self.prefix.clone())
                        .show_ui(ui, |ui| {
                            for p in &prefixes {
                                ui.selectable_value(&mut self.prefix, p.clone(), p.clone());
                            }
                        });
                    if before != self.prefix {
                        self.reload();
                    }
                }
                if ui.button("Load tree…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter(
                            "Newick / Nexus",
                            &["tre", "tree", "nwk", "newick", "nex", "txt"],
                        )
                        .pick_file()
                    {
                        match read_tree(&p) {
                            Ok(t) => {
                                self.data.tree = Some(t);
                                self.status = format!("Tree loaded from {}", p.display());
                            }
                            Err(e) => self.status = format!("Tree error: {e:#}"),
                        }
                    }
                }
                if ui.button("Export all figures (SVG)…").clicked() {
                    if let Some(d) = rfd::FileDialog::new().pick_folder() {
                        self.status = match self.export_all(&d) {
                            Ok(n) => format!("Wrote {n} SVG figures to {}", d.display()),
                            Err(e) => format!("Export error: {e:#}"),
                        };
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                for (t, l) in [
                    (Tab::Summary, "Summary"),
                    (Tab::Tree, "Tree"),
                    (Tab::Branches, "Branch changes"),
                    (Tab::PValues, "P-values"),
                    (Tab::Sizes, "Family sizes"),
                    (Tab::Heatmap, "Heat map"),
                    (Tab::Changes, "Size change"),
                ] {
                    ui.selectable_value(&mut self.tab, t, l);
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.status);
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Summary => self.summary_ui(ui),
            Tab::Tree => self.tree_ui(ui),
            Tab::Branches => self.branches_ui(ui),
            Tab::PValues => self.pvalues_ui(ui),
            Tab::Sizes => self.sizes_ui(ui),
            Tab::Heatmap => self.heatmap_ui(ui),
            Tab::Changes => self.changes_ui(ui),
        });
    }
}

impl App {
    fn summary_ui(&self, ui: &mut egui::Ui) {
        let d = &self.data;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Run summary");
            let nsig = d.families.iter().filter(|f| f.significant).count();
            ui.label(format!(
                "Families: {}   significant: {}",
                d.families.len(),
                nsig
            ));
            let (inc, dec) = d
                .clades
                .iter()
                .fold((0.0, 0.0), |a, c| (a.0 + c.inc, a.1 + c.dec));
            ui.label(format!(
                "Total expansions: {inc}   total contractions: {dec}"
            ));
            ui.separator();
            ui.strong("Most significant families");
            let mut fams: Vec<_> = d.families.iter().filter(|f| f.pvalue.is_finite()).collect();
            fams.sort_by(|a, b| {
                a.pvalue
                    .partial_cmp(&b.pvalue)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            egui::Grid::new("topfam").striped(true).show(ui, |ui| {
                ui.strong("Family");
                ui.strong("p-value");
                ui.strong("Significant");
                ui.end_row();
                for f in fams.iter().take(20) {
                    ui.label(&f.id);
                    ui.label(format!("{:.3e}", f.pvalue));
                    ui.label(if f.significant { "yes" } else { "no" });
                    ui.end_row();
                }
            });
            ui.separator();
            ui.strong("Model results");
            ui.add(egui::Label::new(
                egui::RichText::new(&d.results_text).monospace(),
            ));
        });
    }

    fn tree_ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(
            &mut self.phylo,
            "Scale by branch length (uncheck for cladogram)",
        );
        let Some(tree) = &self.data.tree else {
            ui.label("No tree loaded. Use a *_asr.tre file (auto-loaded) or 'Load tree…'.");
            return;
        };
        let lay = figures::layout(tree, self.phylo);
        let size = egui::vec2(
            ui.available_width(),
            (lay.leaves as f32 * 24.0 + 60.0).max(300.0),
        );
        egui::ScrollArea::vertical().show(ui, |ui| {
            let (resp, painter) = ui.allocate_painter(size, egui::Sense::hover());
            let r = resp.rect;
            let (left, right) = (r.left() + 20.0, r.right() - 170.0);
            let (top, bottom) = (r.top() + 20.0, r.bottom() - 30.0);
            let denom = (lay.leaves.max(2) - 1) as f64;
            let px = |x: f64| left + (right - left).max(50.0) * x as f32;
            let py = |y: f64| top + (bottom - top) * (y / denom) as f32;
            let ink = ui.visuals().text_color();
            let stroke = egui::Stroke::new(1.5, ink);
            let small = egui::FontId::proportional(11.0);
            for (i, nd) in tree.nodes.iter().enumerate() {
                let pos = egui::pos2(px(lay.x[i]), py(lay.y[i]));
                if let Some(p) = nd.parent {
                    let xp = px(lay.x[p]);
                    painter.line_segment([egui::pos2(xp, pos.y), pos], stroke);
                    if let Some((inc, dec)) = self.data.change_for(nd) {
                        let mx = (xp + pos.x) / 2.0;
                        painter.text(
                            egui::pos2(mx, pos.y - 1.0),
                            egui::Align2::CENTER_BOTTOM,
                            format!("+{}", inc as i64),
                            small.clone(),
                            RED,
                        );
                        painter.text(
                            egui::pos2(mx, pos.y + 1.0),
                            egui::Align2::CENTER_TOP,
                            format!("-{}", dec as i64),
                            small.clone(),
                            BLUE,
                        );
                    }
                }
                if let (Some(&a), Some(&b)) = (nd.children.first(), nd.children.last()) {
                    painter.line_segment(
                        [
                            egui::pos2(pos.x, py(lay.y[a])),
                            egui::pos2(pos.x, py(lay.y[b])),
                        ],
                        stroke,
                    );
                } else {
                    painter.text(
                        pos + egui::vec2(6.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &nd.name,
                        egui::FontId::proportional(13.0),
                        ink,
                    );
                }
            }
            painter.text(
                egui::pos2(left, r.bottom() - 8.0),
                egui::Align2::LEFT_CENTER,
                "+ expansions",
                small.clone(),
                RED,
            );
            painter.text(
                egui::pos2(left + 100.0, r.bottom() - 8.0),
                egui::Align2::LEFT_CENTER,
                "- contractions",
                small,
                BLUE,
            );
        });
    }

    fn branches_ui(&self, ui: &mut egui::Ui) {
        if self.data.clades.is_empty() {
            ui.label("No *_clade_results.txt loaded.");
            return;
        }
        let (l, inc, dec) = figures::branch_bars(&self.data);
        bar_plot(
            ui,
            "branches",
            &l,
            &[
                ("Expansions", RED, &inc[..]),
                ("Contractions", BLUE, &dec[..]),
            ],
            "Branch",
            "Families",
        );
    }

    fn pvalues_ui(&mut self, ui: &mut egui::Ui) {
        if self.data.families.is_empty() {
            ui.label("No *_family_results.txt loaded.");
            return;
        }
        ui.add(egui::Slider::new(&mut self.bins, 5..=100).text("bins"));
        let (l, all, sig) = figures::pvalue_hist(&self.data, self.bins);
        bar_plot(
            ui,
            "pvals",
            &l,
            &[
                ("all families", GREY, &all[..]),
                ("significant", RED, &sig[..]),
            ],
            "p-value",
            "Families",
        );
    }

    fn sizes_ui(&mut self, ui: &mut egui::Ui) {
        let Some(ct) = &self.data.counts else {
            ui.label("No *_count.tab loaded.");
            return;
        };
        col_combo(ui, "Species / node", &ct.columns, &mut self.size_col);
        let (l, c) = figures::size_dist(ct, self.size_col, 50);
        bar_plot(
            ui,
            "sizes",
            &l,
            &[("families", BLUE, &c[..])],
            "Gene copies per family",
            "Families",
        );
    }

    fn changes_ui(&mut self, ui: &mut egui::Ui) {
        let Some(ct) = &self.data.changes else {
            ui.label("No *_change.tab loaded.");
            return;
        };
        col_combo(ui, "Branch / node", &ct.columns, &mut self.change_col);
        let (l, c) = figures::change_hist(ct, self.change_col, 20);
        bar_plot(
            ui,
            "changes",
            &l,
            &[("families", RED, &c[..])],
            "Size change (gains − losses)",
            "Families",
        );
    }

    fn heatmap_ui(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Slider::new(&mut self.top_n, 10..=300).text("top variable families"));
        let Some(ct) = &self.data.counts else {
            ui.label("No *_count.tab loaded.");
            return;
        };
        let hm = figures::heatmap(ct, self.top_n);
        if hm.cols.is_empty() {
            return;
        }
        let maxv = hm.m.iter().flatten().cloned().fold(1.0f64, f64::max);
        let left = 140.0;
        let cw = ((ui.available_width() - left - 20.0) / hm.cols.len() as f32).clamp(3.0, 22.0);
        let ch = 20.0;
        let size = egui::vec2(
            left + cw * hm.cols.len() as f32 + 10.0,
            ch * hm.rows.len() as f32 + 10.0,
        );
        egui::ScrollArea::both().show(ui, |ui| {
            let (resp, painter) = ui.allocate_painter(size, egui::Sense::hover());
            let o = resp.rect.min;
            let ink = ui.visuals().text_color();
            for (r, row) in hm.m.iter().enumerate() {
                let y = o.y + r as f32 * ch;
                painter.text(
                    egui::pos2(o.x + left - 6.0, y + ch / 2.0),
                    egui::Align2::RIGHT_CENTER,
                    &hm.rows[r],
                    egui::FontId::proportional(12.0),
                    ink,
                );
                for (c, v) in row.iter().enumerate() {
                    let (rr, gg, bb) = shade_rgb(v / maxv);
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(o.x + left + c as f32 * cw, y),
                        egui::vec2(cw, ch),
                    );
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(rr, gg, bb));
                }
            }
            if let Some(p) = resp.hover_pos() {
                let (cx, cy) = ((p.x - o.x - left) / cw, (p.y - o.y) / ch);
                if cx >= 0.0
                    && cy >= 0.0
                    && (cx as usize) < hm.cols.len()
                    && (cy as usize) < hm.rows.len()
                {
                    let (c, r) = (cx as usize, cy as usize);
                    let text = format!("{} in {}: {}", hm.cols[c], hm.rows[r], hm.m[r][c]);
                    resp.on_hover_ui_at_pointer(|ui| {
                        ui.label(text);
                    });
                }
            }
        });
    }
}
