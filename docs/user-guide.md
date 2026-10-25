# User guide

Open Stomp Manager is a live monitor and editor for the SORAN Dual Stomp Lite. This guide covers everything in the
window. For installing see the [README](../README.md#install); for building from source see
[building.md](building.md).

![Open Stomp Manager replaying a capture](images/main-window.png)

* [Connecting](#connecting)
* [The window](#the-window)
* [Presets](#presets)
* [Signal chain](#signal-chain)
* [Block detail and knobs](#block-detail-and-knobs)
* [Saving](#saving)
* [Tuner, looper and drums](#tuner-looper-and-drums)
* [Device and system settings](#device-and-system-settings)
* [Importing a NAM model](#importing-a-nam-model)
* [Importing an IR](#importing-an-ir)
* [Renaming a user slot](#renaming-a-user-slot)
* [Activity log](#activity-log)
* [Listen only](#listen-only)
* [Replaying a capture](#replaying-a-capture)
* [Keyboard shortcuts](#keyboard-shortcuts)
* [Command line](#command-line)
* [Troubleshooting](#troubleshooting)

## Connecting

Plug the pedal in over USB and start the app, in either order. It finds the pedal by its MIDI port name ("JME
STOMPBOX") and connects by itself, and reconnects if you unplug and replug it. No driver is needed on any OS.

### Wireless (Bluetooth)

With the pedal's **Wireless** setting on, the app also finds it over Bluetooth, with no pairing: it shows up in the
port list as **JME STOMPBOX (Bluetooth)** and the status shows a Bluetooth sign. Everything works as over USB:
editing, presets, saving, settings, looper, drum and tuner. The differences:

* The startup sync takes about 20 seconds instead of under one, since every reply takes about 0.1 s over Bluetooth.
* Knob drags are sent at most every 15 ms, and only the latest value of a drag is kept when the link is busy.
* NAM and IR imports work (the official app doesn't offer them over Bluetooth), but take about 10 seconds to
  upload instead of under one. The dialog says done when the upload is queued, so leave the pedal connected for
  those seconds.
* USB wins: if you plug the cable in while connected over Bluetooth, the app moves to USB.
* On macOS the app asks for Bluetooth permission the first time. If you refuse (or Bluetooth is off) it carries on
  with USB only.

On connect the app reads everything the official app reads: firmware and product name, system settings, user
NAM/IR slot names, all 99 presets and the live edit buffer. The status shows *Reading the pedal…* with a progress
bar; over USB it takes well under a second.

The top bar holds the controls:

| Control | What it does |
|---|---|
| Status | *Searching for pedal…*, *Reading the pedal…*, *Connected V1.1.1* or *Replaying · 40%*. Its **In** and **Out** lights blink for every message received from and sent to the pedal |
| Port list | All MIDI input ports and the pedal over Bluetooth. Picking one connects to it; *Not connected* disconnects (and stops auto-connect until you pick a port again) |
| ⟳ | Re-read presets and state from the pedal (when connected) |
| **Listen only** | Never send anything to the pedal ([below](#listen-only)) |
| **Replay capture…** / **Stop replay** | Play back a recorded session instead of a pedal ([below](#replaying-a-capture)) |
| **Activity** | Show or hide the activity log (also Ctrl+L, ⌘L on macOS) |

In a narrow window the buttons show only their icons; hover one for its name.

> **Windows:** a MIDI port can be open in only one program at a time. Quit the official Stomp Manager first.

## The window

* **Left:** the preset list, P01–P99.
* **Middle:** the active preset (number, name, chips, volume), the signal chain, the detail of one block, then the
  tuner, looper and drum cards, then device info, system settings and user slots.
* **Right** (hidden until you open it with **Activity** or Ctrl+L): the activity log of everything that happens on
  the pedal. In a window narrower than about 1000 px it slides over the page instead.

Anything that changes on the pedal glows briefly where it is shown: the preset header, the block in the chain, the
knob, the log row.

## Presets

The list shows every preset name as read from the pedal. The active preset is highlighted and scrolled into view; an
amber dot next to it means its live sound has **unsaved edits**.

* **Click** a preset to select it on the pedal. Unsaved edits to the previous preset are discarded (the log says so),
  exactly as on the pedal itself.
* **Right-click** a preset to save the current sound there ([Saving](#saving)).
* **Search** above the list (Ctrl+F, ⌘F on macOS) filters by name or slot (`lead`, `5`, `P05`). Enter selects the
  first match, Esc clears the search.

The header shows the preset number, the name, and chips:

| Chip | Meaning |
|---|---|
| `● Edited` | The live sound differs from what is stored in this slot |
| `Saved` | The live sound is what is stored |
| `3/8 blocks on` | How many effect blocks are switched on |
| `Pedal shows Amp` | The page the pedal's display is on |

**Volume** on the right is the preset volume: click or drag along it, scroll over it, or use the arrow keys.

If the app connected while the pedal had unsaved edits, it cannot tell which slot is active (the pedal has no way to
report it) and the header says *Waiting for the pedal…* until the next preset change.

## Signal chain

All blocks in the active preset's chain order: Gate, then the seven effect blocks in the preset's order, then Looper
and Drum. Each shows the model and On/Off; switched-on blocks are coloured.

* **Click a block** to open it in the detail view below. This also makes the pedal's display jump to that block's
  page, as the official app does.
* **Click the power light** in a block's top-left corner to switch it on or off.
* **Click the open block again** to go back to following the pedal.

## Block detail and knobs

The detail card shows one block. By default it **follows the pedal**: whatever you touch on the pedal (or the pedal
page you open) is shown. Once you click a block in the chain it is **pinned**; **Follow pedal** in the card's header
returns to following.

For effect blocks:

* The **model name** (with its number, `#08`) opens the list of the block's models, with user NAM/IR slot names for
  the AMP and CAB user models.
* The **On / Bypassed** button switches the block on or off.
* **Knobs:** drag up or down, or scroll over them; hold **Shift** while dragging for fine steps; **double-click** a
  knob to set it to the middle (50; 0 on centred knobs such as EQ bands, which show ±). Arrow keys and Page Up/Down
  work once a knob has focus.
* **Choice parameters** (CAB mic and position, the Boost switch, some modulation shapes) show every option; click
  one.

Every change is sent at once as a live MIDI message, so you hear it right away, like turning the pedal's own knobs.
It is not stored until you [save](#saving).

The Looper and Drum blocks open their controls ([below](#tuner-looper-and-drums)); the Drum one also has the pattern
picker.

## Saving

When you can edit, the preset header has two buttons:

* **Save** stores the live sound in the active slot under its current name. It is highlighted when there are unsaved
  edits.
* **Save as…** opens a dialog to change the **name** (up to 16 characters, plain ASCII; other characters become `?`)
  and the **slot**. Saving to another slot warns which preset it overwrites. Right-clicking a preset in the list
  opens the same dialog with that slot chosen. Enter in the name field saves.

Saving sends the exact sequence the official app sends (name, preset record, then selecting the saved slot), and the
saved slot becomes active.

> Preset and settings **writes** match the official app byte for byte but have not yet been run on hardware from this
> app. Back up your presets before relying on them.

## Tuner, looper and drums

**Tuner:** switch the tuner on at the pedal. The card shows the note and how many cents sharp or flat, coloured by how
close it is (green within ±5 cents). The pedal only reports pitch while its tuner is on.

**Looper:** state (*Empty*, *Recording*, *Playing*, *Overdubbing*, *Stopped*) and whether an overdub layer exists.
Buttons:

| Button | Action |
|---|---|
| Rec / Play / Dub | The pedal's main looper switch: start recording, close the loop and play, overdub, stop overdubbing |
| ■ | Stop |
| ↺ | Undo / redo the last overdub |
| 🗑 | Clear the loop |

Below them: record and playback levels, **Sync** on/off and **Mode** NR / AR.

**Drums:** pattern name, BPM, play/stop and level. Open the Drum block in the chain to pick a pattern (styles Pop to
Metro, ten patterns each).

## Device and system settings

The device card shows the product, firmware and app versions, and the pedal's global settings once read:

* **Input trim** (−10 to +10 dB)
* **Wireless** and **Loopback** (USB audio loopback)
* **Global EQ**: bass, mid, treble (±). The pedal does not report global EQ changes made on the pedal itself over USB,
  so this can be out of date until the next re-sync (⟳).
* **User slots**: the names of the 10 user AMP (NAM) and 10 user CAB (IR) slots. Hover a slot for its import and
  rename buttons; **Import NAM model…** and **Import IR…** below the list pick the first empty slot.

Settings are written whole each time you change one, starting from the record the pedal sent, so values the app does
not understand are kept.

## Importing a NAM model

**Import NAM model…** (under the user slots, or the upload button on a user AMP slot) converts a `.nam` file and
uploads it to a user AMP slot, named after the file, like the official app does. Choose the file, the slot (it warns
before replacing one in use) and the conversion:

* **Optimised** (default, about a minute): open-stomp fits the pedal's two filters through a simulation of the pedal
  until it sounds like the model. On hardware this is much closer to the real model than the official app's
  conversion ([results](https://jackmechem.github.io/open-stomp-lib/analysis/results.html)).
* **Official app's algorithm** (a few seconds): the same conversion as the official Stomp Manager.

**Normalise the level** (on by default) plays every model at the same volume, like NAM plugins do, whatever level it
was captured at. Without it, imports keep the model's own level, as the official app's do, which is often quiet. It
only changes the volume, not the tone.

The conversion runs in the background with a progress bar and can be cancelled; the slot only changes once it has
finished. You can close the dialog meanwhile: the result also appears in the activity log. You need to be connected
with Listen only off. The model is selected on the AMP block as **USER AMP NN**.

## Importing an IR

**Import IR…** (under the user slots, or the upload button on a user IR slot) uploads an impulse response to a user
IR slot, named after the file (first 16 characters) like the official app does. Any WAV file works: 16/24/32-bit or
float, any sample rate (resampled to 48 kHz), the first channel of a stereo file. Two conversions:

* **Optimised** (default): the pedal only plays the first 384 samples (8 ms) of an IR, so open-stomp fits those 384
  samples to the whole IR. Much closer on the pedal, especially for IRs with a long tail or a delayed start (which
  the official conversion leaves 35–45 dB too quiet).
* **Official app's algorithm**: exactly what the official Stomp Manager sends.

Select it on the CAB block as **USER IR NN**.

## Renaming a user slot

Hover a user AMP or IR slot under **User slots** and click its pencil. Only the name changes; the slot's sound stays
exactly as it is. Names can be up to 16 characters, ASCII only. You need to be connected with Listen only off. The
official app can't do this.

## Activity log

Every message to or from the pedal, as a readable line: time, **Pedal** (from the pedal) or **Host** (sent by this
app, or by the official app in a replay), category, what changed and the new value, e.g.
`Pedal  Amp  Mar J800 · GAIN  51 → 64 ×4`.

* **Knob sweeps merge**: consecutive changes to the same control within 2.5 s become one row showing *from → to* and
  how many messages it took.
* **Filter chips** show or hide categories: Presets, Effects, Looper, Drum, Tuner, Device, **SysEx** (request/reply
  plumbing, hidden by default), **Redundant** (values the pedal repeats after a preset change that were already
  known, hidden by default), and the direction (*From pedal*, *From host*).
* **Search** filters by text, e.g. `gain`, `looper`, `P05`.
* Header buttons: follow new events (scrolls along at the bottom; scrolling up stops it), show the raw MIDI bytes,
  copy the visible log as text, clear it, hide the panel.
* Hover a row for its raw bytes. You can also select and copy text from the rows.

The log keeps the latest 50,000 rows.

## Listen only

**Listen only** makes the app completely passive: no startup reads, no edits, nothing is sent. Use it to watch what
the pedal (or another editor) does without any interference. Turning it off re-reads everything from the pedal.

The app also falls back to listening when it could open the pedal's input but not its output; the status says
*Listening (no output)*.

## Replaying a capture

**Replay capture…** plays a recorded session (a capture log, see the
[capture methodology](https://github.com/JackMechem/open-stomp-lib/blob/main/docs/07-capture-methodology.md)) as if a
pedal were connected: the whole UI follows it, which is handy for trying the app without a pedal. Choose the file
(it starts on `captures/logs/01-main-session.log` when run from the repo) and a speed (1× to 16×), then **Replay**.
Gaps longer than 1.5 s are shortened. `# MARK` notes in the log appear as purple rows. Editing is off while replaying.

## Keyboard shortcuts

| Keys | Action |
|---|---|
| Ctrl+L (⌘L) | Show or hide the activity log |
| Ctrl+F (⌘F) | Search the presets; Enter selects the first match, Esc clears |
| Arrow keys, Page Up/Down | Turn the focused knob or slider |
| Esc | Close a dialog |

## Command line

```
open-stomp-manager [--replay <capture.log>] [--speed <x>] [--version] [--help]
```

| Option | |
|---|---|
| `--replay <file>` | Start replaying a capture log right away (without connecting to the pedal) |
| `--speed <x>` | Replay speed (default 4) |
| `-V`, `--version` | Print the version and git commit |
| `-h`, `--help` | Usage |

## Troubleshooting

| Symptom | Try |
|---|---|
| *Searching for pedal…* forever | Check the USB cable and that the pedal is on. On Linux, `amidi -l` should list "JME STOMPBOX". Pick the port by hand in the port list. For Bluetooth, check the pedal's Wireless setting and that the app has Bluetooth permission (macOS: System Settings → Privacy & Security → Bluetooth) |
| Connected but nothing can be edited | *Listen only* is on, or the status says *Listening (no output)* (Windows: quit the official app) |
| *No reply from pedal to …* in the log | A read timed out (1 s). Usually harmless; ⟳ re-reads everything |
| The header says *Waiting for the pedal…* after syncing | The pedal had unsaved edits when the app connected; change preset once |
| Global EQ looks wrong | It was changed on the pedal; press ⟳ |
| Linux: the window stays blank or flickers | Usually NVIDIA with Wayland: start the app with `WEBKIT_DISABLE_DMABUF_RENDERER=1` |
| Windows: the portable app doesn't start | It needs Microsoft's WebView2 runtime; the `-setup.exe` installs it |
| macOS says the app is from an unidentified developer | Right-click it and choose *Open* (see the [README](../README.md#install)) |
