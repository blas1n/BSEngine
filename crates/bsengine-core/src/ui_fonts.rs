use bevy_ecs::prelude::Resource;

/// The project's UI fonts, from `project.toml`'s `[ui] fonts`, read at start.
///
/// The renderer puts them ahead of egui's built-in fonts, in order: the
/// first is the font text is drawn in, and a character it lacks is looked
/// for in the next, then the next, and last in the built-in ones -- Godot's
/// custom font with its fallbacks, TextMeshPro's font asset with its
/// fallback list. Without one, text in a script egui does not ship (Hangul,
/// kana, Han) draws as a row of identical boxes, so a translated table is no
/// use for those languages.
///
/// Absent in the editor, which keeps its own fonts.
#[derive(Resource, Default, Clone)]
pub struct UiFonts(pub Vec<UiFont>);

/// One font file, by the path the manifest names it with.
#[derive(Clone)]
pub struct UiFont {
    /// As written in `[ui] fonts`; names the font in logs.
    pub path: String,
    /// The file's contents: a `.ttf` or `.otf`.
    pub bytes: Vec<u8>,
}
