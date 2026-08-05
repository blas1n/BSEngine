//! Parses workspace source with `syn` to find components, scripting ops, and
//! reflection registrations.
//!
//! This is deliberately not a text scan. Rust declarations wrap across lines,
//! carry attributes between the derive and the declaration, and nest inside
//! modules; a regex gets all three wrong. An early grep-based attempt at this
//! catalogue found 14 of 49 components and invented a type called `Namepub`.

use serde::Serialize;

/// One field of a component, as written in its declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    /// The field's name.
    pub name: String,
    /// The field's type, rendered back to source form.
    pub ty: String,
}

/// A `#[derive(Component)]` type found in the workspace source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Component {
    /// The type's name, without its module path.
    pub name: String,
    /// The crate it is declared in, e.g. `bsengine-physics`.
    pub krate: String,
    /// Where it is declared, as `<file>:<line>`.
    pub location: String,
    /// Its fields; empty for enums and unit structs.
    pub fields: Vec<Field>,
    /// The first paragraph of its rustdoc.
    pub doc: String,
    /// Whether some `register_type::<..>()` call names it.
    pub registered: bool,
}

/// A `#[op2]` scripting op exposed to JavaScript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Op {
    /// The Rust function name, e.g. `bsengine_get_velocity`.
    pub name: String,
    /// The crate it is declared in.
    pub krate: String,
    /// Where it is declared, as `<file>:<line>`.
    pub location: String,
    /// The first paragraph of its rustdoc.
    pub doc: String,
}

/// Finds every component declared in one source string.
///
/// `krate` and `file` are recorded on the results; this function does no I/O so
/// it can be unit-tested directly.
pub fn components_in_source(src: &str, krate: &str, file: &str) -> Vec<Component> {
    let parsed = match syn::parse_file(src) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    collect_components(&parsed.items, src, krate, file, &mut out);
    out
}

fn collect_components(
    items: &[syn::Item],
    src: &str,
    krate: &str,
    file: &str,
    out: &mut Vec<Component>,
) {
    for item in items {
        match item {
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_components(inner, src, krate, file, out);
                }
            }
            syn::Item::Struct(s) if derives_component(&s.attrs) => {
                out.push(Component {
                    name: s.ident.to_string(),
                    krate: krate.to_string(),
                    location: locate(src, file, &s.ident.to_string()),
                    fields: fields_of(&s.fields),
                    doc: doc_summary(&s.attrs),
                    registered: false,
                });
            }
            syn::Item::Enum(e) if derives_component(&e.attrs) => {
                out.push(Component {
                    name: e.ident.to_string(),
                    krate: krate.to_string(),
                    location: locate(src, file, &e.ident.to_string()),
                    fields: Vec::new(),
                    doc: doc_summary(&e.attrs),
                    registered: false,
                });
            }
            _ => {}
        }
    }
}

/// True when one of the item's `#[derive(..)]` lists names `Component`.
fn derives_component(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("derive") {
            return false;
        }
        let mut found = false;
        // `parse_nested_meta` walks the derive list; the closure returning Ok
        // for everything means an unrecognised entry is skipped, not an error.
        let _ = a.parse_nested_meta(|meta| {
            if meta.path.is_ident("Component") {
                found = true;
            }
            Ok(())
        });
        found
    })
}

fn fields_of(fields: &syn::Fields) -> Vec<Field> {
    match fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|f| Field {
                name: f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default(),
                ty: type_to_string(&f.ty),
            })
            .collect(),
        // Tuple structs like `Name(pub String)` have positional fields; index
        // them so the catalogue still shows their shape.
        syn::Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, f)| Field {
                name: i.to_string(),
                ty: type_to_string(&f.ty),
            })
            .collect(),
        syn::Fields::Unit => Vec::new(),
    }
}

/// Renders a type back to a compact source-like string, e.g. `Vec<Vertex>`.
fn type_to_string(ty: &syn::Type) -> String {
    use quote::ToTokens;
    ty.to_token_stream().to_string().replace(' ', "")
}

/// The first paragraph of an item's rustdoc, with the leading space `///`
/// leaves on each line removed.
fn doc_summary(attrs: &[syn::Attribute]) -> String {
    let mut lines = Vec::new();
    for a in attrs {
        if !a.path().is_ident("doc") {
            continue;
        }
        if let syn::Meta::NameValue(nv) = &a.meta {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
            {
                lines.push(s.value().trim().to_string());
            }
        }
    }
    // Stop at the first blank line: the summary is the first paragraph.
    let mut summary = Vec::new();
    for l in lines {
        if l.is_empty() {
            break;
        }
        summary.push(l);
    }
    summary.join(" ")
}

/// Finds the 1-based line a declaration sits on by searching the source text.
///
/// `syn` identifies the item; this only locates it for display. Getting spans
/// from `syn` needs `proc-macro2`'s `span-locations` feature, which is a
/// heavier dependency than a substring search deserves for a line number.
/// The match must end at a word boundary. A plain `contains` would resolve
/// `RigidBody` to the line declaring `RigidBodyType`, which in
/// `bsengine-physics` sits eleven lines earlier — sending anyone who follows
/// the location to the wrong declaration.
fn locate(src: &str, file: &str, ident: &str) -> String {
    let needles = [format!("struct {ident}"), format!("enum {ident}")];
    for (i, line) in src.lines().enumerate() {
        for n in &needles {
            let Some(at) = line.find(n.as_str()) else {
                continue;
            };
            let after = line[at + n.len()..].chars().next();
            if after.is_none_or(|c| !c.is_alphanumeric() && c != '_') {
                return format!("{file}:{}", i + 1);
            }
        }
    }
    file.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_component_is_found_with_its_fields_and_doc() {
        let src = r#"
            /// Damping and body type for a simulated body.
            ///
            /// Second paragraph is not part of the summary.
            #[derive(Component, Debug, Clone)]
            pub struct RigidBody {
                /// Whether the body is dynamic, static, or kinematic.
                pub body_type: RigidBodyType,
                pub linear_damping: f32,
            }
        "#;
        let found = components_in_source(src, "bsengine-physics", "components.rs");
        assert_eq!(found.len(), 1);
        let c = &found[0];
        assert_eq!(c.name, "RigidBody");
        assert_eq!(c.krate, "bsengine-physics");
        assert_eq!(c.doc, "Damping and body type for a simulated body.");
        assert_eq!(
            c.fields,
            vec![
                Field {
                    name: "body_type".into(),
                    ty: "RigidBodyType".into()
                },
                Field {
                    name: "linear_damping".into(),
                    ty: "f32".into()
                },
            ]
        );
    }

    #[test]
    fn a_type_without_component_in_its_derive_is_not_a_component() {
        let src = r#"
            /// Not a component.
            #[derive(Debug, Clone, Resource)]
            pub struct PendingSceneLoad { pub path: String }
        "#;
        assert!(components_in_source(src, "bsengine-scene", "types.rs").is_empty());
    }

    #[test]
    fn a_multi_line_derive_with_trailing_attributes_still_parses() {
        // Real components look like this -- the derive wraps and #[reflect(..)]
        // or #[serde(..)] sits between the derive and the declaration. A regex
        // scan gets this wrong, which is why this crate uses syn.
        let src = r#"
            /// A component declared the way the codebase actually declares them.
            #[derive(
                Component,
                Debug,
                Default,
            )]
            #[reflect(Component)]
            pub struct Grounded {
                pub is_grounded: bool,
            }
        "#;
        let found = components_in_source(src, "bsengine-physics", "components.rs");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Grounded");
    }

    #[test]
    fn an_enum_component_is_found_and_has_no_fields() {
        let src = r#"
            /// Whether a sound is playing, paused, or stopped.
            #[derive(Component, Debug, Default)]
            pub enum PlaybackState { #[default] Playing, Paused, Stopped }
        "#;
        let found = components_in_source(src, "bsengine-audio", "components.rs");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "PlaybackState");
        assert!(found[0].fields.is_empty());
    }

    #[test]
    fn a_location_points_at_the_type_itself_not_a_longer_name_above_it() {
        // bsengine-physics declares `RigidBodyType` eleven lines before
        // `RigidBody`. A prefix match resolves `RigidBody` to the enum's line
        // and sends anyone following the location to the wrong declaration.
        let src = r#"
            /// The kind of body.
            #[derive(Debug)]
            pub enum RigidBodyType { Dynamic, Static }

            /// A simulated body.
            #[derive(Component)]
            pub struct RigidBody { pub linear_damping: f32 }
        "#;
        let found = components_in_source(src, "bsengine-physics", "components.rs");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].location, "components.rs:8",
            "should point at `pub struct RigidBody`, not at `pub enum RigidBodyType`"
        );
    }

    #[test]
    fn a_component_declared_inside_a_module_is_found() {
        // Nested modules are common; a top-level-items-only walk misses them.
        let src = r#"
            mod inner {
                /// Nested component.
                #[derive(Component)]
                pub struct Nested { pub v: u32 }
            }
        "#;
        let found = components_in_source(src, "bsengine-core", "lib.rs");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Nested");
    }
}
