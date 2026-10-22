# One-off NAM reverse-engineering scripts

Throwaway scripts from working out the official NAM converter and the user AMP upload. Kept for reference; they read
git-ignored data (`analysis/extracted/namconv/`, `captures/files/`), so most only run on a machine that has it. Run
from the repository root with the open_stomp Python bindings installed. The pedal simulator and optimiser analysis
lives in open-stomp-lib `tools/pedalsim`.

| Script | What it checked |
|---|---|
| `nam_traffic.py`, `slotinfo.py` | the user AMP upload traffic (SysEx `0x58`) and slot-info records in the main capture log |
| `ab_test.py` | A/B on the pedal: the app's Jcm800 conversion vs ours, uploaded to two slots and read back |
| `g1check.py` | the app's G1 gain curve and minimum-phase response vs a harness dump (`g1check.py <dump name>`) |
| `blaseed.py`, `multisine.py` | whether the app's BLA excitation and level-tester multisine come from macOS `rand()` (the multisine does; `BLA_x` doesn't) |
| `level_check.py`, `level_check2.py` | re-amp rig levels: M-Track out → pedal → USB (`level_check.py <dir with audio_io.py>`) |
| `nam_reference.py` | fit the rig's input/output calibration against the real model |
| `sigint.py`, `typecheck.py` | Python binding checks: Ctrl+C during `Device`, and the type stubs |
