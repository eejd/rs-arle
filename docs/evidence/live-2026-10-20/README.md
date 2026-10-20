# Live checks, 2026-10-20 (rs-arle#1, #2, #4, #5)

Host: Apple M5 Pro, macOS 27.0.1 (the owner's engine host; its name is scrubbed, see "Scrubbing"). One engine at a time under the host's engine lock, run as a separate engine user. `arle serve --backend metal --model-path <Qwen3.5-4B-4bit snapshot 0e7ffd5c> --metal-pipeline false --metal-paged-kv-read false` (the registry launch; every log shows `kv cache dtype = int8` and `caps rows per step at 1`). Requests by `probe.py` (Python 3.14, `PYTHONPATH=src` of py-quantbench main): the 20 seeded `tool_arg` cases of quantbench (`cases_tool_arg(20, 0)`, the cases of the 2026-10-20 engine run) plus 6 `strnum` cases (a `string` parameter `zip` holding `12345`, `20240131`, `3.50`, `8675309`, `1e3`, `42` and a `string` parameter `word` holding `true`, `null`, `false`, `True`, `None`, `42`), greedy, `enable_thinking` false, each through four paths (OpenAI chat and Anthropic `/v1/messages`, stream and non-stream): 104 requests per binary and run, one run each. A case passes when the returned call equals the expected `{name, args}` exactly. The probe is not `qualify engine` (no harness, no judge, no reference path).

Binaries (built in scratch worktrees without any `port` command: `MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --no-default-features --features metal,no-cuda,cli --package arle --bin arle`, `RUSTC_WRAPPER=` empty); "base" is the installed `arle 0.5.8` (upstream's tarball):

| name | code | sha256 (first 12) |
|---|---|---|
| base | installed `arle 0.5.8` | `ac2ec0abf778` |
| boolean | `topic/boolean-tool-arguments` @ `87c4e5987` (#4 after review round 1) | `b6ac161b2ad4` |
| schema (round 2) | `topic/schema-typed-arguments` @ `fe41d95cc` (#5 before review round 2) | `c9d9bfb490e1` |
| schema (final) | `topic/schema-typed-arguments` @ `f47eca1dc` (#5 after review round 2; the binary was built with the evidence commit on top, code identical) | `8811d540b20e` |

| run | binaries | tool_arg (20), every one of the 4 paths | strnum (6), every one of the 4 paths |
|---|---|---|---|
| `final-f47eca1dc/` | base | 16/20 (the 4 `scalars` cases: `flag: "True"`) | 0/6 |
| | schema (final) | **20/20** | **6/6** |
| `round2-87c4e5987-fe41d95cc/` | base | 16/20 | 0/6 |
| | boolean | 20/20 | 0/6 |
| | schema | 20/20 | 6/6 |
| `round1-040733f/` | base | 16/20 | not in that probe |
| | `topic/boolean-tool-arguments` @ `040733fde` | 20/20 | not in that probe |

All four paths give the same count for every binary. `round1-040733f/` ran the probe before the `strnum` cases were added (its 20 `tool_arg` cases are the same code as in `probe.py`). The base's `strnum` failures: `zip` `12345` / `20240131` / `8675309` / `42` returned as numbers, `3.50` as 3.5, `1e3` as 1000.0, and `word` `true` / `null` / `false` / `42` as true / null / false / 42 (`True` and `None` stay strings: not JSON).

Files per run: the run script, `run*.out` (console output; the `pkill ... Operation not permitted` lines are the root-owned `sudo` parent matching the `pkill -f` pattern, the engine itself exits), `probe-<binary>.json` (every request: got, want, ok), `serve-<binary>.log` (engine log, ~105 lines; the one line containing "error" is the log-file-sink permission message).

## Scrubbing
Absolute host paths, the engine user's name and the host's name were replaced in the scripts, logs and JSON before they were committed, so the scripts are templates: set `SCRATCH` (the scratch checkout), `LIVE` (the output directory of `probe.py`), `QUANTBENCH` (a py-quantbench checkout), `MODEL_CACHE` (the Hugging Face hub cache root), `ENGINE_USER` (the user the engines run as) before running them. Replacements: the hub-cache root -> `$MODEL_CACHE`, the py-quantbench checkout -> `$QUANTBENCH`, the scratch directories -> `$LIVE` / `$SCRATCH`, `sudo -n -u <engine user>` -> `sudo -n -u $ENGINE_USER`, other mentions of that user -> `<engine-user>`, the host name -> `<host>`; `/opt/local/...` (the public MacPorts prefix) was left. Nothing else was changed: the per-request `(case, path, ok, got, want)` records of all 7 `probe-*.json` files hash identically before and after (checked), and the engine logs differ only in the replaced path prefixes.
