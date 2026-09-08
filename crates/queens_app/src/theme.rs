//! Colours, type and the shared UI vocabulary every screen is built from.
//!
//! The design is flat by rule: every colour is a solid fill that means
//! something (a state, a selection, a region), never a gradient or a shadow
//! standing in for depth that is not there. Type carries the hierarchy that
//! elsewhere would come from ornament — a screen's name is the biggest, boldest
//! thing on it.

use bevy::prelude::*;
use bevy::text::{FontFeatureTag, FontFeatures, LetterSpacing};
use queens_core::RegionNames;

// --- palette ---------------------------------------------------------------

/// Builds a colour from a `0xRRGGBB` literal, which is far easier to read and
/// tweak than three floats.
const fn rgb(hex: u32) -> Color {
    Color::srgb(
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    )
}

pub const BACKGROUND: Color = rgb(0x121317);
pub const PANEL: Color = rgb(0x1B1E27);
pub const TEXT: Color = rgb(0xF3F4F8);
pub const TEXT_DIM: Color = rgb(0x8B92A6);
/// Flat, saturated blue: the one colour that means "select this" or "do this".
pub const ACCENT: Color = rgb(0x3D7BFF);
/// Flat, saturated green: a state is correct, or best.
pub const SUCCESS: Color = rgb(0x2ECC71);
/// Flat, saturated red: a state is wrong.
pub const DANGER: Color = rgb(0xEF4444);

pub const BUTTON: Color = rgb(0x242833);
pub const BUTTON_HOVER: Color = rgb(0x2F3543);
pub const BUTTON_PRESSED: Color = rgb(0x3B4356);
/// The fill a typed field takes while it holds the keystrokes, in place of a
/// focus ring: a flat, accent-tinted block rather than a stroke around it.
pub const FIELD_FOCUS: Color = rgb(0x18233D);
/// A control that cannot be used right now. Translucent rather than a solid
/// tone, so it reads as muted against whatever it sits on — a panel or the
/// bare screen behind it — instead of only working on one of the two.
pub const DISABLED: Color = Color::srgba(0.141, 0.157, 0.2, 0.55);

/// Bold separator between two regions.
pub const REGION_EDGE: Color = rgb(0x0E0F13);
/// Faint separator between two cells of the same region.
pub const CELL_EDGE: Color = Color::srgba(0.08, 0.09, 0.11, 0.28);
/// The queen token, which sits on top of whatever colour its region is: a dark
/// disc, so the crown inside it reads the same against every region colour.
pub const QUEEN_BODY: Color = rgb(0x0E0F13);
pub const QUEEN_RING: Color = rgb(0xF6F7FB);
/// The crown drawn inside the disc, and the gem set into its band.
pub const QUEEN_CROWN: Color = rgb(0xFBFCFE);
pub const QUEEN_GEM: Color = rgb(0xFFC233);
/// The player's "no queen here" cross. Opaque, and deliberately softer than
/// the queen's near-black: the two strokes overlap, and a translucent colour
/// would blend with itself and leave a dark patch at the crossing.
pub const CROSS: Color = rgb(0x333A4B);

/// Region colours: bold and saturated rather than pastel, but light enough
/// that the dark queen token and cross stay legible on every one of them.
const REGIONS: [Color; 12] = [
    rgb(0xFF7A63),
    rgb(0xFFC233),
    rgb(0xA9E065),
    rgb(0x36C9AE),
    rgb(0x4FACF7),
    rgb(0x9C8BEF),
    rgb(0xFF7FC0),
    rgb(0xE0B067),
    rgb(0x8FCB6E),
    rgb(0xE38EE8),
    rgb(0x7C93E8),
    rgb(0xDCCB4A),
];

/// An alternative built around the Okabe-Ito palette, which stays separable for
/// the common forms of colour blindness.
const REGIONS_COLOURBLIND: [Color; 12] = [
    rgb(0xE69F00),
    rgb(0x56B4E9),
    rgb(0x009E73),
    rgb(0xF0E442),
    rgb(0x0072B2),
    rgb(0xD55E00),
    rgb(0xCC79A7),
    rgb(0xBFBFBF),
    rgb(0xA6DDF0),
    rgb(0x7FD1AE),
    rgb(0xF5C77E),
    rgb(0x7A7A7A),
];

/// What to call each colour in [`REGIONS`], so a hint can say "the coral
/// region" instead of a number the board never shows. Index for index with the
/// palette; keep the two in step.
const REGION_NAMES: [&str; 12] = [
    "coral",
    "amber",
    "lime",
    "teal",
    "sky",
    "lilac",
    "pink",
    "sand",
    "sage",
    "orchid",
    "periwinkle",
    "khaki",
];

/// The same for [`REGIONS_COLOURBLIND`], whose colours are different enough
/// that the names have to be too.
const REGION_NAMES_COLOURBLIND: [&str; 12] = [
    "orange",
    "sky",
    "green",
    "yellow",
    "blue",
    "red",
    "purple",
    "silver",
    "pale blue",
    "mint",
    "peach",
    "grey",
];

/// The colour of a region. Wraps if a board ever exceeds the palette, though
/// `MAX_SIZE` keeps that from happening.
pub fn region_colour(region: u8, colourblind: bool) -> Color {
    let palette = if colourblind {
        &REGIONS_COLOURBLIND
    } else {
        &REGIONS
    };
    palette[usize::from(region) % palette.len()]
}

/// The names matching whichever palette is in use, for hint text.
pub fn region_names(colourblind: bool) -> RegionNames<'static> {
    RegionNames::colours(if colourblind {
        &REGION_NAMES_COLOURBLIND
    } else {
        &REGION_NAMES
    })
}

// --- type ---------------------------------------------------------------

/// Space Grotesk, embedded so every platform renders the same chunky,
/// geometric type regardless of what happens to be installed locally.
/// `assets/fonts/OFL.txt` is its licence and travels with the binary that
/// carries it. A single variable file spans the whole weight range this game
/// uses, from the body copy to the headline a screen is named by.
const SPACE_GROTESK: &[u8] = include_bytes!("../assets/fonts/SpaceGrotesk-Variable.ttf");

/// The base unit every spacing value in the UI is a multiple of, so the layout
/// reads as one deliberate grid rather than assorted paddings picked by eye.
pub const GRID: f32 = 8.0;

/// The one-word brand headline on the main menu: as oversized as the window
/// comfortably allows, because the name of the game is not a label, it is the
/// first thing on screen.
const SIZE_HERO: f32 = 84.0;
/// A screen's own name, and the handful of overlay moments that stand in for
/// one (paused, solved, building a puzzle).
const SIZE_TITLE: f32 = 46.0;
/// An uppercase section header inside a panel ("BOARD SIZE", "DIFFICULTY").
const SIZE_LABEL: f32 = 14.0;
/// Supporting prose under a title or a control.
const SIZE_BODY: f32 = 17.0;
/// Small print, sat below the content it belongs to.
const SIZE_CAPTION: f32 = 13.0;

/// Plain text at a given size and colour, in the game's one typeface.
pub fn text(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font_size: FontSize::Px(size),
            weight: FontWeight::MEDIUM,
            ..default()
        },
        TextColor(color),
    )
}

/// The one constructor every text helper here builds on, so each can pick its
/// own complete `TextFont` without two of them ever landing on the same
/// entity — Bevy panics at spawn if they do.
fn text_with(content: impl Into<String>, font: TextFont, color: Color) -> impl Bundle {
    (Text::new(content), font, TextColor(color))
}

/// A number that should not jitter width as its digits change: the clock, a
/// queen count, a share code. Bold, and set with tabular figures so every
/// digit takes the same width.
pub fn numeric(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    text_with(
        content,
        TextFont {
            font_size: FontSize::Px(size),
            weight: FontWeight::BOLD,
            font_features: FontFeatures::builder()
                .enable(FontFeatureTag::TABULAR_FIGURES)
                .build(),
            ..default()
        },
        color,
    )
}

/// The brand headline: "QUEENS" on the main menu, and nothing else.
pub fn hero(content: impl Into<String>) -> impl Bundle {
    (
        text_with(
            content.into().to_uppercase(),
            TextFont {
                font_size: FontSize::Px(SIZE_HERO),
                weight: FontWeight::BOLD,
                ..default()
            },
            TEXT,
        ),
        LetterSpacing::Px(0.5),
        Node {
            margin: UiRect::bottom(Val::Px(GRID)),
            ..default()
        },
    )
}

/// A screen's own name, rendered as a bold uppercase headline rather than a
/// caption: the hierarchy the rest of the screen sits under.
pub fn title(content: impl Into<String>) -> impl Bundle {
    (
        text_with(
            content.into().to_uppercase(),
            TextFont {
                font_size: FontSize::Px(SIZE_TITLE),
                weight: FontWeight::BOLD,
                ..default()
            },
            TEXT,
        ),
        LetterSpacing::Px(0.5),
        Node {
            margin: UiRect::bottom(Val::Px(GRID * 2.0)),
            ..default()
        },
    )
}

/// An uppercase, letter-spaced section header inside a panel — a structural
/// label, not a sentence, so it reads at a glance as the name of the group of
/// controls under it.
pub fn label(content: impl Into<String>) -> impl Bundle {
    (
        text_with(
            content.into().to_uppercase(),
            TextFont {
                font_size: FontSize::Px(SIZE_LABEL),
                weight: FontWeight::SEMIBOLD,
                ..default()
            },
            TEXT_DIM,
        ),
        LetterSpacing::Px(1.2),
    )
}

/// A quieter line of supporting text.
pub fn subtitle(content: impl Into<String>) -> impl Bundle {
    text(content, SIZE_BODY, TEXT_DIM)
}

/// Small print, pinned to the bottom edge of the screen it sits on rather than
/// sitting in the flow of the rest of the content.
///
/// The text is a child of the full-width box rather than a box of its own:
/// `justify_content` centres a child within its parent, but has nothing to
/// act on when the text sits directly on the positioned node itself.
pub fn footnote(content: impl Into<String>) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(GRID * 3.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![text(content, SIZE_CAPTION, TEXT_DIM)],
    )
}

// --- building blocks -------------------------------------------------------

/// A full-screen column that fills the window and centres its content.
pub fn screen(state_label: impl Bundle) -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(GRID * 3.0),
            padding: UiRect::all(Val::Px(GRID * 4.0)),
            ..default()
        },
        BackgroundColor(BACKGROUND),
        state_label,
    )
}

/// A flat block of colour that groups related controls. No border, no shadow:
/// the fill against the screen behind it is what marks it out as a group.
pub fn panel() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(GRID * 2.0),
            padding: UiRect::axes(Val::Px(GRID * 5.0), Val::Px(GRID * 4.0)),
            border_radius: BorderRadius::all(Val::Px(GRID)),
            ..default()
        },
        BackgroundColor(PANEL),
    )
}

/// A horizontal group of controls.
pub fn row(gap: f32) -> impl Bundle {
    Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        column_gap: Val::Px(gap),
        ..default()
    }
}

/// Remembers a button's resting colour so hover and press can be layered on top
/// without losing it — selectable buttons change their base as they toggle.
#[derive(Component, Clone, Copy)]
pub struct ButtonTint {
    pub base: Color,
}

impl Default for ButtonTint {
    fn default() -> Self {
        Self { base: BUTTON }
    }
}

/// A wide button for menu screens.
pub fn menu_button(label: &str) -> impl Bundle {
    tinted_button(label, BUTTON)
}

/// A wide button for the one action a screen wants to steer the player toward.
///
/// Separate from passing a [`ButtonTint`] alongside [`menu_button`], because a
/// bundle may not carry the same component twice.
pub fn accent_button(label: &str) -> impl Bundle {
    tinted_button(label, ACCENT)
}

fn tinted_button(label: &str, base: Color) -> impl Bundle {
    (
        Button,
        ButtonTint { base },
        Node {
            width: Val::Px(GRID * 32.0),
            padding: UiRect::axes(Val::Px(GRID * 2.5), Val::Px(GRID * 1.5)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(GRID * 0.75)),
            ..default()
        },
        BackgroundColor(base),
        children![(
            text_with(
                label.to_string(),
                TextFont {
                    font_size: FontSize::Px(19.0),
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TEXT,
            ),
            TextLayout::justify(Justify::Center)
        )],
    )
}

/// A compact button for toolbars and pickers.
pub fn small_button(label: &str) -> impl Bundle {
    (
        Button,
        ButtonTint::default(),
        Node {
            min_width: Val::Px(GRID * 6.5),
            padding: UiRect::axes(Val::Px(GRID * 1.75), Val::Px(GRID)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(GRID * 0.75)),
            ..default()
        },
        BackgroundColor(BUTTON),
        children![text_with(
            label.to_string(),
            TextFont {
                font_size: FontSize::Px(16.0),
                weight: FontWeight::SEMIBOLD,
                ..default()
            },
            TEXT,
        )],
    )
}

/// How quickly a button's colour and scale settle on their target: high enough
/// that the motion reads as a snap rather than a drift, low enough that it is
/// visibly a transition and not a cut.
const BUTTON_EASE_RATE: f32 = 22.0;
const HOVER_SCALE: f32 = 1.03;
const PRESSED_SCALE: f32 = 0.96;

/// Registers the hover and press feedback shared by every button.
pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        {
            // Overwrites the same asset id `FontSource::default()` resolves
            // to, so every `TextFont` in the game renders in this face without
            // threading a handle through every call site that builds one.
            let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
            fonts
                .insert(
                    AssetId::<Font>::default(),
                    Font::from_bytes(SPACE_GROTESK.to_vec()),
                )
                .expect("the default font id always accepts an overwrite");
        }
        app.add_systems(Update, animate_buttons);
    }
}

fn animate_buttons(
    time: Res<Time>,
    mut buttons: Query<(
        &Interaction,
        &ButtonTint,
        &mut BackgroundColor,
        &mut UiTransform,
    )>,
) {
    let k = 1.0 - (-BUTTON_EASE_RATE * time.delta_secs()).exp();
    for (interaction, tint, mut background, mut transform) in &mut buttons {
        let (target_color, target_scale) = match interaction {
            Interaction::None => (tint.base, 1.0),
            Interaction::Hovered => (mix(tint.base, BUTTON_HOVER, 0.55), HOVER_SCALE),
            Interaction::Pressed => (mix(tint.base, BUTTON_PRESSED, 0.7), PRESSED_SCALE),
        };
        background.0 = mix(background.0, target_color, k);
        let scale = transform.scale.x + (target_scale - transform.scale.x) * k;
        transform.scale = Vec2::splat(scale);
    }
}

/// Linear blend between two colours in sRGB.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_srgba();
    let b = b.to_srgba();
    Color::srgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    )
}

/// Formats a duration the way a puzzle timer should read.
pub fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as u32;
    let (minutes, seconds) = (total / 60, total % 60);
    if minutes >= 60 {
        format!("{}:{:02}:{:02}", minutes / 60, minutes % 60, seconds)
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hint names a region by its colour, so every colour a board can use
    /// must have a name, and no two may share one.
    #[test]
    fn every_region_colour_has_a_distinct_name() {
        for names in [REGION_NAMES, REGION_NAMES_COLOURBLIND] {
            assert_eq!(names.len(), REGIONS.len());
            let mut seen: Vec<&str> = Vec::new();
            for name in names {
                assert!(!seen.contains(&name), "{name} is used twice");
                seen.push(name);
            }
        }
    }

    /// The embedded font only carries the glyphs Space Grotesk ships with,
    /// which is a normal Latin set - but the game still promises plain ASCII
    /// throughout, so nothing here should start relying on anything wider.
    #[test]
    fn region_names_are_plain_ascii() {
        for name in REGION_NAMES.iter().chain(&REGION_NAMES_COLOURBLIND) {
            assert!(name.is_ascii(), "{name}");
        }
    }
}
