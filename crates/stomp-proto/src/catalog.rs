//! Module / model / parameter catalogue (docs/02).

/// A block in the signal chain. The discriminant is the page value of CC `0x3B` and, for the
/// eight effect modules, also the head CC number and the head-byte offset in a preset record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Module {
    OdFx = 0,
    Amp = 1,
    Cab = 2,
    Mod = 3,
    Delay = 4,
    Reverb = 5,
    Gate = 6,
    Eq = 7,
    Loop = 8,
    Drum = 9,
}

impl Module {
    pub const ALL: [Module; 10] = [
        Module::OdFx,
        Module::Amp,
        Module::Cab,
        Module::Mod,
        Module::Delay,
        Module::Reverb,
        Module::Gate,
        Module::Eq,
        Module::Loop,
        Module::Drum,
    ];

    /// The eight modules that have a head CC, models and parameters.
    pub const EFFECTS: [Module; 8] = [
        Module::OdFx,
        Module::Amp,
        Module::Cab,
        Module::Mod,
        Module::Delay,
        Module::Reverb,
        Module::Gate,
        Module::Eq,
    ];

    pub fn from_index(i: u8) -> Option<Module> {
        Module::ALL.get(i as usize).copied()
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn is_effect(self) -> bool {
        (self as u8) < 8
    }

    pub fn name(self) -> &'static str {
        match self {
            Module::OdFx => "OD/FX",
            Module::Amp => "AMP",
            Module::Cab => "CAB",
            Module::Mod => "MOD",
            Module::Delay => "DELAY",
            Module::Reverb => "REVERB",
            Module::Gate => "GATE",
            Module::Eq => "EQ",
            Module::Loop => "LOOP",
            Module::Drum => "DRUM",
        }
    }

    pub fn head_cc(self) -> Option<u8> {
        self.is_effect().then_some(self as u8)
    }

    /// First parameter CC and number of parameter CCs.
    pub fn param_ccs(self) -> Option<(u8, u8)> {
        Some(match self {
            Module::Gate => (0x08, 2),
            Module::OdFx => (0x0C, 4),
            Module::Amp => (0x12, 6),
            Module::Cab => (0x1A, 5),
            Module::Eq => (0x20, 12),
            Module::Mod => (0x2C, 4),
            Module::Delay => (0x30, 4),
            Module::Reverb => (0x36, 4),
            Module::Loop | Module::Drum => return None,
        })
    }

    /// Which module (and param slot) a parameter CC belongs to.
    pub fn from_param_cc(cc: u8) -> Option<(Module, usize)> {
        Module::EFFECTS.into_iter().find_map(|m| {
            let (first, n) = m.param_ccs()?;
            (first..first + n)
                .contains(&cc)
                .then(|| (m, (cc - first) as usize))
        })
    }

    pub fn models(self) -> &'static [&'static str] {
        match self {
            Module::OdFx => OD_MODELS,
            Module::Amp => AMP_MODELS,
            Module::Cab => CAB_MODELS,
            Module::Mod => MOD_MODELS,
            Module::Delay => DELAY_MODELS,
            Module::Reverb => REVERB_MODELS,
            Module::Gate => &["Noise Gate"],
            Module::Eq => &["6-band EQ", "10-band EQ"],
            Module::Loop | Module::Drum => &[],
        }
    }

    pub fn model_name(self, model: u8) -> String {
        match self.models().get(model as usize) {
            Some(n) => (*n).to_owned(),
            None => format!("Model {}", model + 1),
        }
    }

    /// Parameters of a model in app display order.
    pub fn params(self, model: u8) -> Vec<ParamDef> {
        let m = model as usize;
        match self {
            Module::OdFx => {
                let mut v = knobs(OD_PARAMS.get(m).copied().unwrap_or(&[]));
                if m == 13 {
                    v[0] = ParamDef::enumerated(0, "GAIN BOOST", &["OFF", "ON"]); // Boost: a switch
                }
                v
            }
            Module::Amp => knobs(&["GAIN", "BASS", "MIDDLE", "TREBLE", "PRESENCE", "MASTER"]),
            Module::Cab => {
                let mut v = vec![
                    ParamDef::enumerated(0, "MIC", MICS),
                    ParamDef::enumerated(1, "POSITION", POSITIONS),
                    ParamDef::knob(2, "LOW CUT"),
                    ParamDef::knob(3, "HIGH CUT"),
                    ParamDef::knob(4, "VOLUME"),
                ];
                if model >= 30 {
                    v.drain(..2); // user IRs hide MIC / POSITION
                }
                v
            }
            Module::Mod => MOD_PARAMS.get(m).map(|p| p.to_vec()).unwrap_or_default(),
            Module::Delay => knobs(if m == 2 {
                &["TIME", "FEEDBACK", "MOD", "LEVEL"]
            } else {
                &["TIME", "FEEDBACK", "LEVEL"]
            }),
            Module::Reverb => knobs(REVERB_PARAMS.get(m).copied().unwrap_or(&[])),
            Module::Gate => knobs(&["THRESHOLD", "DECAY"]),
            Module::Eq => {
                let names: &[&'static str] = if model == 1 { EQ10 } else { EQ6 };
                names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| ParamDef {
                        slot: i as u8,
                        name: n,
                        kind: if model == 1 && (i == 0 || i == 11) {
                            ParamKind::Knob
                        } else {
                            ParamKind::Bipolar
                        },
                    })
                    .collect()
            }
            Module::Loop | Module::Drum => vec![],
        }
    }

    /// The parameter definition for a given param slot of a model, if the model uses it.
    pub fn param(self, model: u8, slot: usize) -> Option<ParamDef> {
        self.params(model).into_iter().find(|p| p.slot as usize == slot)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    /// 0–100 knob.
    Knob,
    /// 0–100 with 50 = centre (EQ bands).
    Bipolar,
    /// Index into a list of choices.
    Enum(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamDef {
    /// Offset from the module's first param CC (= offset in the preset record).
    pub slot: u8,
    pub name: &'static str,
    pub kind: ParamKind,
}

impl ParamDef {
    const fn knob(slot: u8, name: &'static str) -> Self {
        ParamDef { slot, name, kind: ParamKind::Knob }
    }

    const fn enumerated(slot: u8, name: &'static str, opts: &'static [&'static str]) -> Self {
        ParamDef { slot, name, kind: ParamKind::Enum(opts) }
    }

    /// Human-readable value.
    pub fn format(&self, v: u8) -> String {
        match self.kind {
            ParamKind::Knob => v.to_string(),
            ParamKind::Bipolar => format!("{:+}", v as i16 - 50),
            ParamKind::Enum(opts) => opts
                .get(v as usize)
                .map(|s| (*s).to_owned())
                .unwrap_or_else(|| v.to_string()),
        }
    }
}

fn knobs(names: &[&'static str]) -> Vec<ParamDef> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| ParamDef::knob(i as u8, n))
        .collect()
}

pub const OD_MODELS: &[&str] = &[
    "Red Comp", "RS Comp", "Yellow Dist", "Boost RC", "Boost AC", "DS1", "TS9", "BD2", "Klone", "Ratt2",
    "Red Drive", "Cruncher", "Muff", "Boost", "Face Fuzz", "ST Drive",
];

const OD_PARAMS: &[&[&str]] = &[
    &["OUTPUT", "SENSITIVITY"],
    &["SUSTAIN", "LEVEL"],
    &["OUTPUT", "DISTORTION"],
    &["GAIN", "VOLUME", "BASS", "TREBLE"],
    &["GAIN", "VOLUME", "BASS", "TREBLE"],
    &["GAIN", "TONE", "VOLUME"],
    &["DRIVE", "TONE", "LEVEL"],
    &["LEVEL", "TONE", "GAIN"],
    &["VOLUME", "DRIVE", "TONE"],
    &["DIST", "FILTER", "VOLUME"],
    &["DRIVE", "TONE", "LEVEL"],
    &["VOLUME", "TONE", "GAIN"],
    &["VOLUME", "TONE", "SUSTAIN"],
    &["GAIN BOOST", "VOLUME"],
    &["GAIN", "TONE", "OUTPUT"],
    &["VOLUME", "GAIN", "FILTER"],
];

pub const AMP_MODELS: &[&str] = &[
    "Black Twin", "Tweed 5F1", "Bassguy", "Prince Rev", "Tweed 5F8", "Deluxe Rev", "Bdotone CL", "Voxy 30HW",
    "Mar J800", "Mar J45", "Mar Plexi100", "Jazz CL", "Mes Rector", "Mes MKIV Lead", "Mes MKV Red",
    "Mes LStar CL", "Cornfed M50", "ODS-100", "Spawn Qrod", "Soloist 100", "Fryman HB", "Fryman DS", "PVH 61",
    "Eddie 51", "2Rock J35", "Ampug SV Bass", "Ampug Port Bass", "FR Bass", "Mes 400 Bass", "Agula DP751",
    "USER AMP 01", "USER AMP 02", "USER AMP 03", "USER AMP 04", "USER AMP 05", "USER AMP 06", "USER AMP 07",
    "USER AMP 08", "USER AMP 09", "USER AMP 10",
];

pub const CAB_MODELS: &[&str] = &[
    "Twin 212", "Tweed 110", "Bassguy 410", "Prince 112", "Tweed 210", "Deluxe 112", "Bdotone 212", "Voxy 212",
    "Mar G12M 412", "Mar Silver 212", "Mar V30 412", "Jazz 212", "Mes OS 412", "Mes Trad 412", "Mes V30 212",
    "Mes EV 112", "Cornfed 212", "ODS 212", "Spawn 412", "SLO 412", "Fryman 412", "Fryman 212", "PVH 412",
    "Eddie 412", "2Rock 212", "Ampug 810", "Ampug 410", "FR 210", "Mes 410", "Agula 810",
    "USER IR 01", "USER IR 02", "USER IR 03", "USER IR 04", "USER IR 05", "USER IR 06", "USER IR 07",
    "USER IR 08", "USER IR 09", "USER IR 10",
];

pub const MICS: &[&str] = &["E906", "M160", "MD421", "R121", "SM57", "SM58", "SM7B", "U47"];
pub const POSITIONS: &[&str] = &["EDGE", "MIDDLE", "CENTER"];

pub const MOD_MODELS: &[&str] = &[
    "Chorus-1", "Chorus-2", "Stereo Chorus", "Vibrato", "Detune", "Flanger", "Phaser 90", "Phaser 100", "Chorus+",
    "Uni Vibe", "Tremolo", "Chorus-3", "Harmony",
];

const KEYS: &[&str] = &["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const HARM: &[&str] = &["-8TH", "-6TH", "-5TH", "-4TH", "-3TH", "3TH", "4TH", "5TH", "6TH", "8TH"];

const MOD_PARAMS: &[&[ParamDef]] = &[
    &[ParamDef::knob(0, "INTENSITY"), ParamDef::knob(1, "DEPTH"), ParamDef::knob(2, "RATE")],
    &[ParamDef::knob(0, "RATE"), ParamDef::knob(1, "DEPTH")],
    &[ParamDef::knob(0, "INTENSITY"), ParamDef::knob(1, "WIDTH"), ParamDef::knob(2, "RATE")],
    &[ParamDef::knob(0, "RATE"), ParamDef::knob(1, "DEPTH")],
    &[ParamDef::knob(0, "L"), ParamDef::knob(1, "MIX"), ParamDef::knob(2, "R")],
    &[
        ParamDef::knob(0, "LEVEL"),
        ParamDef::knob(1, "RATE"),
        ParamDef::knob(2, "WIDTH"),
        ParamDef::knob(3, "FEEDBACK"),
    ],
    &[ParamDef::knob(0, "SPEED")],
    &[ParamDef::knob(0, "INTENSITY"), ParamDef::knob(1, "SPEED")],
    &[
        ParamDef::knob(0, "SPEED"),
        ParamDef::knob(1, "WIDTH"),
        ParamDef::enumerated(2, "MODE", &["CHORUS", "FLANGER"]),
        ParamDef::knob(3, "INTENSITY"),
    ],
    &[
        ParamDef::knob(0, "SPEED"),
        ParamDef::knob(1, "VOLUME"),
        ParamDef::knob(2, "INTENSITY"),
        ParamDef::enumerated(3, "MODE", &["CHORUS", "VIBE"]),
    ],
    &[ParamDef::knob(0, "RATE"), ParamDef::knob(1, "DEPTH")],
    &[ParamDef::knob(0, "RATE"), ParamDef::knob(1, "DEPTH"), ParamDef::knob(2, "TONE")],
    &[
        ParamDef::enumerated(0, "KEY", KEYS),
        ParamDef::enumerated(1, "MODE", &["MINOR", "MAJOR"]),
        ParamDef::enumerated(2, "HARM", HARM),
        ParamDef::knob(3, "MIX"),
    ],
];

pub const DELAY_MODELS: &[&str] = &[
    "Analog Delay", "Digital Delay", "Mod Delay", "Tape Delay", "Reverse Delay", "Pan Delay", "Physical Delay",
];

pub const REVERB_MODELS: &[&str] = &["Room", "Hall", "Plate", "Spring", "Shimmer", "Damp"];

const REVERB_PARAMS: &[&[&str]] = &[
    &["DECAY", "TONE", "LEVEL"],
    &["DECAY", "TONE", "PRE DELAY", "LEVEL"],
    &["DECAY", "LEVEL"],
    &["DECAY", "LEVEL"],
    &["LEVEL", "DECAY", "SHIMMER"],
    &["LEVEL", "DEPTH"],
];

const EQ6: &[&str] = &["100Hz", "220Hz", "500Hz", "1.2kHz", "2.5kHz", "6.4kHz"];
const EQ10: &[&str] = &[
    "GAIN", "31Hz", "62.5Hz", "125Hz", "250Hz", "500Hz", "1kHz", "2kHz", "4kHz", "8kHz", "16kHz", "VOLUME",
];

pub const NOTES: &[&str] = &["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

pub const DRUM_STYLES: &[&str] = &[
    "Pop", "Rock", "Metal", "Disco", "Funk", "Blues", "Hip Hop", "Jazz", "Latin", "Metro",
];
const METRO: &[&str] = &["4/4", "2/4", "3/4", "5/4", "6/8", "7/8", "9/8", "10/8", "11/8", "12/8"];

/// Drum pattern name from the flat value (`style*10 + pattern`).
pub fn drum_pattern_name(v: u8) -> String {
    let (style, pat) = ((v / 10) as usize, (v % 10) as usize);
    match DRUM_STYLES.get(style) {
        Some(&"Metro") => format!("Metro {}", METRO[pat]),
        Some(s) => format!("{s} {:02}", pat + 1),
        None => format!("Pattern {v}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_cc_lookup() {
        assert_eq!(Module::from_param_cc(0x12), Some((Module::Amp, 0)));
        assert_eq!(Module::from_param_cc(0x09), Some((Module::Gate, 1)));
        assert_eq!(Module::from_param_cc(0x2B), Some((Module::Eq, 11)));
        assert_eq!(Module::from_param_cc(0x3A), None);
    }

    #[test]
    fn model_counts() {
        assert_eq!(OD_MODELS.len(), 16);
        assert_eq!(AMP_MODELS.len(), 40);
        assert_eq!(CAB_MODELS.len(), 40);
        assert_eq!(MOD_MODELS.len(), 13);
        assert_eq!(MOD_PARAMS.len(), 13);
        assert_eq!(OD_PARAMS.len(), 16);
    }

    #[test]
    fn drum_names() {
        assert_eq!(drum_pattern_name(13), "Rock 04");
        assert_eq!(drum_pattern_name(92), "Metro 3/4");
    }
}
