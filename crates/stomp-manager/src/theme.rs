//! Colours, fonts and the egui style.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle};
use stomp_proto::{Category, Module};

pub const BG: Color32 = Color32::from_rgb(0x0D, 0x0F, 0x13);
pub const PANEL: Color32 = Color32::from_rgb(0x13, 0x16, 0x1C);
pub const CARD: Color32 = Color32::from_rgb(0x19, 0x1D, 0x25);
pub const CARD_HI: Color32 = Color32::from_rgb(0x21, 0x26, 0x30);
pub const STROKE: Color32 = Color32::from_rgb(0x2A, 0x30, 0x3C);
pub const TEXT: Color32 = Color32::from_rgb(0xE9, 0xEB, 0xF1);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9A, 0xA2, 0xB2);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x5E, 0x66, 0x75);
pub const ACCENT: Color32 = Color32::from_rgb(0xFF, 0xAE, 0x42);
pub const GOOD: Color32 = Color32::from_rgb(0x3E, 0xDB, 0x8A);
pub const BAD: Color32 = Color32::from_rgb(0xFF, 0x5D, 0x6C);
pub const HOST: Color32 = Color32::from_rgb(0x6C, 0xA8, 0xFF);
pub const MARK: Color32 = Color32::from_rgb(0xD4, 0x7C, 0xFF);

pub fn module_color(m: Module) -> Color32 {
    match m {
        Module::Gate => Color32::from_rgb(0x7F, 0xD8, 0xC0),
        Module::OdFx => Color32::from_rgb(0xFF, 0x9F, 0x43),
        Module::Amp => Color32::from_rgb(0xFF, 0x5E, 0x5B),
        Module::Cab => Color32::from_rgb(0xD6, 0xAE, 0x82),
        Module::Eq => Color32::from_rgb(0xFE, 0xD3, 0x5A),
        Module::Mod => Color32::from_rgb(0x4C, 0xD7, 0xF6),
        Module::Delay => Color32::from_rgb(0x5B, 0x9C, 0xFF),
        Module::Reverb => Color32::from_rgb(0xA9, 0x8B, 0xFF),
        Module::Loop => Color32::from_rgb(0xFF, 0x6F, 0xB0),
        Module::Drum => Color32::from_rgb(0x3E, 0xDB, 0x8A),
    }
}

pub fn category_color(c: Category) -> Color32 {
    match c {
        Category::Effect(m) => module_color(m),
        Category::Preset => ACCENT,
        Category::Volume => Color32::from_rgb(0xFF, 0xCF, 0x8A),
        Category::Navigation => TEXT_DIM,
        Category::Looper => module_color(Module::Loop),
        Category::Drum => module_color(Module::Drum),
        Category::Tuner => Color32::from_rgb(0x9C, 0xF2, 0x6B),
        Category::Settings | Category::Device => Color32::from_rgb(0xB8, 0xC4, 0xD8),
        Category::Protocol => TEXT_FAINT,
        Category::Unknown => BAD,
    }
}

pub fn module_icon(m: Module) -> &'static str {
    use egui_phosphor::regular as i;
    match m {
        Module::Gate => i::FUNNEL,
        Module::OdFx => i::LIGHTNING,
        Module::Amp => i::SPEAKER_HIGH,
        Module::Cab => i::VINYL_RECORD,
        Module::Eq => i::FADERS,
        Module::Mod => i::WAVE_SINE,
        Module::Delay => i::REPEAT,
        Module::Reverb => i::WAVEFORM,
        Module::Loop => i::CASSETTE_TAPE,
        Module::Drum => i::METRONOME,
    }
}

pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

pub fn mono_bold() -> FontFamily {
    FontFamily::Name("mono-bold".into())
}

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn font_b(size: f32) -> FontId {
    FontId::new(size, bold())
}

pub fn font_sb(size: f32) -> FontId {
    FontId::new(size, semibold())
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn mono_b(size: f32) -> FontId {
    FontId::new(size, mono_bold())
}

pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn fonts() -> FontDefinitions {
    macro_rules! font {
        ($f:literal) => {
            FontData::from_static(include_bytes!(concat!("../assets/fonts/", $f))).into()
        };
    }
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("inter".into(), font!("Inter-Regular.ttf"));
    fonts.font_data.insert("inter-semibold".into(), font!("Inter-SemiBold.ttf"));
    fonts.font_data.insert("inter-bold".into(), font!("Inter-Bold.ttf"));
    fonts.font_data.insert("jbm".into(), font!("JetBrainsMono-Regular.ttf"));
    fonts.font_data.insert("jbm-bold".into(), font!("JetBrainsMono-Bold.ttf"));

    let fallbacks: Vec<String> = fonts.families[&FontFamily::Proportional].clone();
    let fam = |first: &str| {
        let mut v = vec![first.to_owned()];
        v.extend(fallbacks.iter().cloned());
        v
    };
    fonts.families.insert(FontFamily::Proportional, fam("inter"));
    fonts.families.insert(bold(), fam("inter-bold"));
    fonts.families.insert(semibold(), fam("inter-semibold"));
    fonts.families.insert(FontFamily::Monospace, fam("jbm"));
    fonts.families.insert(mono_bold(), fam("jbm-bold"));

    // icons, available in every family
    let icons = egui_phosphor::Variant::Regular.font_bytes();
    fonts.font_data.insert("phosphor".into(), FontData::from_static(icons).into());
    for keys in fonts.families.values_mut() {
        keys.insert(1.min(keys.len()), "phosphor".into());
    }
    fonts
}

pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.text_styles = [
            (TextStyle::Heading, font_b(20.0)),
            (TextStyle::Body, font(14.0)),
            (TextStyle::Button, font(13.5)),
            (TextStyle::Small, font(11.5)),
            (TextStyle::Monospace, mono(12.5)),
        ]
        .into();
        let s = &mut style.spacing;
        s.item_spacing = egui::vec2(8.0, 6.0);
        s.button_padding = egui::vec2(10.0, 5.0);
        s.interact_size.y = 26.0;
        s.menu_margin = egui::Margin::same(10);

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.panel_fill = PANEL;
        v.window_fill = CARD;
        v.window_stroke = Stroke::new(1.0, STROKE);
        v.window_corner_radius = CornerRadius::same(12);
        v.menu_corner_radius = CornerRadius::same(10);
        v.extreme_bg_color = BG;
        v.faint_bg_color = CARD;
        v.code_bg_color = CARD_HI;
        v.hyperlink_color = ACCENT;
        v.selection.bg_fill = with_alpha(ACCENT, 0.35);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.override_text_color = None;
        v.window_shadow = egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(140) };
        v.popup_shadow = v.window_shadow;

        let r = CornerRadius::same(8);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = CARD;
        w.noninteractive.weak_bg_fill = CARD;
        w.noninteractive.bg_stroke = Stroke::new(1.0, STROKE);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
        w.noninteractive.corner_radius = r;
        w.inactive.bg_fill = CARD_HI;
        w.inactive.weak_bg_fill = CARD_HI;
        w.inactive.bg_stroke = Stroke::new(1.0, STROKE);
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        w.inactive.corner_radius = r;
        w.hovered.bg_fill = mix(CARD_HI, TEXT, 0.08);
        w.hovered.weak_bg_fill = mix(CARD_HI, TEXT, 0.08);
        w.hovered.bg_stroke = Stroke::new(1.0, mix(STROKE, TEXT, 0.25));
        w.hovered.fg_stroke = Stroke::new(1.0, TEXT);
        w.hovered.corner_radius = r;
        w.hovered.expansion = 0.0;
        w.active.bg_fill = mix(CARD_HI, ACCENT, 0.25);
        w.active.weak_bg_fill = mix(CARD_HI, ACCENT, 0.25);
        w.active.bg_stroke = Stroke::new(1.0, ACCENT);
        w.active.fg_stroke = Stroke::new(1.0, TEXT);
        w.active.corner_radius = r;
        w.open = w.active;
    });
}
