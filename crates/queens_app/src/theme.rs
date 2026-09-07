//! Colours and the shared UI vocabulary every screen is built from.

use bevy::prelude::*;

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

pub const BACKGROUND: Color = rgb(0x16181F);
pub const PANEL: Color = rgb(0x1E212B);
pub const PANEL_EDGE: Color = rgb(0x323747);
pub const TEXT: Color = rgb(0xEAECF2);
pub const TEXT_DIM: Color = rgb(0x949BAE);
pub const ACCENT: Color = rgb(0x5AA9FA);
pub const SUCCESS: Color = rgb(0x5FD48A);
pub const DANGER: Color = rgb(0xEC5A5C);

pub const BUTTON: Color = rgb(0x2A2F3D);
pub const BUTTON_HOVER: Color = rgb(0x3A4153);
pub const BUTTON_PRESSED: Color = rgb(0x4A5570);

/// Bold separator between two regions.
pub const REGION_EDGE: Color = rgb(0x14161C);
/// Faint separator between two cells of the same region.
pub const CELL_EDGE: Color = Color::srgba(0.08, 0.09, 0.11, 0.28);
/// The queen token, which sits on top of whatever colour its region is: a dark
/// disc, so the crown inside it reads the same against every region colour.
pub const QUEEN_BODY: Color = rgb(0x14161C);
pub const QUEEN_RING: Color = rgb(0xF6F7FB);
/// The crown drawn inside the disc, and the gem set into its band.
pub const QUEEN_CROWN: Color = rgb(0xFBFCFE);
pub const QUEEN_GEM: Color = rgb(0xF4B740);
/// The player's "no queen here" cross. Opaque, and deliberately softer than
/// the queen's near-black: the two strokes overlap, and a translucent colour
/// would blend with itself and leave a dark patch at the crossing.
pub const CROSS: Color = rgb(0x333A4B);

/// Region colours: light enough that the dark queen token and cross stay
/// legible on every one of them.
const REGIONS: [Color; 12] = [
    rgb(0xF7A28D),
    rgb(0xFFD46A),
    rgb(0xB7E07C),
    rgb(0x7DD3C0),
    rgb(0x8FC6F5),
    rgb(0xB3A6F0),
    rgb(0xF6A0C8),
    rgb(0xD9C39A),
    rgb(0xA8D8A0),
    rgb(0xEFBDF2),
    rgb(0x9BB3E8),
    rgb(0xE4E08A),
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
            row_gap: Val::Px(18.0),
            padding: UiRect::all(Val::Px(24.0)),
            ..default()
        },
        BackgroundColor(BACKGROUND),
        state_label,
    )
}

/// A raised card to group related controls.
pub fn panel() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(14.0),
            padding: UiRect::axes(Val::Px(34.0), Val::Px(28.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(14.0)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(PANEL_EDGE),
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

/// Plain text at a given size and colour.
pub fn text(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

/// A screen title.
pub fn title(content: impl Into<String>) -> impl Bundle {
    (
        text(content, 46.0, TEXT),
        Node {
            margin: UiRect::bottom(Val::Px(6.0)),
            ..default()
        },
    )
}

/// A quieter line of supporting text.
pub fn subtitle(content: impl Into<String>) -> impl Bundle {
    text(content, 17.0, TEXT_DIM)
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
            width: Val::Px(260.0),
            padding: UiRect::axes(Val::Px(20.0), Val::Px(13.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(base),
        BorderColor::all(PANEL_EDGE),
        children![(
            text(label, 20.0, TEXT),
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
            min_width: Val::Px(52.0),
            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(BUTTON),
        BorderColor::all(PANEL_EDGE),
        children![text(label, 17.0, TEXT)],
    )
}

/// Registers the hover and press feedback shared by every button.
pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tint_buttons);
    }
}

fn tint_buttons(
    mut buttons: Query<
        (&Interaction, &ButtonTint, &mut BackgroundColor),
        Or<(Changed<Interaction>, Changed<ButtonTint>)>,
    >,
) {
    for (interaction, tint, mut background) in &mut buttons {
        // Selected buttons carry a bright base of their own; nudging it toward
        // the shared hover colours would wash that out, so blend instead.
        background.0 = match interaction {
            Interaction::None => tint.base,
            Interaction::Hovered => mix(tint.base, BUTTON_HOVER, 0.55),
            Interaction::Pressed => mix(tint.base, BUTTON_PRESSED, 0.7),
        };
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
