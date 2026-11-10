//! Localization panel: the project's CSV string tables as an editable grid,
//! one row per key and one column per locale.
//!
//! The tables shipped in 2026-09 (`bsengine_core::Localization`), but nothing
//! in the editor showed them: a translation was edited in a spreadsheet and
//! seen only after a restart. Unity's String Table window is the model here --
//! keys down the side, a column per locale, an empty cell marked as missing --
//! and Unity, like this panel, shows the change in the running game at once.
//! (Unreal's Localization Dashboard is a gather-and-compile pipeline around
//! the same tables; Godot has no table editor and imports the CSV.)
//!
//! The panel edits the file as written -- header, comment columns (`_notes`),
//! row order, line endings, byte-order mark -- not the merged tables the game
//! reads, so saving a table changes only the cells that were edited.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bsengine_core::localization::{normalize_locale, parse_csv, parse_csv_table, write_csv};
use bsengine_core::{EditorPanel, EditorPanelContext, Localization, ProjectDir};

/// Width of a column in the grid, in points.
const COLUMN_WIDTH: f32 = 160.0;

/// One string table as written: its header row and then one row per key,
/// every row padded to the header's width.
#[derive(Debug, Clone)]
pub struct StringTableDoc {
    /// The table as `project.toml` names it, project-relative.
    pub source: String,
    /// Where it is on disk.
    pub path: PathBuf,
    /// The header (`keys,en,ko,_notes`) and then the rows.
    rows: Vec<Vec<String>>,
    /// What the file ended its lines with, kept so a save does not rewrite
    /// every line of a table someone made on Windows.
    line_ending: &'static str,
    /// Whether the file started with a UTF-8 byte-order mark (Excel writes
    /// one), kept for the same reason.
    bom: bool,
    /// Edited since it was opened or saved.
    pub dirty: bool,
}

impl StringTableDoc {
    /// The table `source` names, read from `path`.
    ///
    /// # Errors
    ///
    /// A file that cannot be read, or that is not a string table the game
    /// could load -- the panel will not edit a table into one the game then
    /// refuses at start.
    pub fn open(source: &str, path: PathBuf) -> Result<Self, String> {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_text(source, path, &text)
    }

    /// [`Self::open`] on text already read.
    ///
    /// # Errors
    ///
    /// As [`Self::open`].
    pub fn from_text(source: &str, path: PathBuf, text: &str) -> Result<Self, String> {
        parse_csv_table(text).map_err(|e| format!("{source}: {e}"))?;
        let bom = text.starts_with('\u{feff}');
        let body = text.strip_prefix('\u{feff}').unwrap_or(text);
        let line_ending = if body.contains("\r\n") { "\r\n" } else { "\n" };
        // Blank rows carry nothing the game reads (`parse_csv_table` skips
        // them), and a row of empty cells in the grid would read as a key
        // with an empty name.
        let mut rows: Vec<Vec<String>> = parse_csv(body)
            .map_err(|e| format!("{source}: {e}"))?
            .into_iter()
            .filter(|r| r.iter().any(|c| !c.is_empty()))
            .collect();
        let width = rows.first().map_or(0, Vec::len);
        for row in &mut rows {
            row.resize(width, String::new());
        }
        Ok(Self {
            source: source.to_string(),
            path,
            rows,
            line_ending,
            bom,
            dirty: false,
        })
    }

    /// The header: the key column's name, then each column's locale or
    /// comment name.
    pub fn header(&self) -> &[String] {
        &self.rows[0]
    }

    /// The number of keys (rows after the header).
    pub fn len(&self) -> usize {
        self.rows.len() - 1
    }

    /// Whether the table has no keys.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The cells of key row `row` (0 is the first key, not the header).
    pub fn row(&self, row: usize) -> &[String] {
        &self.rows[row + 1]
    }

    /// Whether column `column` holds a locale's text -- not the key column,
    /// and not a comment column (`_notes`) or one with no header, which the
    /// game ignores.
    pub fn is_locale_column(&self, column: usize) -> bool {
        let name = self.header()[column].trim();
        column > 0 && !name.is_empty() && !name.starts_with('_')
    }

    /// How many keys column `column` has no text for -- the cells the game
    /// falls back on another locale for.
    pub fn missing(&self, column: usize) -> usize {
        self.rows[1..]
            .iter()
            .filter(|r| !r[0].trim().is_empty() && r[column].is_empty())
            .count()
    }

    /// The cell at key row `row`, column `column`, for editing; marks the
    /// table edited.
    pub fn cell_mut(&mut self, row: usize, column: usize) -> &mut String {
        self.dirty = true;
        &mut self.rows[row + 1][column]
    }

    /// Adds a row for `key`, every translation empty.
    ///
    /// # Errors
    ///
    /// An empty key, or one the table already has -- which the game would
    /// refuse the whole table for.
    pub fn add_key(&mut self, key: &str) -> Result<(), String> {
        let key = key.trim();
        if key.is_empty() {
            return Err("a key needs a name".to_string());
        }
        if self.rows[1..].iter().any(|r| r[0].trim() == key) {
            return Err(format!("the key {key:?} is already in {}", self.source));
        }
        let mut row = vec![String::new(); self.header().len()];
        row[0] = key.to_string();
        self.rows.push(row);
        self.dirty = true;
        Ok(())
    }

    /// Removes key row `row`.
    pub fn remove_row(&mut self, row: usize) {
        self.rows.remove(row + 1);
        self.dirty = true;
    }

    /// Adds an empty column for `locale`, written normalized (`pt_br` ->
    /// `pt-BR`), as the game reads it.
    ///
    /// # Errors
    ///
    /// An empty locale, one starting with `_` (a comment column), or one
    /// the table already has a column for.
    pub fn add_locale(&mut self, locale: &str) -> Result<(), String> {
        // Checked before normalizing, which reads `_` as a separator and
        // would turn `_x` into the locale `x`.
        let comment = locale.trim().starts_with('_');
        let locale = normalize_locale(locale);
        if locale.is_empty() || comment {
            return Err("a locale needs a name such as \"ko\" or \"pt-BR\"".to_string());
        }
        if (1..self.header().len())
            .any(|c| self.is_locale_column(c) && normalize_locale(&self.header()[c]) == locale)
        {
            return Err(format!("{} already has a {locale} column", self.source));
        }
        for (i, row) in self.rows.iter_mut().enumerate() {
            row.push(if i == 0 {
                locale.clone()
            } else {
                String::new()
            });
        }
        self.dirty = true;
        Ok(())
    }

    /// The table as it will be written.
    pub fn text(&self) -> String {
        let csv = write_csv(&self.rows, self.line_ending);
        if self.bom {
            format!("\u{feff}{csv}")
        } else {
            csv
        }
    }

    /// Writes the table back to its file.
    ///
    /// # Errors
    ///
    /// A table the game could not load -- two rows with one key, after a
    /// rename -- which is not written, so the file on disk stays one the game
    /// starts with; or a write that fails.
    pub fn save(&mut self) -> Result<(), String> {
        let text = self.text();
        parse_csv_table(&text).map_err(|e| format!("{}: {e}", self.source))?;
        std::fs::write(&self.path, text).map_err(|e| format!("{}: {e}", self.path.display()))?;
        self.dirty = false;
        Ok(())
    }
}

/// The panel's request for the game's tables to be read again, and the
/// system that answers it. Shared like `MixerShared`: the panel cannot reach
/// the world, so it sets a flag and [`reload_localization_tables`] does the
/// reading.
#[derive(bevy_ecs::prelude::Resource, Clone, Default)]
pub struct LocalizationReload(Arc<AtomicBool>);

impl LocalizationReload {
    /// Asks for the tables to be read again on the next update.
    pub fn request(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn take(&self) -> bool {
        self.0.swap(false, Ordering::AcqRel)
    }
}

/// Re-reads every table the game loaded, from disk, into the live
/// [`Localization`] when the panel saved one -- every table, not just the
/// saved one, because the game's tables are a merge in `project.toml`'s
/// order and a later table overrides an earlier one key by key. The locale
/// and settings stay ([`Localization::reload_tables`]): a game that switched
/// to Korean is still in Korean.
///
/// A table that does not load leaves the game's text as it was, with an
/// error, rather than blanking it; the panel's own check means that is a
/// table edited outside the editor in the meantime.
pub fn reload_localization_tables(
    reload: Option<bevy_ecs::prelude::Res<LocalizationReload>>,
    project_dir: Option<bevy_ecs::prelude::Res<ProjectDir>>,
    localization: Option<bevy_ecs::prelude::ResMut<Localization>>,
) {
    let (Some(reload), Some(mut localization)) = (reload, localization) else {
        return;
    };
    if !reload.take() {
        return;
    }
    let texts: Result<Vec<(String, String)>, String> = localization
        .sources()
        .iter()
        .map(|source| {
            let path = bsengine_core::resolve_project_path(project_dir.as_deref(), source);
            std::fs::read_to_string(&path)
                .map(|text| (source.clone(), text))
                .map_err(|e| format!("{path}: {e}"))
        })
        .collect();
    let result = texts.and_then(|texts| {
        localization.reload_tables(texts.iter().map(|(s, t)| (s.as_str(), t.as_str())))
    });
    match result {
        Ok(()) => tracing::info!(
            "localization: tables reloaded, locales {:?}",
            localization.locales()
        ),
        Err(e) => tracing::error!("localization: tables not reloaded: {e}"),
    }
}

/// Edits the project's string tables.
pub struct LocalizationPanel {
    project_dir: Option<ProjectDir>,
    sources: Vec<String>,
    reload: LocalizationReload,
    /// Which of `sources` is open.
    selected: usize,
    /// The open table; read when the panel is first drawn, and again after
    /// **Reload** or a switch to another table.
    doc: Option<StringTableDoc>,
    /// Why the selected table could not be opened or saved.
    error: Option<String>,
    filter: String,
    new_key: String,
    new_locale: String,
}

impl LocalizationPanel {
    /// A panel over `sources` (project-relative, as `project.toml` names
    /// them), asking `reload` to refresh the game after each save.
    pub fn new(
        project_dir: Option<ProjectDir>,
        sources: Vec<String>,
        reload: LocalizationReload,
    ) -> Self {
        Self {
            project_dir,
            sources,
            reload,
            selected: 0,
            doc: None,
            error: None,
            filter: String::new(),
            new_key: String::new(),
            new_locale: String::new(),
        }
    }

    /// The open table, reading it first if it is not open yet.
    pub fn doc(&mut self) -> Option<&mut StringTableDoc> {
        if self.doc.is_none() && self.error.is_none() {
            if let Some(source) = self.sources.get(self.selected) {
                let path = bsengine_core::resolve_project_path(self.project_dir.as_ref(), source);
                match StringTableDoc::open(source, PathBuf::from(path)) {
                    Ok(doc) => self.doc = Some(doc),
                    Err(e) => self.error = Some(e),
                }
            }
        }
        self.doc.as_mut()
    }

    /// Saves the open table and, when it was written, asks for the game's
    /// tables to be read again. What **Save** does.
    ///
    /// # Errors
    ///
    /// As [`StringTableDoc::save`]; the game is then left as it was.
    pub fn save(&mut self) -> Result<(), String> {
        let Some(doc) = self.doc.as_mut() else {
            return Ok(());
        };
        doc.save()?;
        self.reload.request();
        Ok(())
    }

    /// Opens table `index` of the sources, dropping the one open.
    fn select(&mut self, index: usize) {
        self.selected = index;
        self.doc = None;
        self.error = None;
    }

    /// The key rows whose key or any cell contains the filter, ignoring
    /// case; every row when the filter is empty.
    fn visible_rows(doc: &StringTableDoc, filter: &str) -> Vec<usize> {
        let filter = filter.trim().to_lowercase();
        (0..doc.len())
            .filter(|&r| {
                filter.is_empty()
                    || doc
                        .row(r)
                        .iter()
                        .any(|c| c.to_lowercase().contains(&filter))
            })
            .collect()
    }

    /// The id of a cell's text box. Stable across frames and unique per
    /// table, so focus stays in the cell being typed in while the grid
    /// scrolls, and the tests can put focus there.
    pub fn cell_id(source: &str, row: usize, column: usize) -> egui::Id {
        egui::Id::new(("localization-cell", source, row, column))
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let dirty = self.doc.as_ref().is_some_and(|d| d.dirty);
        ui.horizontal(|ui| {
            // Switching tables would drop the edits, so it waits for a save
            // or a revert -- said on hover rather than lost silently.
            let mut selected = self.selected;
            ui.add_enabled_ui(!dirty, |ui| {
                egui::ComboBox::from_id_salt("localization-table")
                    .selected_text(&self.sources[self.selected])
                    .show_ui(ui, |ui| {
                        for (i, source) in self.sources.iter().enumerate() {
                            ui.selectable_value(&mut selected, i, source);
                        }
                    });
            })
            .response
            .on_disabled_hover_text("Save or reload this table first");
            if selected != self.selected {
                self.select(selected);
            }
            let save = ui.add_enabled(
                dirty,
                egui::Button::new(format!("{} Save", egui_phosphor::regular::FLOPPY_DISK)),
            );
            if save.clicked() {
                if let Err(e) = self.save() {
                    self.error = Some(e);
                } else {
                    self.error = None;
                }
            }
            // Always enabled: it is also how a table edited outside the
            // editor, or one that failed to open, is read again -- by the
            // panel and by the game, which would otherwise keep the text it
            // started with until the next save.
            if ui
                .button(format!(
                    "{} Reload",
                    egui_phosphor::regular::ARROW_CLOCKWISE
                ))
                .on_hover_text("Read the tables from disk again, dropping any edits")
                .clicked()
            {
                self.select(self.selected);
                self.reload.request();
            }
            if dirty {
                ui.label(egui::RichText::new("edited").italics().weak());
            }
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text(format!(
                        "{} Search",
                        egui_phosphor::regular::MAGNIFYING_GLASS
                    ))
                    .desired_width(160.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.new_key)
                    .hint_text("NEW_KEY")
                    .desired_width(120.0),
            );
            if ui
                .button(format!("{} Key", egui_phosphor::regular::PLUS))
                .clicked()
            {
                if let Some(doc) = self.doc.as_mut() {
                    match doc.add_key(&self.new_key) {
                        Ok(()) => {
                            self.new_key.clear();
                            self.error = None;
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.new_locale)
                    .hint_text("locale")
                    .desired_width(60.0),
            );
            if ui
                .button(format!("{} Locale", egui_phosphor::regular::PLUS))
                .clicked()
            {
                if let Some(doc) = self.doc.as_mut() {
                    match doc.add_locale(&self.new_locale) {
                        Ok(()) => {
                            self.new_locale.clear();
                            self.error = None;
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            }
        });
    }

    fn grid(ui: &mut egui::Ui, doc: &mut StringTableDoc, filter: &str) {
        let columns = doc.header().len();
        let rows = Self::visible_rows(doc, filter);
        let missing_fill = ui.visuals().warn_fg_color.gamma_multiply(0.18);
        // The header, outside the scroll area's rows so it stays in view.
        ui.horizontal(|ui| {
            for column in 0..columns {
                let name = &doc.header()[column];
                let text = if column == 0 {
                    egui::RichText::new(if name.is_empty() { "key" } else { name }).strong()
                } else if doc.is_locale_column(column) {
                    let missing = doc.missing(column);
                    if missing > 0 {
                        egui::RichText::new(format!("{name} ({missing} missing)"))
                            .strong()
                            .color(ui.visuals().warn_fg_color)
                    } else {
                        egui::RichText::new(name).strong()
                    }
                } else {
                    // A comment column: the game ignores it.
                    egui::RichText::new(name).italics().weak()
                };
                ui.add_sized([COLUMN_WIDTH, 18.0], egui::Label::new(text).truncate());
            }
        });
        ui.separator();
        let row_height = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
        let mut remove = None;
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, rows.len(), |ui, range| {
                for &row in &rows[range] {
                    ui.horizontal(|ui| {
                        for column in 0..columns {
                            let empty = doc.row(row)[column].is_empty();
                            let locale = doc.is_locale_column(column);
                            let id = Self::cell_id(&doc.source, row, column);
                            // Read without marking the table edited, and
                            // written back only when the text box changed it.
                            let mut text = doc.row(row)[column].clone();
                            let mut edit = egui::TextEdit::singleline(&mut text)
                                .id(id)
                                .desired_width(COLUMN_WIDTH);
                            if locale && empty {
                                edit = edit.background_color(missing_fill);
                            }
                            if ui.add(edit).changed() {
                                *doc.cell_mut(row, column) = text;
                            }
                        }
                        if ui
                            .small_button(egui_phosphor::regular::TRASH)
                            .on_hover_text("Remove this key")
                            .clicked()
                        {
                            remove = Some(row);
                        }
                    });
                }
            });
        if let Some(row) = remove {
            doc.remove_row(row);
        }
    }
}

impl EditorPanel for LocalizationPanel {
    fn id(&self) -> &str {
        "localization"
    }

    fn title(&self) -> String {
        "Localization".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut EditorPanelContext) {
        if self.sources.is_empty() {
            // The ordinary state for a project that is not translated, said
            // plainly with what to do about it, as the Mixer says it.
            ui.label("No string tables.");
            ui.label(
                egui::RichText::new(
                    "List CSV tables in project.toml: [localization] tables = [\"strings.csv\"]",
                )
                .weak()
                .small(),
            );
            return;
        }
        self.doc();
        self.toolbar(ui);
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(doc) = self.doc.as_mut() {
            Self::grid(ui, doc, &self.filter);
        }
    }
}

/// Puts the Localization panel in the editor's panel registry, over the
/// tables the game loaded, and the reload handle beside it.
///
/// A `Startup` system for the reason `register_mixer_panel` is one: the
/// registry is final only once every plugin has built. The tables are known
/// by then too -- `project.toml`'s are inserted before the plugins.
///
/// Does nothing without a registry (no editor). A project with no tables
/// still gets the panel, which says how to add one.
pub fn register_localization_panel(
    mut commands: bevy_ecs::prelude::Commands,
    registry: Option<bevy_ecs::prelude::Res<bsengine_core::EditorPanelRegistry>>,
    localization: Option<bevy_ecs::prelude::Res<Localization>>,
    project_dir: Option<bevy_ecs::prelude::Res<ProjectDir>>,
) {
    let Some(registry) = registry else {
        return;
    };
    let Ok(mut panels) = registry.0.lock() else {
        return;
    };
    if panels.contains_key("localization") {
        return;
    }
    // Inserted only with the panel that holds it: a second handle would be
    // one no panel ever sets.
    let reload = LocalizationReload::default();
    commands.insert_resource(reload.clone());
    let sources = localization.map_or_else(Vec::new, |l| l.sources().to_vec());
    let project_dir = project_dir.map(|p| p.clone());
    panels.insert(
        "localization".to_string(),
        Box::new(LocalizationPanel::new(project_dir, sources, reload)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory for one test's tables.
    fn temp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bsengine-localization-panel-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Written by hand the way a spreadsheet writes it: a byte-order mark,
    /// CRLF, a comment column, a cell quoted for its comma, an empty cell.
    const TABLE: &str = "\u{feff}keys,en,ko,_notes\r\n\
                         GREETING,Hello,안녕하세요,casual\r\n\
                         SCORE,\"Score, {points}\",,\r\n\
                         QUIT,Quit,종료,\r\n";

    fn doc_in(dir: &std::path::Path) -> StringTableDoc {
        let path = dir.join("strings.csv");
        std::fs::write(&path, TABLE).unwrap();
        StringTableDoc::open("strings.csv", path).unwrap()
    }

    /// One text the panel drew, with where.
    struct Drawn {
        text: String,
        rect: egui::Rect,
    }

    fn collect(shape: &egui::Shape, out: &mut Vec<Drawn>) {
        match shape {
            egui::Shape::Text(t) => out.push(Drawn {
                text: t.galley.text().to_string(),
                rect: t.galley.rect.translate(t.pos.to_vec2()),
            }),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }

    /// Runs the panel one frame with `events`, in a window wide enough for
    /// four columns; what it drew.
    fn frame(
        ctx: &egui::Context,
        panel: &mut dyn EditorPanel,
        events: Vec<egui::Event>,
    ) -> Vec<Drawn> {
        let mut insp = bsengine_core::InspectorState::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 500.0),
                )),
                events,
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    let mut panel_ctx = EditorPanelContext {
                        insp: &mut insp,
                        entities_snapshot: &[],
                        cursor_pos: (0.0, 0.0),
                        type_registry: None,
                    };
                    panel.ui(ui, &mut panel_ctx);
                });
            },
        );
        output.textures_delta.clear();
        let mut drawn = Vec::new();
        for clipped in &output.shapes {
            collect(&clipped.shape, &mut drawn);
        }
        drawn
    }

    /// Every text drawn on the second of two frames: egui lays a widget out
    /// against the sizes it measured on the previous one.
    fn drawn_text(panel: &mut dyn EditorPanel) -> Vec<String> {
        let ctx = egui::Context::default();
        frame(&ctx, panel, Vec::new());
        frame(&ctx, panel, Vec::new())
            .into_iter()
            .map(|d| d.text)
            .collect()
    }

    /// Types `text` into the cell at `row`, `column` of the open table, the
    /// way a user does: focus on the cell's text box, then keystrokes.
    fn type_into(
        ctx: &egui::Context,
        panel: &mut dyn EditorPanel,
        source: &str,
        row: usize,
        column: usize,
        text: &str,
    ) {
        frame(ctx, panel, Vec::new());
        let id = LocalizationPanel::cell_id(source, row, column);
        ctx.memory_mut(|m| m.request_focus(id));
        frame(ctx, panel, vec![egui::Event::Text(text.to_string())]);
    }

    /// Clicks the drawn text that contains `label` -- a button's caption.
    fn click(ctx: &egui::Context, panel: &mut dyn EditorPanel, label: &str) {
        let drawn = frame(ctx, panel, Vec::new());
        let pos = drawn
            .iter()
            .find(|d| d.text.contains(label))
            .unwrap_or_else(|| panic!("premise: {label:?} must be drawn"))
            .rect
            .center();
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        frame(
            ctx,
            panel,
            vec![egui::Event::PointerMoved(pos), button(true), button(false)],
        );
    }

    fn panel_over(dir: &std::path::Path, reload: LocalizationReload) -> LocalizationPanel {
        LocalizationPanel::new(
            Some(ProjectDir(dir.display().to_string())),
            vec!["strings.csv".to_string()],
            reload,
        )
    }

    /// The hard side is "nothing else changed": a save must not re-quote,
    /// re-order, drop the comment column, the byte-order mark or the CRLF
    /// of a hand-written table, or its diff would be every line.
    #[test]
    fn a_table_is_written_back_as_it_was_but_for_the_edited_cell() {
        let dir = temp_project("roundtrip");
        let mut doc = doc_in(&dir);
        assert!(!doc.dirty);
        doc.save().unwrap();
        assert_eq!(
            std::fs::read_to_string(&doc.path).unwrap(),
            TABLE,
            "an unedited table is written back byte for byte"
        );

        *doc.cell_mut(1, 2) = "점수, {points}".to_string();
        assert!(doc.dirty);
        doc.save().unwrap();
        assert!(!doc.dirty);
        assert_eq!(
            std::fs::read_to_string(&doc.path).unwrap(),
            TABLE.replace(
                "SCORE,\"Score, {points}\",,",
                "SCORE,\"Score, {points}\",\"점수, {points}\","
            ),
        );
    }

    #[test]
    fn keys_and_locales_are_added_as_the_game_reads_them() {
        let dir = temp_project("add");
        let mut doc = doc_in(&dir);
        assert!(doc.add_key("GREETING").unwrap_err().contains("already"));
        assert!(doc.add_key("  ").is_err());
        assert!(!doc.dirty, "a refused key changes nothing");
        doc.add_key(" NEW_GAME ").unwrap();
        assert_eq!(doc.row(3), ["NEW_GAME", "", "", ""]);

        assert!(doc.add_locale("KO").unwrap_err().contains("already"));
        assert!(
            doc.add_locale("_x").is_err(),
            "a comment column is no locale"
        );
        doc.add_locale("pt_br").unwrap();
        assert_eq!(doc.header(), ["keys", "en", "ko", "_notes", "pt-BR"]);
        assert_eq!(doc.missing(4), 4, "every key is missing in a new locale");
        assert_eq!(doc.missing(2), 2, "SCORE and NEW_GAME have no Korean");
        doc.save().unwrap();

        let tables = parse_csv_table(&std::fs::read_to_string(&doc.path).unwrap()).unwrap();
        assert_eq!(tables["pt-BR"].len(), 4);
        assert_eq!(tables["en"]["NEW_GAME"], "");

        doc.remove_row(0);
        doc.save().unwrap();
        let tables = parse_csv_table(&std::fs::read_to_string(&doc.path).unwrap()).unwrap();
        assert!(!tables["en"].contains_key("GREETING"));
        assert!(tables["en"].contains_key("SCORE"));
    }

    /// A rename onto another key makes a table the game refuses at start.
    /// It is not written: the file stays one the game loads.
    #[test]
    fn a_table_the_game_would_refuse_is_not_written() {
        let dir = temp_project("refuse");
        let mut doc = doc_in(&dir);
        *doc.cell_mut(2, 0) = "GREETING".to_string();
        let err = doc.save().unwrap_err();
        assert!(err.contains("twice"), "{err}");
        assert!(doc.dirty, "still edited: nothing was saved");
        assert_eq!(std::fs::read_to_string(&doc.path).unwrap(), TABLE);
    }

    #[test]
    fn the_grid_shows_every_key_its_text_and_what_is_missing() {
        let dir = temp_project("grid");
        doc_in(&dir);
        let mut panel = panel_over(&dir, LocalizationReload::default());
        let text = drawn_text(&mut panel);
        for expected in [
            "GREETING",
            "안녕하세요",
            "Score, {points}",
            "QUIT",
            "종료",
            "en",
            "ko (1 missing)",
            "_notes",
            "casual",
        ] {
            assert!(
                text.iter().any(|t| t == expected),
                "expected {expected:?} among the drawn text: {text:?}"
            );
        }
        assert!(
            !text
                .iter()
                .any(|t| t.starts_with("en (") || t.starts_with("_notes (")),
            "a complete locale and a comment column count nothing missing: {text:?}"
        );
    }

    #[test]
    fn the_search_shows_only_matching_rows() {
        let dir = temp_project("search");
        doc_in(&dir);
        let mut panel = panel_over(&dir, LocalizationReload::default());
        panel.filter = "종료".to_string();
        let text = drawn_text(&mut panel);
        assert!(
            text.iter().any(|t| t == "QUIT"),
            "matched by its text: {text:?}"
        );
        assert!(!text.iter().any(|t| t == "GREETING"), "{text:?}");
        panel.filter = "greet".to_string();
        let text = drawn_text(&mut panel);
        assert!(
            text.iter().any(|t| t == "GREETING"),
            "matched by its key, ignoring case: {text:?}"
        );
        assert!(!text.iter().any(|t| t == "QUIT"), "{text:?}");
    }

    #[test]
    fn no_tables_says_how_to_add_one() {
        let mut panel = LocalizationPanel::new(None, Vec::new(), LocalizationReload::default());
        let text = drawn_text(&mut panel);
        assert!(
            text.iter().any(|t| t.contains("No string tables")),
            "{text:?}"
        );
        assert!(
            text.iter().any(|t| t.contains("[localization] tables")),
            "{text:?}"
        );
    }

    /// The grid's text boxes are bound to the cells they show: typing into
    /// one changes that cell and no other, and marks the table edited.
    #[test]
    fn typing_in_a_cell_edits_that_cell() {
        let dir = temp_project("typing");
        doc_in(&dir);
        let mut panel = panel_over(&dir, LocalizationReload::default());
        let ctx = egui::Context::default();
        type_into(&ctx, &mut panel, "strings.csv", 2, 2, "끝");
        let doc = panel.doc().unwrap();
        assert_eq!(doc.row(2)[2], "종료끝");
        assert_eq!(
            doc.row(0)[2],
            "안녕하세요",
            "the other cells are as they were"
        );
        assert_eq!(doc.row(2)[1], "Quit");
        assert!(doc.dirty);
    }

    fn app_over(dir: &std::path::Path, table: &str) -> bevy_app::App {
        std::fs::write(dir.join("strings.csv"), table).unwrap();
        let mut localization =
            Localization::from_csv_tables([("strings.csv", table)], "en").unwrap();
        localization.set_locale("ko");
        let mut app = bsengine_app::new_app();
        app.insert_resource(bsengine_core::EditorPanelRegistry::default())
            .insert_resource(ProjectDir(dir.display().to_string()))
            .insert_resource(localization)
            .add_systems(bevy_app::Startup, register_localization_panel)
            .add_systems(bevy_app::Update, reload_localization_tables);
        app.update();
        app
    }

    /// End to end, through the panel the editor registered: type a new
    /// translation, click **Save**, and the game's own `Localization` has it
    /// on the next update -- still in the locale the game had switched to.
    #[test]
    fn a_saved_translation_reaches_the_game_on_the_next_update() {
        let dir = temp_project("live");
        let mut app = app_over(&dir, TABLE);
        let tr = |app: &bevy_app::App| app.world().resource::<Localization>().tr("QUIT");
        assert_eq!(tr(&app), "종료", "premise: the game shows the old text");

        let registry = app
            .world()
            .resource::<bsengine_core::EditorPanelRegistry>()
            .0
            .clone();
        {
            let mut panels = registry.lock().unwrap();
            let panel = panels
                .get_mut("localization")
                .expect("the startup system registers the panel")
                .as_mut();
            let ctx = egui::Context::default();
            type_into(&ctx, panel, "strings.csv", 2, 2, "!");
            app.update();
            assert_eq!(
                tr(&app),
                "종료",
                "an edit is not the game's until it is saved"
            );
            click(&ctx, panel, "Save");
        }
        assert!(
            std::fs::read_to_string(dir.join("strings.csv"))
                .unwrap()
                .contains("QUIT,Quit,종료!,"),
            "premise: Save wrote the file"
        );
        app.update();
        assert_eq!(tr(&app), "종료!");
        assert_eq!(
            app.world().resource::<Localization>().locale(),
            "ko",
            "the game's locale survives the reload"
        );
    }

    /// A table edited outside the editor -- in a spreadsheet, by a merge --
    /// reaches both the grid and the game with **Reload**.
    #[test]
    fn reload_reads_a_table_edited_outside_the_editor() {
        let dir = temp_project("outside");
        let mut app = app_over(&dir, TABLE);
        let registry = app
            .world()
            .resource::<bsengine_core::EditorPanelRegistry>()
            .0
            .clone();
        let mut panels = registry.lock().unwrap();
        let panel = panels.get_mut("localization").unwrap().as_mut();
        let ctx = egui::Context::default();
        let drawn = |ctx: &egui::Context, panel: &mut dyn EditorPanel| {
            frame(ctx, panel, Vec::new());
            frame(ctx, panel, Vec::new())
                .into_iter()
                .map(|d| d.text)
                .collect::<Vec<_>>()
        };
        assert!(
            drawn(&ctx, panel).iter().any(|t| t == "종료"),
            "premise: the grid shows the table as it was"
        );

        std::fs::write(dir.join("strings.csv"), "keys,en,ko\nQUIT,Quit,나가기\n").unwrap();
        assert!(
            drawn(&ctx, panel).iter().any(|t| t == "종료"),
            "premise: nothing is read again unasked"
        );
        click(&ctx, panel, "Reload");
        let text = drawn(&ctx, panel);
        assert!(text.iter().any(|t| t == "나가기"), "{text:?}");
        app.update();
        assert_eq!(app.world().resource::<Localization>().tr("QUIT"), "나가기");
    }

    /// A table broken outside the editor between saves leaves the game's
    /// text as it was rather than blanking every string to its key.
    #[test]
    fn a_reload_that_fails_keeps_the_games_text() {
        let dir = temp_project("broken");
        let mut app = app_over(&dir, TABLE);
        std::fs::write(dir.join("strings.csv"), "keys,en\nQUIT,\"open\n").unwrap();
        app.world().resource::<LocalizationReload>().request();
        app.update();
        assert_eq!(
            app.world().resource::<Localization>().tr("QUIT"),
            "종료",
            "the old tables stay"
        );

        std::fs::write(dir.join("strings.csv"), "keys,en,ko\nQUIT,Quit,나가기\n").unwrap();
        app.update();
        assert_eq!(
            app.world().resource::<Localization>().tr("QUIT"),
            "종료",
            "nothing is read again until a save asks"
        );
        app.world().resource::<LocalizationReload>().request();
        app.update();
        assert_eq!(app.world().resource::<Localization>().tr("QUIT"), "나가기");
    }
}
