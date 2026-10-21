# Architecture

How the app is put together, for anyone changing it. Everything about the pedal itself (MIDI messages, records,
state tracking, the connection engine) lives in the [open-stomp](https://github.com/JackMechem/open-stomp-lib)
library; see its [library guide](https://github.com/JackMechem/open-stomp-lib/blob/main/docs/10-library-guide.md).
The app is the UI on top.

## Source files

`crates/open-stomp-manager/src/`

| File | What it holds |
|---|---|
| `main.rs` | Command line, window options and icon, Windows console attach, version strings |
| `app.rs` | `StompApp`: all panels (top bar, preset list, header, chain, detail, tuner/looper/drum, device/settings, log panel, save dialog), sending edits, polling the engine, flash animations, the screenshot hook |
| `logbook.rs` | The activity log model: rows, merging knob sweeps, filtering, text export (unit-tested) |
| `widgets.rs` | Custom-painted widgets: cards, knobs, level sliders, LEDs, chips, arcs, filter chips, drag/scroll input |
| `theme.rs` | Colours, fonts (Inter, JetBrains Mono, Phosphor icons) and the egui style; colour per module and log category |
| `build.rs` | Git commit for the version; Windows icon and version resource |

## Data flow

```
          ┌───────────────── open-stomp (library) ─────────────────┐
 pedal ◀──┤ device::Device engine thread                          │
 (USB-MIDI)│   port scan · connect · startup sync · request queue  │
          └──────────────┬──────────────────────────▲──────────────┘
                 Update  │ (+ repaint via waker)    │ helpers: select_preset, set_head,
                         ▼                          │ set_param, save_preset, write_settings, …
          ┌──────────────────────── StompApp ───────┴──────────────┐
          │ poll(): Update::Message → DeviceState::apply → Event    │
          │                     ├→ LogBook::push_event (log panel) │
          │                     └→ flash timestamps (glow)          │
          │ panels read DeviceState and draw; user input → helpers  │
          └─────────────────────────────────────────────────────────┘
```

1. `StompApp::new` starts the engine with `Device::spawn_with_waker`, passing a closure that calls
   `ctx.request_repaint()`, so egui redraws whenever something arrives. The MIDI client is named "Open Stomp Manager".
2. Every frame, `poll()` drains the engine's `Update`s:
   * `Message`: applied to the app's `DeviceState`, which returns an `Event` (category, subject, value). The event
     goes to the log and drives the glow (`note_activity`).
   * `Reset`: a new session; the `DeviceState` is replaced with a fresh one.
   * `Status`, `Sync`, `Ports`: the top bar.
   * `Mark`, `Info`: notes in the log. The engine's `SystemTime` stamps become `chrono::DateTime<Local>` here.
3. Panels only **read** `self.state` to draw. User actions call the `Device` helpers (or `host::` builders through
   `send`), never mutate the state directly.

### Why edits don't touch the state

The pedal doesn't echo what the host sends. The engine reports every message it sends back as an
`Update::Message` with `Origin::Host`, and `poll()` applies those exactly like pedal messages. So a knob turned in the
app updates `DeviceState` through the same path as a knob turned on the pedal, the log shows it as a HOST row, and
there is one source of truth. Anything sent while listen-only or disconnected is refused by the engine and never
appears.

### Editing rules

* `editable()`: connected and not listen-only. `can_edit_sound()` also needs `live_known` (the edit buffer has been
  read), because one head CC carries model and on/off together, so switching a block on needs its current model.
* Saving uses `state.live` (the edit buffer with all edits) with the chosen slot and name, and
  `Device::save_preset(current, &preset)`.
* Settings changes clone `state.settings`, change one field and `write_settings` the whole record; the engine
  collapses queued settings writes into the newest.
* Opening a block in the chain also sends the pedal-page CC, as the official app does.

## The activity log

`LogBook` turns `Event`s into `Row`s:

* **Merging:** an event with a `coalesce` key merges into a recent row with the same key (within 2.5 s, looking back
  past up to 12 hidden plumbing rows), updating its value, keeping the starting value (`from`) and counting messages
  (`×4`).
* **Filtering:** `Filter` holds the category chips, direction, and the search text. The visible-row index list is
  cached by (log version, filter) in `view_cache`, so a 50,000-row log stays fast.
* Rows are capped at 50,000.

## Flash animations

`note_activity` records when each module, parameter, the preset header and each log row last changed (`module_flash`,
`param_flash`, `preset_flash`, `row_flash`). Drawing fades the glow over 0.9 s (`FLASH_SECS`). `animating()` keeps
requesting repaints while anything is fading or the looper is recording, and lets the app idle otherwise.

## Screenshots

`STOMP_SCREENSHOT=<file.ppm>:<seconds>` (in `app.rs`, `screenshot_hook`) asks egui for a screenshot after that many
seconds, writes it as a PPM and closes the window. With `--replay` it gives reproducible screenshots without a pedal:

```sh
STOMP_SCREENSHOT=/tmp/shot.ppm:8 cargo run -p open-stomp-manager -- --replay captures/logs/01-main-session.log --speed 60
```

## Adding a feature

* **New pedal behaviour** (a message, a record field, a command): add it to the library first, with a test against a
  capture if possible, then use it here.
* **New UI:** add a panel method in `app.rs` reading `self.state`; reuse `widgets.rs` (`card`, `knob`, `level_slider`,
  `chip`) and the `theme.rs` colours so it matches. Send through the `Device` helpers so listen-only and the log keep
  working.
