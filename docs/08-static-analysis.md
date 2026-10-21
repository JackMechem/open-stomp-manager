# 08 — Static analysis (Stomp Manager v1.0.2, macOS)

> **Read this first: corrections from the live USB captures.** This report was written from the binary alone,
> before any capture. Where it disagrees with docs 02–05, the captures win:
>
> * `dir` values: 0 = read, 1 = write, **2 = the pedal's read reply**, **3 = write ack** (not "V2 getter"). `0x7E` in the
>   cmd position is a notification.
> * The `[total<<4|index]` fragmentation prefix was **never observed** over USB. Large data uses the chunked
>   `0x58`/`0x10` uploads instead.
> * Newly identified msgIds: `0x10` IR upload data, `0x58` user AMP (NAM) slot (info/data/delete), `0x60` short frame
>   = product name, `0x00` short frame = version.
> * Large records are 8-bit structs packed into a 7-bit little-endian bitstream ([03](03-sysex-protocol.md)).
> * The Dual Stomp Lite preset layout is **not** `NBA150PPresetStruct`; see [04](04-data-structures.md).
> * The pedal also takes plain CC/PC for live control, on **channel 2** host→pedal ([02](02-midi-cc-reference.md)).
> * The effect table in §5 was cross-checked against the app UI and is correct.
> * Paths in this report refer to `analysis/` in the repo. The large extracted binaries are not committed.


Target: `/Applications/Stomp Manager.app` (bundle `com.jiyou.stompManager`, CFBundleShortVersionString 1.0.2, build 1).
Device: SORAN Dual Stomp Lite (USB VID 0x3962, PID 0x2000, "JME STOMPBOX").
Everything here is static analysis. Nothing was run against a device. Each item is marked:

- **[CONFIRMED]**: read directly from the binary, its object pool or the disassembly.
- **[STRONG]**: inferred from code flow I traced by hand. Very likely right, but no device capture backs it yet.
- **[HYPOTHESIS]**: a plausible reading that needs a USB capture to check.

Paths below are relative to `~/Desktop/stompManagerRE/analysis/` unless they are absolute.

---

## 0. Executive summary (protocol-relevant)

1. **Transport.** USB uses USB-MIDI SysEx through CoreMIDI, via `flutter_midi_command` (channel `plugins.invisiblewrench.com/flutter_midi_command`). BLE uses BLE-MIDI: service **`03b80e5a-ede8-4b33-a751-6ce34ec4c700`**, characteristic **`7772e5db-3868-4112-a1a9-f2669d106bf3`**, through `universal_ble` (pigeon `writeValue`/`setNotifiable`/`requestMtu`). [CONFIRMED]
2. **Device discovery.** The app matches port/device names against the regex list `[".*JME.*", ".*SORAN AMPEY.*"]`. [CONFIRMED]
3. **Current frame format ("NEP").** The class is `NEPStruct(data, vidBytes, msgId, uid, dir)`. Field layout from the constructor at code 0x4f610:
   ```
   F0 | 00 21 7A | uid | msgId | dir | payload... | F7
   ```
   - `vidBytes` = `[0x00, 0x21, 0x7A]` [CONFIRMED: literal at code 0x4f7d0 and 0x53ee0].
   - `uid` is always **0x70** in both request builders (0x3a3bfc, 0x3aaa64) [CONFIRMED constant; mapping arg→field STRONG].
   - `msgId` and `dir` are 1-byte fields [STRONG].
   - The wire order equals the field-declaration order [STRONG].
4. **Legacy/alternate header.** A second header constant, `[0x43, 0x58, 0x70, 0x7E]`, is used for incoming messages. A short `[0x43, 0x58]` form also exists. The error string is "**SyxCherubID not equals**". The receive path tries three headers in turn: the default, then `43 58`, then `00 21 7A` (function 0x53e28). [CONFIRMED constants; role STRONG]
5. **Command table (msgId, dir).** Recovered from call sites of the request builder 0x3a3bfc(name, msgId, dir). Full list in §3.4.
   - dir=0 means get, dir=1 means set. dir=2 is used by the init/"V2" getters.
   - Main IDs: tuner **111 (0x6F)**, loop 24, drum 25, system-set 20, current-state/device-state 21, preset 11, curset 12, preset-link/module-link 13, preset-name 18, IR CRC+name 9, delete IR 22, bluetooth params 27.
6. **Payload schema.** The app contains a full JSON-like struct dictionary built in code (§4). It covers presets (11 modules, each `[model, params...]`), IR names (with a `MidiUint28` CRC), system settings, tuner, drum, loop, bluetooth/speaker EQ, and a MIDI-CC map. There are two families: `10xx` (codename "NBA150P", single-chain) and **`20xx` (dual-chain: nr1/nr2, preefx1/2, amp, cab, eq1/2, postefx1/2, rvb1/2)**, which matches the Dual Stomp.
7. **Field types.** `Uint8`, `Uint16`, `MidiUint14` (14-bit value in two 7-bit bytes), `MidiUint28` (28-bit value in four 7-bit bytes), `Char[n]`, `Array`, `struct`, and bit-fields (`EffectHead`: index 6 bits + switch 1 bit). [CONFIRMED names; byte order HYPOTHESIS]
8. **Multi-packet reassembly.** Incoming payload byte 0 carries `hi-nibble = total packets`, `lo-nibble = packet index`; a new message starts at index 0 (code 0x53d80). [STRONG] Large transfers (firmware/IR) go through a "multi package" sender (`get_fwMultiPackages`, builder 0x3aaa64). BLE writes appear to be chunked at 128 bytes. [HYPOTHESIS]
9. **IR pipeline.** User audio (wav/aiff/flac/ogg/CoreAudio formats) is converted by the JUCE-based `nux_plugins` into a 48 kHz WAV (`*_ir_48.wav`). Wrong-rate input raises "error: sample rate error". The device identifies IRs by CRC (`getAllIRCrcAndName` / `setIRCrcAndName`). [CONFIRMED strings]
10. **Amp captures.** "imagelib" is a disguised **Neural Amp Modeler (NAM)** loader: NeuralAmpModelerCore, WaveNet/LSTM/ConvNet/A2-fast/SlimmableContainer, plus nlohmann::json. The UI calls the AMP slot "Amp (Nfile)". The provided "Marshall JCM800" zip is a `.nam` file, not an IR.
11. **Firmware.** NXP **i.MX RT10xx** (MCUXpresso `fsl_*` drivers: SAI, SEMC, FlexSPI, eDMA, LPUART, ADC_ETC). The container is `NUX DFU\x02` with TEXT/BINA/EXTR sections. BLE is an external module driven by AT commands over UART. Firmware updates use a separate USB-HID DFU tool, not the Flutter app.

---

## 1. Build facts

| Item | Value |
|---|---|
| Flutter engine | `0cd610717bde95fd88343c64f81c11ba4e5c0010` (FlutterMacOS Info.plist) |
| Dart SDK | **3.12.2 (stable)**, Tue Jun 9 2026, macos_arm64/x64 |
| Snapshot hash | `ace654289f5abc240509fc941453ebc5` |
| Snapshot flags | `product no-code_comments dwarf_stack_traces_mode dedup_instructions ... arm64 macos no-compressed-pointers` |
| Obfuscation | **Yes** (`--obfuscate`): class/method names are 2–3-char tokens such as `jGa`, `Nz`, `MFa`, `yGa`. String literals and `toString()` texts survive. |
| Build path leak | `file:///Users/nux/Desktop/project/flutter/stomp_manager/...` (developer account "nux") |
| App Info.plist | `NSBluetoothAlwaysUsageDescription` = "Using Bluetooth to connect SORAN devices", LSMinimumSystemVersion 10.15, Xcode 26.6 / SDK macOS 26.5 |
| DMG `SORAN_Stomp_Manager_V1.0.2.1_macOS.dmg` | Contains only `Stomp Manager.app`, `.background/bg.jpg` and an Applications alias. `diff -rq` against the installed app found **no differences**. |

Mach-O layout of `App` (arm64 slice): `__TEXT` vmaddr 0 == fileoff 0.
- `_kDartVmSnapshotInstructions` @0x4000
- `_kDartIsolateSnapshotInstructions` @0x10000
- `_kDartVmSnapshotData` @0x410000
- `_kDartIsolateSnapshotData` @0x41c000

### 1.1 Blutter
- I got upstream blutter running against the macOS Mach-O with a small patch (`scripts/blutter_macos_macho.patch`). The patch adds a `macos` target with `DART_TARGET_OS_MACOS` and no compressed pointers, a Mach-O LC_SYMTAB loader in `ElfHelper.cpp`, an always-defined `CSREG_DART_HEAP`, and a guard against bogus signature pointers.
- Dart VM 3.12.2 built in about 2 minutes. Sources are in `tools_src/blutter`.
- **Result:** the object pool and object dumps are complete and very useful:
  - `extracted/blutter_out/pp.txt` (copied to `blutter_object_pool.txt`)
  - `objs.txt`
- **Limitation:** `asm/` covers only about 1.3k functions. dwarf-stack-traces mode drops most Code objects, so blutter cannot find most functions.
- **Workaround:** I disassembled the whole `__text` with llvm-objdump and annotated every PP (x27) load with its object-pool entry, and every `bl` with the names blutter knows (`scripts/annotate_disasm.py`). Output: `extracted/app_snapshot/annotated_disasm.txt` (1.07M lines). Every protocol finding in §3 comes from this file.
- Dart arm64 conventions used when reading it:
  - Registers: x27 = PP, x26 = THR, x22 = NULL, x15 = Dart SP, x28 = HEAP_BITS.
  - Smis are value<<1, so `mov x16,#0x86` stored into an array is Smi 0x43.
  - Unboxed int parameters are passed raw in x3/x5.

---

## 2. Dart package / library structure

Obfuscation strips almost all `package:` URIs. Only 8 survive (`dart_package_uris.txt`): flutter (plugin registrant, services, widgets), ffi, objective_c.

**Dependencies from NOTICES.Z** (gzip; full list `dependencies_from_NOTICES.txt`, 250 entries). App-relevant ones:

- **NUX/vendor-private:**
  - `nux_helper`, `nux_plugins`, `nux_nep` (very likely the NEP protocol package, consistent with the `NEPStruct` class)
  - `nux_win_extra_dll`
  - `device_manager`, `midi`
  - `ffx_spd`, `fconsole`
  - `imagelib` (the NAM loader)
- **Transport:**
  - `flutter_midi_command` (+ `_linux`, `_windows`, `_platform_interface`)
  - `universal_ble`, `flutter_web_bluetooth`, `bluez` (Linux BLE), `dbus`
- **Misc:** `mutex`, `crypto`, `archive`, `file_picker`, `file_selector`, `shared_preferences`, `share_plus`, `go_router`, `provider`, `easy_refresh`, `flutter_smart_dialog`, `permission_handler`, `window_manager`, `screen_retriever`, `macos_window_utils`, `flutter_dynamic_icon_plus`. The app icons include `app_icon_lekato.png`, `app_icon_jamelo.png` and `app_icon_strich.png`, and the strings "LEKATO", "JAMELO", "STRICH" appear: this is a white-label/OEM build.
- `flutter_midi_command_linux` and `bluez` are both in the dependency list. The vendor already builds this app for Linux internally.

**Surviving class names** (from literals and `toString`, not obfuscated):
- `NEPStruct`, `SyxReqCmdStruct`, `NuxSendItem`, `NuxSenderResult`
- `XDeviceInfo`, `XDeviceSendResult`, `XDeviceType.`, `EffectDeviceDataType.`, `SendDataState.`, `EffectMenuModel`, `EffectModelType.`
- `BleDevice`, `BleService`, `BleCharacteristic`
- Struct schema names: `NBA150P*Struct`, `SMidiParaCC`, `EffectHead`, `NBA150PHeaders`
- Dialogs: `PresetImportListDialog`, `LoopDialog`, `DrumDialog`, `TunerDialog`, `FactoryResetDialog`, `SelectSerialPortDialog`

`toString` field lists (useful for naming fields in Rust):
```
NEPStruct(data: , vidBytes: , msgId:, uid: , dir:)
SyxReqCmdStruct(msgID: , dir:, buffer: )
NuxSendItem(name: , timeout: , data: , check: , checking: , timeoutCount: )
NuxSenderResult(isSuccess: , resMap: , resUnit8List: )
XDeviceInfo(type: , id: , connected: , obj: )
XDeviceSendResult(type: , fDevsList: , usbDevsList: , connecting: )
BleDevice: deviceId: , name: , rssi: , paired: , services: , isSystemDevice: , manufacturerDataList:
EffectMenuModel(type: ...)
status:, tunerMode:, tunerPitch:, tunerMute:, xmode:      (tuner state)
state:,note:,number:,cent:                                (tuner reading)
{loopVol:;loopState:;}                                    (NUXSystem)
```

---

## 3. MIDI / SysEx protocol

### 3.1 Transport layer [CONFIRMED]
- USB is driven through `flutter_midi_command`. Method-channel calls seen in the pool: `getDevices`, `connectToDevice`, `disconnectDevice`, `sendData` (map `{data, timestamp, deviceId}`). Receive goes through EventChannel `.../rx_channel` and setup through `.../setup_channel`. Device map keys: `inputs`, `outputs`, `ports`, `connected`, `type: usb|ble|edrum`. Log strings: `usbmidi connectDevice error`, `MIDINotificationMessageID(rawValue: 1)`, `setup changed`.
- The macOS `flutter_midi_command` binary has CoreMIDI only (no CoreBluetooth). It handles SysEx (`SYSEX`, `sysExBuffer`, `MIDIPacketList` of 256 bytes).
- BLE goes through `universal_ble` pigeon channels: `startScan`, `connect`, `discoverServices`, `requestMtu`, `setNotifiable`, `writeValue`, `onValueChanged`. UUIDs `03b80e5a-...c700` (MIDI service) and `7772e5db-...6bf3` (MIDI I/O characteristic). Short UUIDs are expanded with `-0000-1000-8000-00805f9b34fb`.
- Because BLE bypasses `flutter_midi_command` on macOS, BLE-MIDI framing (header/timestamp bytes) must be done in Dart. [HYPOTHESIS: irrelevant for USB]
- Log strings: `mtu `, `connectDevice3 success`, `characteristic not found`, `蓝牙断开连接` ("Bluetooth disconnected").
- Windows uses a serial-port path as well: `SelectSerialPortDialog`, `serialPortListInit error`, plus device-name tables `CON/PRN/AUX/COM1..9/LPT1..9`.
- Device-name filter: `XFa([".*JME.*", ".*SORAN AMPEY.*"])` (code ≈0x2640xx). The USB product string "JME STOMPBOX" matches `.*JME.*`.

### 3.2 NEP frame [CONFIRMED constants / STRONG layout]

Constructor `MFa` (NEPStruct) at code **0x4f610**. Named params (obfuscated names in parentheses):
`data`, `BNb` (= dir), `Bmf` (= uid), `INb` (= msgId), plus positional `vidBytes`.

| Field offset | Field | Wire bytes | Source |
|---|---|---|---|
| +0x2f | start | `F0` | `yGa(value=0xF0, len=1)` @0x4f790 |
| +0x37 | vidBytes | `00 21 7A` | `pGa([0x00,0x21,0x7A])` @0x4f7d0 (Smis 0,0x42,0xF4) |
| +0x3f | uid (Bmf) | 1 byte, **0x70** in all requests | builders 0x3a3ec4 / 0x3aab40 push Smi 0xE0 |
| +0x47 | msgId (INb) | 1 byte | builder arg x3 |
| +0x4f | dir (BNb) | 1 byte: 0=get, 1=set, 2=see below | builder arg x5 |
| +0x57 | data | payload | `LNb` named arg → 0x522bc serialiser |
| +0x5f | end | `F7` | `yGa(0xF7,1)` @0x4f978 |
| +0x67 | (int) | total/data length; printed as "vidBytes" in toString | |

- **Parse path.** 0x4f33c(this, rawBytes, headerList) works as follows:
  1. Strip the leading F0 (`sublist(1)`).
  2. Compare `header[1..]` against the input (`header[0]` is not compared).
  3. Split off the header.
  4. Build an `MFa`, then check that the struct length equals the input length.
- **Receive dispatch** at 0x4d304: `if (bytes[0] == 0xF0) handleSysex(...)`. 0x4d350 builds `[0x43,0x58,0x70,0x7E]` and calls 0x4fbd4. 0x53e28 tries the default header, then `[0x43,0x58]`, then `[0x00,0x21,0x7A]`.
- **Error strings:** `length not match`, `SyxCherubID not equals`, `timeout`, `no connect`, ` no device connect`, `removeDuplicate`, `no lock to release` (requests are serialised with a mutex), `indata: dir:`.
- **"Syx" (Cherub) header.** `43 58 70 7E` is a second header family. The name `SyxReqCmdStruct(msgID, dir, buffer)` matches it. Note the shared `0x70` (product/uid byte) in both families. Function 0x555bc builds `[F0] + bytes + [00, F7]`: this is a fixed request form that appends a literal `00` before F7. [HYPOTHESIS: a legacy Cherub/NUX "Syx" request whose trailing 0 is a checksum/terminator placeholder]
- **Payload 7-bit safety.** The typed `MidiUint14`/`MidiUint28` fields exist because SysEx data bytes must be ≤0x7F. Plain `Uint8` fields are presumably ≤127. [HYPOTHESIS]

### 3.3 Fragmentation [STRONG]
- Reassembly at 0x53d80: `b = payload[0]; if ((b & 0x0F) == 0) { total = b >> 4; count = 0; buffer.clear(); }` Then each fragment's `payload[1..]` is appended. When `count == total`, the buffer is delivered.
- So multi-part replies carry a 1-byte `[total<<4 | index]` prefix.
- The sender side for large data is class `JFa` with fields `RCb`, `data`, `oQe`, `xub` (call at 0x3a9ea8), used with `get_fwMultiPackages`.

### 3.4 Command table (from call sites of builder `0x3a3bfc(name, msgId, dir)`)
`nep_command_calls.txt` (script `scripts/cmd_table.py`). [CONFIRMED immediates]

| msgId (dec/hex) | dir | API name (Dart string) | Response stored under / notes |
|---|---|---|---|
| 9 / 0x09 | 0 | `getAllIRCrcAndName` | IR list (struct 1009 `NBA150PIrNameStruct`: index, crc MidiUint28, name Char[32]) |
| 9 / 0x09 | 1 | `setIRCrcAndName` | |
| 11 / 0x0B | 2 | (getPresetV2 path, fn 0x55260) | `preset` / `curPreset.` (struct 1008/2008) |
| 12 / 0x0C | 2 | `getCursetV2` (fn 0x54360, named arg `drb`=6) | `tuner`/`curset` |
| 13 / 0x0D | 2 | `getModuleLink` (fn 0x54650) | `link` (struct 1010 `NBA150PLinkStruct`) |
| 13 / 0x0D | 0 / 1 | `getCurrentPresetLink` | |
| 18 / 0x12 | 1 | `setPresetName` | |
| 20 / 0x14 | 2 | `getSystemSetV2` | `system.` (struct 1005 `SysSet`) |
| 20 / 0x14 | 0 / 1 | `getSystemSet` (also used for set) | |
| 21 / 0x15 | 2 | `getCurStateV2` | `curState.` (struct 1002) |
| 21 / 0x15 | 0 | `getDeviceState` | |
| 22 / 0x16 | 1 | `deleteIR` | |
| 24 / 0x18 | 0 / 1 | `getLoop` / `setLoop` | struct 1004 |
| 25 / 0x19 | 2 / 0 / 1 | `getDrum` / `setDrum` | struct 1003 |
| 27 / 0x1B | 0 | (getBluetoothParams, fn 0x54e8c) | `curBluetooth` / `bluetooth` (struct 1001) |
| 111 / 0x6F | 2 / 0 / 1 | `getTuner` / `setTuner` | `tuner.` (struct 1007) |

- Other API names with no msgId recovered yet. They go through other paths: name-based dispatch 0x527d4/0x525c4, the multi-packet builder 0x3aaa64, or packets built elsewhere.
  - `getFwV2` (timeout 1000 ms), `get_fwMultiPackages`, `getBlackBoxGen`, `getVersion`, `getNUXVersion`, `getEdrum`
  - `getAllNamInfo`, `loadNamFile`, `clearLoadNam`, `coverSingleIr`, `getPresetV2`, `getBluetoothParams`
- `dir=2` is used only by the startup/"V2" getters in class `Nz`/`jGa`. One reading: "request full struct" versus dir 0 "request" and dir 1 "write". [HYPOTHESIS]
- MIDI CC/PC path: `midiCCDataChanged`, `onProgramChange `, `onChangeValue cc: value:`, `cc.auxeq`, `cc_`, `pc_`. Struct `SMidiParaCC` (1011/2011) maps each parameter to a CC number. So the device also accepts plain CC/PC for parameter changes. [STRONG]

### 3.5 Checksums
- No CRC-32/CRC-16 tables or obvious checksum loop were found in the NEP send path. The frame appears to carry no checksum. [HYPOTHESIS]
- IRs are identified by a `crc` field of type `MidiUint28`. The algorithm is not identified. `crypto` (Dart) is a dependency, and `xxhash` appears only in the engine notices. A capture of `getAllIRCrcAndName` against a known WAV would settle it.

---

## 4. Device data model (struct dictionary)

Built in code at 0x44e08 (class `TFa`) as `{"version":1, "build": { "<id>": {"id","name","params":[{type,name,unit,size,bits,id}]} } }`.
- Full dump: `device_struct_schema.txt` / `.json`
- Raw token stream: `struct_schema_tokens.txt`

[CONFIRMED structure; semantics of individual fields mostly HYPOTHESIS]

```
1001 NBA150PBluetoothStruct : volume, hz_31, hz_62, hz_125, hz_250, hz_500, hz_1K, hz_2K, hz_4K, hz_8K, hz_16K, gain, phase, mute   (all Uint8)
1002 NBA150PCurstateStruct  : preset, auxeq, micNGEnable, micNGSens, micNGDecay, spkeq
1003 NBA150PDrumStruct      : drum_command, drumtype, drum_vol, drumeqL, drumeqM, drumeqH, tmpo:MidiUint14
1004 NBA150PLoopStruct      : level, state, nrar
1005 NBA150PSysSetStruct    : micmute, outmodeeq1..3, display1..3, expset1..4, preeq1..4, usbrount1..4, presetrange, micvolume, midiChan
1006 NBA150PSpeakerStruct   : volume, hz_31..hz_16K, gain
1007 NBA150PTunerIDStruct   : status, mode, pitch, mute
1008 NBA150PPresetStruct    : head:struct1013(size 11), wah[3], cmp[5], efx[7], amp[9], eq[13], ng[5], mod[7], dly[9], rvb[5], cab[7], sr[3],
                              master, delayTimeFlag, bpmH, bpmL, bitCtrl, link[11], userName[16], swsel[3], pdsel1, pdsel2, version,
                              scene1:Uint16, scene2:Uint16, scene3:Uint16
1009 NBA150PIrNameStruct    : index, crc:MidiUint28, name:Char[32]
1010 NBA150PLinkStruct      : index, data[9]
1011 SMidiParaCC            : head:struct1014, tuner1[2], cmp[4], efx[6], amp[8], eq[12], ng[4], mod[6], dly[8], rvb[4], cab[6], tuner2[2],
                              master, mselect, pedal, scene, drumenable, drumtype, drumlevel, looplevel, loopstate, auxeq, presetrange,
                              micvolume, micmute, usbrount[4], aux_mute, aux_phase, aux_band[12], drumeq[3], looparnr, micnr[3], spkeq, spk_band[12]
1012 EffectHead             : index:Uint8 bits=6, switch:Uint8 bits=1
1013 NBA150PHeaders         : wah,cmp,efx,amp,eq,ng,mod,dly,rvb,cab,sr  (each struct 1012 -> 1 byte)
1014 NBA150P MIDI CC Headers: wah,cmp,efx,amp,eq,ng,mod,dly,rvb,cab,sr (Uint8)
2008 NBA150PPresetStruct    : identical layout to 1008
2011 SMidiParaCC (dual)     : head:struct2014, nr1[4], nr2[4], preefx1[6], preefx2[6], amp[8], cab[6], eq1[12], eq2[12], postefx1[8], postefx2[8],
                              rvb1[4], rvb2[4], tuner12[2], tuner34[2], master, mselect, pedal, scene, drumenable, drumtype, drumlevel,
                              looplevel, loopstate, auxeq, presetrange, micvolume, micmute, usbrount[4], aux_mute, aux_phase, aux_band[12],
                              drumeq[3], looparnr, micnr[3], spkeq, spk_band[12]
2014 NBA150P MIDI CC Headers (dual): nr1, nr2, preefx1, preefx2, amp, cab, eq1, eq2, postefx1, postefx2, rvb1, rvb2
```

Observations [HYPOTHESIS]:
- **Preset module arrays look like `[model, p1..pN]`.** Each module array is exactly "1 + max param count" of that category:
  - cab[7] = model + MIC, POSITION, LOW CUT, HIGH CUT, VOLUME (5)
  - eq[13] = model + 12 (10-band EQ has GAIN + 10 bands + VOLUME)
  - rvb[5] = model + 4 (Hall: DECAY, TONE, PRE DELAY, LEVEL)
  - amp[9] = model + 6 knobs (+2 spare)
- **`SMidiParaCC` arrays are one smaller:** one CC number per parameter, no model byte.
- **`head`** (11 × EffectHead) is one byte per module: low 6 bits = model index, bit 6 = on/off.
- **`10xx` versus `20xx`.** The `10xx` set ("NBA150P", single chain: wah/cmp/efx/amp/eq/ng/mod/dly/rvb/cab/sr) is probably a sibling product. The `20xx` CC header (nr1/nr2/preefx1/2/amp/cab/eq1/2/postefx1/2/rvb1/2/tuner12/34) matches the **Dual Stomp** two-channel topology. The app UI module list (§5) matches that dual chain.
- **Pool field names** (`._property_._index_`, `curPreset.link`, `link.data`, `preset.`, `curState.`, `system.`, `tuner.`, `drum.`) show that responses are decoded into nested maps keyed by these struct field names.

---

## 5. Effect / model catalogue

From code 0x255a38–0x264000 (`effect_db_readable.txt`). Each parameter object `TKa` is `{kind, name, default/index double, options[]}`. kind enum `QKa`: 0 = knob, 11 = selector, 7 = switch.

**UI module order and labels.** Strings are tri-lingual zh-CN / zh-HK / en.
- GATE (降噪), OD/FX (过载), AMP "Amp (Nfile)", CAB "Cab (Ir)", EQ, MOD, DEL, REV, LOOP, DRUM.
- Icon set: DRUM, EFX, AMP, IR, MOD, DLY, RVB, LOOP.

| Module | Models (parameters) |
|---|---|
| Gate | Noise Gate (THRESHOLD, DECAY) |
| Comp | RS Comp (SUSTAIN, LEVEL); Red Comp (OUTPUT, SENSITIVITY) |
| OD/FX | Yellow Dist (OUTPUT, Distortion); Boost RC (GAIN, VOLUME, BASS, TREBLE); Boost AC (GAIN, VOLUME, BASS, TREBLE); DS1 (GAIN, TONE, VOLUME); TS9 (DRIVE, TONE, LEVEL); BD2 (LEVEL, TONE, GAIN); Klone (VOLUME, DRIVE, TONE); Ratt2 (DIST, FILTER, VOLUME); Red Drive (DRIVE, TONE, LEVEL); Cruncher (VOLUME, TONE, GAIN); Muff (VOLUME, TONE, SUSTAIN); Boost (GAIN, BOOST[switch], VOLUME); Face Fuzz (GAIN, TONE, OUTPUT); ST Drive (VOLUME, GAIN, FILTER) |
| Amp (32) | Black Twin, Tweed 5F1, Bassguy, Prince Rev, Tweed 5F8, Deluxe Rev, Bdotone CL, Voxy 30HW, Mar J800, Mar J45, Mar Plexi100, Jazz CL, Mes Rector, Mes MKIV Lead, Mes MKV Red, Mes LStar CL, Cornfed M50, ODS-100, Spawn Qrod, Soloist 100, Fryman HB, Fryman DS, PVH 61, Eddie 51, 2Rock J35, Ampug SV Bass, Ampug Port Bass, FR Bass, Mes 400 Bass, Agula DP751, plus "user" (NAM slot). All have GAIN, BASS, MIDDLE, TREBLE, PRESENCE, MASTER. |
| Cab (30) | Twin 212, Tweed 110, Bassguy 410, Prince 112, Tweed 210, Deluxe 112, Bdotone 212, Voxy 212, Mar G12M 412, Mar Silver 212, Mar V30 412, Jazz 212, Mes OS 412, Mes Trad 412, Mes V30 212, Mes EV 112, Cornfed 212, ODS 212, Spawn 412, SLO 412, Fryman 412, Fryman 212, PVH 412, Eddie 412, 2Rock 212, Ampug 810, Ampug 410, FR 210, Mes 410, Agula 810, plus "user" (user IR). Params: MIC {E906, M160, MD421, R121, SM57, SM58, SM7B, U47}, POSITION {EDGE, MIDDLE, CENTER}, LOW CUT(Hz), HIGH CUT(Hz), VOLUME. The user IR slot has LOW CUT, HIGH CUT, VOLUME only. |
| EQ | 6-band EQ (100Hz, 220Hz, 500Hz, 1.2kHz, 2.5kHz, 6.4kHz, in dB); 10-band EQ (GAIN(dB), 31Hz, 62.5Hz, 125Hz, 250Hz, 500Hz, 1kHz, 2kHz, 4kHz, 8kHz, 16kHz, VOLUME(dB)) |
| Mod | Chorus-1 (INTENSITY, DEPTH, RATE); Chorus-2 (RATE, DEPTH); Stereo Chorus (INTENSITY, WIDTH, RATE); Vibrato (RATE, DEPTH); Detune (L, MIX, R); Flanger (LEVEL, RATE, WIDTH, FEEDBACK); Phaser 90 (SPEED); Phaser 100 (INTENSITY, SPEED); Chorus+ (SPEED, WIDTH, MODE{CHORUS,FLANGER}, INTENSITY); Uni Vibe (SPEED, VOLUME, INTENSITY, MODE{CHORUS,VIBE}); Tremolo (RATE, DEPTH); Chorus-3 (RATE, DEPTH, TONE); Harmony (KEY{C..B}, MODE{MINOR,MAJOR}, HARM{-8TH,-6TH,-5TH,-4TH,-3TH,3TH,4TH,5TH,6TH,8TH}, MIX) |
| Delay | Analog Delay, Digital Delay, Tape Delay, Reverse Delay, Pan Delay, Physical Delay (all TIME, FEEDBACK, LEVEL); Mod Delay (TIME, FEEDBACK, MOD, LEVEL) |
| Reverb | Room (DECAY, TONE, LEVEL); Hall (DECAY, TONE, PRE DELAY (ms), LEVEL); Plate (DECAY, LEVEL); Spring (DECAY, LEVEL); Shimmer (LEVEL, DECAY, SHIMMER); Damp (LEVEL, DEPTH) |
| Drum | 10 styles × 10: 01.Pop, 02.Rock, 03.Metal, 04.Disco, 05.Funk, 06.Blues, 07.Hip Hop, 08.Jazz, 09.Latin, 10.Metro (Metro 4/4, 2/4, 3/4, 5/4, 6/8, 7/8, 9/8, 10/8, 11/8, 12/8). Drum Sync, BPM. |
| Looper | Rec / Play / Stop / Clear / Undo / Redo / Overdub; "LOOP SYNC:", "LOOP MODE:" NR/AR |
| Tuner | Modes: Chromatic, Guitar Standard, Guitar Compensated, Bass. Display: Standard/Flowing. A4 reference (Hz). Output: Bypass/Muted. |
| Global | Global EQ ("EQ Group {}", LP/HP/LF/LMF/HMF/HF, FREQ/GAIN/Q), USB audio routing (Normal, Normal-LoopBack, Reamp, Dry/Wet), Input Trim, Energy Saving Mode, Bluetooth settings, Factory reset ("Reset All Presets In Device") |

Model-index order within each category is assumed to follow this declaration order, starting at 0. [HYPOTHESIS] The cutoff-frequency strings "20…1000" and "5000…20000" are log-spaced lookup tables, which suggests LOW CUT/HIGH CUT are sent as table indices.

The parameter-name ordering in the table relies on my reading of the token stream: parameters are constructed before the model object. Models whose order is ambiguous: Damp, Physical Delay. Cross-check against a capture.

---

## 6. flutter_assets

Full list: `flutter_assets_filelist.txt` (111 files). Decoded `AssetManifest.bin` (StandardMessageCodec): `asset_manifest_decoded.json`. Contents:
- **Images:**
  - `assets/image/{appIcon (6), icon (29), loop (28), tuner (35)}`
  - Referenced but not shipped: `assets/image/model_icon/icon_<CAT>_<1-4>.png` (CAT = DRUM, EFX, AMP, IR, MOD, DLY, RVB, LOOP)
- **Fonts:** Carlito-Bold, MaterialIcons, CupertinoIcons.
- **Shaders:** `ink_sparkle.frag`, `stretch_effect.frag`.
- **`assets/translations/{en,zh-CN,zh-HK}.json`:** about 130 UI keys, no protocol content. Most UI strings are inlined in Dart instead.
- **`assets/translations/lang.bin`:**
  - 1,152,516 bytes, entropy 7.9998 bits/byte (encrypted or compressed). Size ≡ 4 mod 16. No known magic.
  - No string referencing `lang.bin` exists in the Dart snapshot, so the loader path is unknown (possibly the obfuscated `JMEL10n` delegate).
  - Newer than the JSONs: Aug 13 versus Apr 20.
  - Hypothesis: an encrypted localisation/model database. Not needed for the protocol, since the model DB and schema are in code.
- `NativeAssetsManifest.json`: only `package:objective_c`. `FontManifest.json`: standard.
- `NOTICES.Z`: gzip (not zlib). Decompressed copy at `extracted/NOTICES.txt`.

---

## 7. Native frameworks

Symbol and string dumps are in `extracted/native/` (`*_nm.txt`, `*_nm_demangled.txt`, `*_defined_symbols_demangled.txt`, `*_strings.txt`).

### nux_plugins (JUCE audio-file utility; not protocol)
- **ObjC `NuxCallbackBridge`:**
  - `+convertAudioToWaveFormat:outputPath:tmpPathChar:`
  - `+convertAudioToWavWithOut:outputPath:`
  - `+loadAudioFileData:`
  - `+registerAudioFileDataCallBackWithBlock:`
  - `+string2wchart:`
- **C++ `JuceUtil`:** `convertAudioToWaveFormat(wchar_t const*, wchar_t const*, wchar_t const*)`, `convertAudioToWavWithOut(float*, wchar_t const*)`, `loadAudioFileData(wchar_t const*)`, `setLoadAudioFileDataCallBack(void(*)(float*, int*))`, audio thread "Audio File Thread".
- **Swift `NuxPluginsPlugin`:** method channel `nux_plugins` (short Swift string) with methods `getPlatformVersion`, `loadAudioFileData`, `convertAudioToWaveFormat`, `exportAudioToWaveFormat`. EventChannel **`com.nux.juce.midi/events`** (the name is misleading: it streams audio-file float data via `audioFileDataCallback(data:value:)`). It also has `initMidiListener()`.
- **Formats** (JUCE: WAV/BWF, AIFF, FLAC 1.4.3, Ogg Vorbis, CoreAudio): `error: sample rate error`, `error: file format error`.
- **Dart side:** targetPath/tmpPath, `/Nexustone/cache/ir/`, `_ir_48.wav`, `_t_ir_48.wav`, "Save As IR File", `.bin`, `NCabDataTBLE_RES`, `IR [`.
- In short: imported IR audio is resampled/converted to a 48 kHz WAV, then the float samples are sent to Dart for upload.
- Accelerate/IOKit/CoreMIDI are linked only through JUCE.

### imagelib (disguised NAM loader)
- **ObjC `ImageCallbackBridge`:**
  - `-loadNamFile:`, `-loadImageFile:`, `-loadDeepImageFile:`
  - `+register{LoadNamEnd,NamInfo,LoadImageEnd,LoadImageInfo,LoadDeepImageEnd,LoadDeepImageInfo}CallBackWithBlock:`
- **C++ `ImageUtility::{loadNamFile, loadImageFile, loadDeepImageFile, namModel, namBuffer,...}`.** Callbacks: `(float*,float*,float*)` for NAM/Image ("g1, g2, factor") and `(float*,float*)` for DeepImage.
- **Swift `ImagelibPlugin`:** channel `imagelib`, EventChannel **`com.nux.imagelib/events`**. Event types `NamInfo`, `NamData`, `ImageInfo`, `ImageData`, `DeepImageInfo`, `DeepImageData`, with keys `g1`, `g2`, `factor`. Dart: `Failed to load NAM file: `, `loadNamFile`, `clearLoadNam`, `getAllNamInfo`.
- **Embedded NeuralAmpModelerCore:** `nam::wavenet::WaveNet`, `nam::wavenet::a2_fast::A2FastModel<3>/<8>`, `nam::slimmable_wavenet`, `nam::container::ContainerModel`, `nam::lstm::LSTM`, `nam::convnet::ConvNet`, `nam::Linear`, all activations; nlohmann::json 3.12; Eigen. Strings: `MLCapture`, `sample_rate`, `A2FastConfig: unsupported channel count`.
- **Hypothesis:** the device runs NAM "A2" WaveNets (channels 3 or 8). imagelib loads `.nam`, then converts or extracts weight vectors (g1/g2) and a gain/normalisation `factor` for upload into the AMP "user"/Nfile slot. "Image" and "DeepImage" are probably other capture formats.

### nux_helper
Only `NuxHelperPlugin` with `getPlatformVersion`. Trivial.

### flutter_midi_command / universal_ble
Stock plugins (`SwiftFlutterMidiCommandPlugin`, `ConnectedNativeDevice`, ...). Nothing vendor-specific.

---

## 8. "IR" zip: `Marshall Jcm800 + V30 1960.zip`
- Contains a single file, `Jcm800.nam` (295,867 bytes, JSON). Extracted to `extracted/ir_zip/`; summary in `extracted/ir_zip/nam_summary.txt`.
- NAM version 0.7.0. architecture `SlimmableContainer`, sample_rate 48000. metadata: gear_type amp_cab, gear_make "Marshall JCM800", tone_type "british crunch", trainer "TONE3000", loudness −23.05, gain 0.8247.
- Two WaveNet sub-models:
  - `max_value 0.5`: channels 3, bottleneck 3, 23 layers, 1871 weights.
  - `max_value 1.0`: channels 8, 12146 weights.
  - Shared: kernel_sizes 6/15, dilations [1,3,7,17,41,101,239,…], LeakyReLU(0.01), head kernel 16.
- These are exactly the shapes of `A2FastModel<3>` and `A2FastModel<8>` in imagelib. So this file is an amp+cab **NAM capture** for the Amp(Nfile) slot, not a WAV IR.

---

## 9. Firmware `JME-EN_V1.1.1_modified.bin` (quick look)

Dumps are in `extracted/firmware/`: `firmware_summary.txt`, `raw_strings.txt`, `BINA_not.bin`, `BINA_not_strings.txt`.

**Header** [CONFIRMED]:
```
00: "NUX DFU" 02
08: u32 0x00F6B6A1 = total file size (16168609)
0C: u32 2
10: u32 0xC1579527            (checksum/CRC? did not match plain CRC32 of any obvious range)
14: "JME STOMPBOX DFU"         (matches USB product string)
114: "SORAN"
214: "V1.1.0.x"               (despite the V1.1.1 file name; file is "_modified")
224: section table {tag, size, offset}:
     TEXT size 0x589    off 0xF6B018   -> multilingual release notes [en]/[zh-CN]/[zh-TW]
     BINA size 0xF6AC5C off 0x3BC      -> firmware image (~15.4 MB)
     EXTR size 0x100    off 0xF6B5A1   -> all zero
3B8: 01 00 00 01
```

- **MCU:** NXP **i.MX RT10xx** (Cortex-M7). Evidence:
  - MCUXpresso drivers `fsl_sai.c`, `fsl_semc.c` (SDRAM), `fsl_flexspi.c`, `fsl_edma.c`, `fsl_lpuart.c`, `fsl_lpspi.c`, `fsl_adc.c`, `fsl_adc_etc.c`, `fsl_xbara.c`, `fsl_pit.c`, `fsl_gpio.c`, `fsl_os_abstraction_bm.c`, `metro.c`.
  - SAI bases 0x40384000/0x40388000/0x4038C000.
  - Build path `Y:\Studio\GIO\JME\JME\{jme3,master,m_dfu}\...`.
- **Other strings:**
  - `BOOT JME=1/2/3`, `NUXPEDAL20201210`, `OK=MIDI`, `USB HOST`, `Soran Dual Stomp`, `Plexi Lead`, `AE-100X`, `Soran ASIO USB Audio Drive V6.14.0`
  - BLE module AT commands: `AT+GETBLENAME`, `AT+SETBLENAME=`, `AT+SETLOCALNAME=`
- **BINA** is mostly plain code/data (entropy 3–6 bits/byte per MB), with one high-entropy region around +0xD00000 (≈7.4 bits/byte; compressed tables, IR/NAM data or encrypted). A few header strings are bitwise-NOT obfuscated (e.g. `~"V1.1.0.x"` at BINA+0x1C).
- Effect names are not visible as plaintext, apart from `Plexi Lead`.
- **Updater** (`Device Firmware Updater 1.0.0.dmg`, `com.dfu.updater`): a JUCE app using libusb/hidapi (`HID_Write_Thread`, `NuxLibUSBManager`, `NUX_ODM/USB_HID_Updater`). The device must be put in DFU mode, and the update runs over **USB HID**, not MIDI. The binary is copied to `extracted/updater/`.

---

## 10. Open questions / suggested next captures
1. Capture USB-MIDI SysEx while the app connects. This confirms the `F0 00 21 7A 70 <msgId> <dir> ... F7` layout and the field order (uid/msgId/dir), and what dir=2 means. The same capture shows which header (`43 58 70 7E` or `00 21 7A`) the Dual Stomp Lite answers with.
2. Byte order and packing of `MidiUint14`, `MidiUint28` and `Uint16` (scene1..3). Check whether `Uint8` values above 127 are split.
3. The first byte of fragmented responses (the `total<<4|index` nibble), and the payload framing of preset dumps (11 vs 12 for preset/curset).
4. Check the msgIds for `getFwV2`, `getVersion`, IR upload/`coverSingleIr` and NAM upload in a capture. For deeper static work, follow callers of 0x3aaa64 and the name dispatcher 0x527d4 in `annotated_disasm.txt`.
5. The IR CRC algorithm (compare `getAllIRCrcAndName` against the WAV bytes of a known IR).
6. The `lang.bin` decryption: only needed if the in-code model DB turns out incomplete.

---

## 11. Files written (analysis/)

| File | Content |
|---|---|
| `static_analysis.md` | this report |
| `dependencies_from_NOTICES.txt` | 250 package names from NOTICES.Z |
| `dart_package_uris.txt` | surviving `package:` URIs |
| `flutter_assets_filelist.txt`, `asset_manifest_decoded.json` | asset listing and decoded AssetManifest.bin |
| `app_strings_filtered.txt` | protocol/device-related strings from the snapshot |
| `pool_strings_ordered.txt`, `pp_filtered.txt`, `blutter_object_pool.txt` | object-pool strings in code-use order; full blutter pool |
| `nep_command_calls.txt` | msgId/dir table from request-builder call sites |
| `device_struct_schema.txt` / `.json`, `struct_schema_tokens.txt` | struct dictionary |
| `effect_db_readable.txt`, `effect_db_tokens.txt` | effect/amp/cab model database |
| `inline_int_lists.txt` | all inline int-list literals in code (where the headers were found) |
| `proto_region_tokens.txt` | token stream of protocol code region 0x4cfff–0x62000 |
| `extracted/app_snapshot/` | thin arm64 App, `strings_all.txt`, `strings_uniq.txt`, `text_disasm.txt`, **`annotated_disasm.txt`** |
| `extracted/blutter_out/` | blutter output (pp.txt, objs.txt, partial asm, ida_script) |
| `extracted/native/` | per-framework nm / demangled / strings dumps |
| `extracted/NOTICES.txt` | decompressed licences |
| `extracted/ir_zip/` | `Jcm800.nam` + summary |
| `extracted/firmware/` | firmware header summary, strings, NOT-decoded BINA |
| `extracted/updater/` | DFU updater binary, Info.plist, strings |
| `scripts/` | `decode_assetmanifest.py`, `find_patterns.py`, `annotate_disasm.py`, `find_inline_int_lists.py`, `token_stream.py`, `schema_from_tokens.py`, `callers.py`, `cmd_table.py`, `blutter_macos_macho.patch` |
| `tools_src/blutter/` | patched blutter + Dart 3.12.2 VM build (about 133 MB; can be deleted) |
| `.venv/` | Python venv (pyelftools, requests) |
