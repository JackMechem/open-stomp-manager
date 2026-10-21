//! Pure encode/decode for the SORAN Dual Stomp (Lite) USB-MIDI protocol.
//!
//! No I/O lives here so that the captures in `captures/logs/` can serve as test vectors.
//! See `docs/` at the repository root for the protocol description.

pub mod catalog;
pub mod capture;
pub mod midi;
pub mod pack;
pub mod records;
pub mod state;

pub use catalog::Module;
pub use midi::{Assembler, Msg};
pub use records::{Preset, Settings};
pub use state::{Category, DeviceState, Event, Origin};
