# Architecture

How the app is put together, for anyone changing it. Everything about the pedal itself (MIDI messages, records,
state tracking, the connection engine) lives in the [open-stomp](https://github.com/JackMechem/open-stomp-lib)
library; see its [library guide](https://github.com/JackMechem/open-stomp-lib/blob/main/docs/10-library-guide.md).
The app is a [Tauri](https://tauri.app) program: a Rust side that owns the engine and the pedal state, and a React
interface in the system's web view.

## Source files

`app/src-tauri/src/` (Rust)

| File | What it holds |
|---|---|
| `main.rs` | Command line, version strings, the Tauri commands (`snapshot`, `command`, `log_rows`, `log_export`), startup |
| `hub.rs` | The hub thread: owns the `Device` engine and the `DeviceState`, applies updates, runs commands (`Cmd`), emits `state` events |
| `view.rs` | The state cut into JSON sections for the frontend |
| `log.rs`, `logbook.rs` | The activity log: rows, merging knob sweeps, filtering, text export, and the rows the frontend asks for |
| `import.rs` | NAM / IR conversion on a worker thread, with progress and cancel |

`app/src/` (TypeScript, React, CSS)

| File | What it holds |
|---|---|
| `store.ts` | The frontend's copy of the sections; `useStore` subscribes a component to just what it reads; `send` runs a command |
| `types.ts` | The JSON shapes from `view.rs` and the commands in `hub::Cmd`; keep them in step |
| `ui.ts` | UI-only state: the open dialog, whether the activity log shows |
| `App.tsx` | Layout, the pinned / followed block, keyboard shortcuts, `STOMP_DEV` flags |
| `components/` | One component per area (`TopBar`, `PresetList`, `CurrentPreset`, `Chain`, `Detail`, `Extras`, `DeviceCard`, `LogPanel`, `Dialogs`) and the controls (`Knob`, `Slider`, `ModelPicker`), each with its own `.css` |
| `styles/global.css` | Design tokens (colours, radii, spacing), the module colours, shared buttons and the highlight animation |

## Data flow

```
 pedal ◀──▶ open-stomp Device engine thread (port scan, connect, sync, request queue)
                 │ Update (+ wake-up)                   ▲ Device helpers: select_preset, set_head,
                 ▼                                      │ set_param, save_preset, write_settings, …
            hub thread (hub.rs) ─────────────────────────┘
              Message → DeviceState::apply → Event → activity log, highlights
              every ≤16 ms: rebuild sections (view.rs), emit the changed ones
                 │ "state" event {seq, sections, activity, rx, tx}      ▲ invoke("command", Cmd)
                 ▼                                                      │
            store.ts ── useStore ──▶ components ── user input ── send ──┘
```

1. `Hub::spawn` starts the engine with `Device::spawn_with_waker`, whose wake-up sends a message to the hub's
   channel. Frontend commands arrive on the same channel, so they run in order with the pedal's updates.
2. The hub drains the engine's `Update`s:
   * `Message`: applied to the `DeviceState`, which returns an `Event` (category, subject, value). The event goes to
     the activity log and, unless redundant, becomes an `activity` entry that makes the block, knob or preset header
     light up.
   * `Reset`: a new session; the `DeviceState` is replaced with a fresh one.
   * `Status`, `Sync`, `Ports`: the `conn` section.
   * `Mark`, `Info`: notes in the log.
3. After a batch, at most once per frame, the hub rebuilds every section, compares each with what it last sent, and
   emits only the changed ones as one `state` event. A section only changes when its contents do, so a knob sweep
   sends the `live` section, not the 99 presets.
4. On start, the frontend subscribes to `state`, then asks for a `snapshot` of every section. Each section carries
   the number of the event that last set it, so a snapshot that arrives late can't overwrite newer data.

### Sections

| Section | Contents | Changes when |
|---|---|---|
| `conn` | Connection status, ports, sync progress, listen-only, `editable`, `canEditSound` | Connecting, syncing, replay progress |
| `presets` | All 99 names, the active preset, whether it's edited | Preset changes, saves, sync |
| `live` | The live sound: chain order, every block's model, on/off and parameters, volume, the followed block | Every edit |
| `models` | Model names per module, with user slot names | User slots change |
| `extras` | Tuner, looper, drum machine | Those change |
| `device` | Product, firmware, system settings, user slot names | Sync, settings writes, imports, renames |
| `log` | Row count and a generation number | Every message |
| `import` | The conversion in progress and the last outcome | Import progress |
| `catalog` | Drum patterns, `STOMP_DEV` flags, versions | Never |

### Why edits don't touch the state

The pedal doesn't echo what the host sends. The engine reports every message it sends back as an
`Update::Message` with `Origin::Host`, and the hub applies those exactly like pedal messages. So a knob turned in the
app updates `DeviceState` through the same path as a knob turned on the pedal, the log shows it as a Host row, and
there is one source of truth. Anything sent while listen-only or disconnected is refused by the engine (and the hub
doesn't send it in the first place).

The knobs and sliders show their own value while you turn them and send at most one change per animation frame;
when you let go they show the pedal's value again once its echo has arrived.

### Editing rules

* `editable`: connected and not listen-only. `canEditSound` also needs the live preset to be known, because one head
  CC carries model and on/off together, so switching a block on needs its current model. The hub checks the same
  rules before sending.
* Saving uses the live sound (the edit buffer with all edits) with the chosen slot and name, through
  `Device::save_preset`.
* Settings changes start from the record the pedal sent, change the given fields and `write_settings` the whole
  record; the engine collapses queued settings writes into the newest.
* Opening a block in the chain also sends the pedal-page CC, as the official app does.
* After a preset change from the pedal, its messages are held until it has been quiet for 0.15 s (at most 5 s), then
  applied together, so turning the preset knob several clicks jumps instead of stepping through every preset.

## The activity log

The log stays on the Rust side (`log.rs`, `logbook.rs`), so the frontend never holds 50,000 rows. The panel asks for
the rows on screen (`log_rows`, with a few extra above and below) whenever the `log` section's generation, the filter
or the scroll position changes.

* **Merging:** an event with a `coalesce` key merges into a recent row with the same key (within 2.5 s, looking back
  past up to 12 hidden plumbing rows), updating its value, keeping the starting value (`from`) and counting messages
  (`×4`).
* **Filtering:** the chips and search go to the hub (`SetLogFilter`). The visible-row index list is cached by (log
  generation, filter).
* Rows are capped at 50,000.

## Highlights

Each `activity` entry bumps a counter in the store, keyed by module, parameter or preset header. A component renders
`<span key={counter} className="glow" />`; a new key remounts the element, which replays its CSS animation. The
In / Out lights in the top bar work the same way with the `rx` / `tx` counts.

## Screenshots and tests

`tools/dev/webdriver.py` drives the real app through `WebKitWebDriver` (Linux). Debug builds expose the store as
`window.__stomp`, so scripts can wait for the pedal state instead of guessing timings. `tools/dev/shot.py` uses it
for the docs' screenshots.

## Adding a feature

* **New pedal behaviour** (a message, a record field, a command): add it to the library first, with a test against a
  capture if possible, then use it here.
* **Showing something new:** add it to a section in `view.rs` (or a new section), mirror it in `types.ts`, and read it
  with `useStore` in a component.
* **Sending something new:** add a `Cmd` variant in `hub.rs` that calls a `Device` helper (so listen-only and the
  log keep working), mirror it in `types.ts`, and `send` it. The hub's tests parse the frontend's JSON for the
  trickier commands.
* **Styling:** use the variables in `global.css`. A block's colour comes from `data-module="amp"` and is read as
  `var(--mod)`.
