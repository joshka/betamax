//! Font database setup and `FontFamily` resolution shared by every text rasterizer.
//!
//! With the default `bundled-font` feature, Betamax embeds JetBrains Mono (the default
//! `FontFamily`) and registers it in place of any installed copy, so the default family renders
//! the same on every machine. Other families still come from installed fonts, and glyphs the
//! chosen font lacks, such as emoji and CJK, still fall back to installed fonts.

use cosmic_text::fontdb::{self, Query};
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};

/// Family name of the default font, which Betamax bundles when `bundled-font` is enabled.
pub(crate) const DEFAULT_FAMILY: &str = "JetBrains Mono";

/// Create a font system from installed fonts plus the bundled default font.
///
/// Installed faces named [`DEFAULT_FAMILY`] are dropped in favor of the bundled copy, so a local
/// install of a different JetBrains Mono version cannot change output. This scans system fonts,
/// so callers should create one per renderer and reuse it.
pub(crate) fn font_system() -> FontSystem {
    let font_system = FontSystem::new();
    if !cfg!(feature = "bundled-font") {
        return font_system;
    }
    let (locale, mut db) = font_system.into_locale_and_db();
    let installed: Vec<_> = db
        .faces()
        .filter(|face| face.families.iter().any(|(name, _)| name == DEFAULT_FAMILY))
        .map(|face| face.id)
        .collect();
    for id in installed {
        db.remove_face(id);
    }
    load_bundled(&mut db);
    db.set_monospace_family(DEFAULT_FAMILY);
    FontSystem::new_with_locale_and_db(locale, db)
}

#[cfg(feature = "bundled-font")]
fn load_bundled(db: &mut fontdb::Database) {
    for data in betamax_fonts::ALL {
        db.load_font_source(fontdb::Source::Binary(std::sync::Arc::new(data)));
    }
}

#[cfg(not(feature = "bundled-font"))]
fn load_bundled(_db: &mut fontdb::Database) {}

/// Family to shape text with for a requested `FontFamily`.
///
/// A requested family that no loaded face provides would otherwise let `cosmic-text` pick an
/// arbitrary, possibly proportional, face. Unresolved and absent requests use the generic
/// monospace family instead, which is the bundled default font when `bundled-font` is enabled.
pub(crate) fn resolve_family<'a>(
    font_system: &FontSystem,
    requested: Option<&'a str>,
) -> Family<'a> {
    match requested {
        Some(name) if has_family(font_system, name) => Family::Name(name),
        _ => Family::Monospace,
    }
}

/// Warn once per renderer when the requested `FontFamily` does not resolve to a loaded face.
///
/// The warning names the family used instead, so a missing font in CI is visible in the log
/// rather than only in the rendered pixels.
pub(crate) fn warn_if_unresolved(font_system: &mut FontSystem, requested: Option<&str>) {
    let Some(name) = requested else {
        return;
    };
    if has_family(font_system, name) {
        return;
    }
    // Report the face cosmic-text actually shapes ASCII with, which may not be monospace.
    let mut buffer = Buffer::new(font_system, Metrics::new(16.0, 16.0));
    let attrs = Attrs::new().family(Family::Monospace);
    let mut shaped = buffer.borrow_with(font_system);
    shaped.set_text("M", &attrs, Shaping::Advanced, None);
    shaped.shape_until_scroll(false);
    let font_id = buffer
        .layout_runs()
        .find_map(|run| run.glyphs.first().map(|glyph| glyph.font_id));
    let fallback = font_id
        .and_then(|id| font_system.db().face(id))
        .and_then(|face| face.families.first())
        .map(|(family, _)| family.clone());
    match fallback {
        Some(fallback) => {
            tracing::warn!("FontFamily {name:?} is not installed; using {fallback:?}");
        }
        None => tracing::warn!("FontFamily {name:?} is not installed and no fonts were found"),
    }
}

/// Whether any loaded face provides `name`, matching `cosmic-text`'s own family lookup.
fn has_family(font_system: &FontSystem, name: &str) -> bool {
    font_system
        .db()
        .query(&Query {
            families: &[Family::Name(name)],
            ..Query::default()
        })
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_family_uses_monospace() {
        let font_system = font_system();
        assert_eq!(
            resolve_family(&font_system, Some("Betamax Missing Font")),
            Family::Monospace
        );
        assert_eq!(resolve_family(&font_system, None), Family::Monospace);
    }

    #[cfg(feature = "bundled-font")]
    #[test]
    fn bundled_family_matches_default() {
        assert_eq!(betamax_fonts::JETBRAINS_MONO_FAMILY, DEFAULT_FAMILY);
    }

    #[cfg(feature = "bundled-font")]
    #[test]
    fn bundled_default_family_resolves_without_installed_copy() {
        let font_system = font_system();
        let db = font_system.db();
        let ids: Vec<_> = db
            .faces()
            .filter(|face| face.families.iter().any(|(name, _)| name == DEFAULT_FAMILY))
            .map(|face| face.id)
            .collect();
        assert_eq!(ids.len(), betamax_fonts::ALL.len());
        for id in ids {
            assert!(matches!(
                db.face_source(id),
                Some((fontdb::Source::Binary(_), _))
            ));
        }
        assert_eq!(
            resolve_family(&font_system, Some(DEFAULT_FAMILY)),
            Family::Name(DEFAULT_FAMILY)
        );
        assert_eq!(db.family_name(&Family::Monospace), DEFAULT_FAMILY);
    }
}
