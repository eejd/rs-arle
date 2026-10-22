# Upstream and adopted work (`local` branch)

Primary upstream: `acupof-ai/arle` (GitHub fork parent). Secondary upstreams tracked: none.
Fork repo: `eejd/rs-arle` (linguist primary language of the upstream: Rust, 6.4 MB of 9.5 MB in GitHub's language statistics;
repo name per docs/development-model.md "Repository naming" in `eejd/macports-ports-local`).

`main` mirrors `acupof-ai/arle` `main` (fast-forward only, never edited). `local` is `main` at
`5caecf6a1` plus the topic branches below.

## Base of `local`: `5caecf6a1` is upstream's `v0.5.8`, re-written

[INFERENCE] Upstream re-wrote the history of `main` (it has a tag `backup/pre-cursor-strip`; the
rewrite itself was not examined). Measured: no release tag of upstream is an ancestor of `main` (checked 2026-10-20 for `v0.5.0`,
`v0.5.1`, `v0.5.3`, `v0.5.8`; `git merge-base --is-ancestor v0.5.8 origin/main` fails). The
commit that carries `v0.5.8`'s content on `main` is `5caecf6a1` ("docs(changelog): 0.5.8 eval ...",
2026-08-21):

| | |
|---|---|
| Upstream tag | `v0.5.8`, annotated, tag object `aa8fc9fe11073f48261efa49b8f855911328c76f`, commit `ca793eacbeba36fe48a5e206fc36a2b4080e1f4f` (pushed to this fork as the tag `v0.5.8`; its history is not reachable from `main`) |
| Same content on `main` | `5caecf6a16a127c7ff816f79b5f878ccd0bd5c70`; `git rev-parse ca793eacb^{tree} 5caecf6a1^{tree}` is `0286bc01a809b6b93e9f9ce4819792d602f97cdc` for both |
| MacPorts distfile | `https://github.com/acupof-ai/arle/archive/v0.5.8/arle-0.5.8.tar.gz` (port `llm/arle` 0.5.8, a stable-tier port fetched from upstream): sha256 `f49502af...a96`, 16945102 bytes, recomputed 2026-10-20 and equal to the Portfile's; the unpacked tarball is file-for-file identical (`diff -r`) to `git archive 5caecf6a1` |
| `main` at fork creation | `af5fa54cd76071bc3e10d4f641c1880c0dbe8bb6` (2026-09-24), 1104 commits after `5caecf6a1` |

`local` therefore starts at the commit equal to the released tree, not at current `main`: the
release is what the port builds and what the defects below were measured on. A topic is
upstreamable only when it also applies to `main`; each topic row says whether it does.
Release tags on `local` are `v0.5.8-local<NN>` (`<base>` = 0.5.8, the last release `local`
contains; docs/development-model.md, ADR-0008 §2); they are listed under "Tags" below. The
MacPorts port `arle-local` (`llm/arle-local` in `eejd/macports-ports-local`) builds them.

## Adopted from other repos

None.

## Topic branches

Topics are merged into `local` with `--no-ff` (`Merge topic/<name> into local (#N)`); the three
topics below are merged. Nothing here is posted to `acupof-ai/arle`
before the quantbench + agentbench soak (ADR-0008); no topic has an upstream PR. The topics are
branched from the base (`5caecf6a1`), not from upstream `main` (`af5fa54`, 1104 commits past it);
"Upstreaming plan" below records how each applies there.

| Topic branch | Merged | What | Origin | Upstream status |
|---|---|---|---|---|
| `topic/boolean-tool-arguments` (head `7c489ba38`) | eejd/rs-arle#4, merge `3d0640e64`; fixes #1 | Qwen3.5 writes a boolean tool argument as Python `True`/`False` (its chat template renders non-container arguments with Jinja's `string` filter), which the server returned as the string `"True"` (quantbench `tool_arg` 16/20). `chat::BooleanArguments` retyped string arguments of declared-boolean parameters after the parse (schema read through `type`, `anyOf`/`oneOf`/`allOf` and local `$ref`, node limit 128, depth 8). Touches `crates/chat`, `crates/infer-server`. **Superseded in code by the next row:** `BooleanArguments` no longer exists on `local` (`git grep BooleanArguments` is empty); the merge stays in history and #5 keeps #4's schema walker. | authored here (eejd/rs-arle#1 has the evidence) | not-submitted |
| `topic/schema-typed-arguments` (head `f2b0ed4a0`, stacked on `topic/boolean-tool-arguments`) | eejd/rs-arle#5, merge `af29d047a`; fixes #2 (and #1 again, in the parse) | Tool-call arguments are typed by the request's tool schema inside the parse: `chat::ToolSchemas` (parameters declared `string` or `boolean`) is passed into `parse_tool_calls`, `openai_parse_tool_calls`, `StreamingToolCalls::new` and `parse_native_function_block`. A declared `string` parameter keeps its exact text (`12345`, `3.50`, `1e3`, `true`, `null` were returned as number/boolean/null), a declared `boolean` accepts `true`/`false` in any case (`True`); everything else is typed as before. All callers migrated (`infer-server` OpenAI and Anthropic, stream and non-stream; the `arle run` agent in `crates/agent`). Touches `crates/chat`, `crates/infer-server`, `crates/agent`. Subsumes #4's post-pass. The merge also brings `docs/evidence/live-2026-10-20/` (the live probe scripts and results of #1, #2, #4 and #5, scrubbed of host paths and names; README there). Live, Qwen3.5-4B-4bit, 4 API paths: `tool_arg` 20/20 (installed 0.5.8: 16/20) and `strnum` 6/6 (installed 0.5.8: 0/6). | authored here (eejd/rs-arle#2) | not-submitted; the evidence commit `f2b0ed4a0` is to be dropped when cutting the topic for upstream |
| `topic/served-model-name` (4 commits: `fix(infer-api)` hub cache snapshot id, `feat(serve)` `--served-model-name`, `docs(serve)`, `test(infer-api)` helper and CPU router test) | eejd/rs-arle#8, merge `988db6761`; fixes #7 | `GET /v1/models` listed `0e7ffd5c...` for Qwen3.5-4B-4bit: the served id is the final segment of `--model-path`, which for a Hugging Face cache snapshot (`.../models--<org>--<name>/snapshots/<revision>`) is the 40-hex revision, and nothing could name it. `arle serve --served-model-name <NAME>` sets the id `/v1/models` lists and responses carry as `model` (all `router_*` backends and the CUDA multi-group coordinator); the default id is unchanged except that a hub cache snapshot is named by its repo `<name>`. The request's own `model` stays unmatched (accept-any: the repo's tooling and `cc_harness` depend on it; `docs/http-api.md` and `docs/troubleshooting.md` described a `404 model_not_found` the server lost when the legacy `infer/` crate was deleted in `7adabec86`, and are corrected). Touches `crates/cli` (`args.rs`, `serve.rs`), `crates/infer-api` (`loaded.rs`, `serve.rs`, `serve_engine.rs`), docs, `CHANGELOG.md`. | authored here (root cause: rs-arle#7) | not-submitted; the quantbench registry follows it (`served_id = "flag"`, py-quantbench draft) |

## Tags

| Tag | Commit | Contents | Checked |
|---|---|---|---|
| `v0.5.8-local01` (annotated, tag object `67e31e4ed4ddeb2f170e27bba6e14ce61d3c6751`) | `af29d047a58bcb50f29426b2983d81af79507f42` (`local` after the merge of #5, 2026-10-21) | upstream `v0.5.8` (tree of `5caecf6a1`) + `UPSTREAM.md` + `topic/boolean-tool-arguments` + `topic/schema-typed-arguments`. `Cargo.lock`, `Cargo.toml` and every crate manifest are byte-identical to `v0.5.8`'s (`git diff v0.5.8 v0.5.8-local01 -- '*.toml' '*.lock'` is empty), so the port's registry crate list is unchanged. The tag's `UPSTREAM.md` is the one before this file's topic and tag rows; it is documentation only, the rows were added afterwards. | live probes of #5 at `f47eca1dc` (code identical to the tag; see the evidence directory); the port `arle-local` builds the tag's archive (sha256 `c6fc2da4c9e78af8ac2b56b324a094740d010962f2c00325b6b6512bc5931483`, 16987835 bytes, `https://github.com/eejd/rs-arle/archive/v0.5.8-local01/rs-arle-0.5.8-local01.tar.gz`) |
| `v0.5.8-local02` (annotated, tag object `e3ea410549e4fba860574e71aef136b8b75ff66d`) | `988db67613807354294beee6a94723f5b519c454` (`local` after the merge of #8, 2026-10-22) | `v0.5.8-local01` + `UPSTREAM.md` rows (#6) + `topic/served-model-name` (#8). Against `v0.5.8-local01` the tag changes 9 files (`CHANGELOG.md`, `UPSTREAM.md`, `crates/cli`: `args.rs`, `serve.rs`; `crates/infer-api`: `loaded.rs`, `serve.rs`, `serve_engine.rs`; `docs/http-api.md`, `docs/troubleshooting.md`); `Cargo.lock` and every `Cargo.toml` are still byte-identical to `v0.5.8`'s (the topic adds no dependency to the build; the `tempfile` dev-dependency exists only on `prep/upstream-served-model-name`, not on `local`), so the port's registry crate list is unchanged. The tag's `UPSTREAM.md` is the one before this file's `local02` rows (it still lists #8 as a draft); documentation only, the rows were added afterwards. Behaviour change for a client that reads the id: `GET /v1/models` and the `model` of responses name a hub cache snapshot by its repo (`Qwen3.5-4B-4bit`), not by its revision hash; `--served-model-name` sets it; the request's own `model` is still not matched. | tests of the topic (`infer-api` model-id, `served_model_id` and CPU router tests, `cli` serve tests) and the live table of #8 at `895522621` (the tag's code differs from `895522621` by the review commit `3644eec29` only, `crates/infer-api/src/loaded.rs`: new tests, and `served_model_id` now also ignores a blank name and trims one, which the CLI already refused and trimmed, so no live run covers that helper); the port `arle-local` builds the tag's archive (sha256 `7431e94547a542273e89e34910243e83d2413b2e7b68a8227de4391a4f06e8eb`, 16994119 bytes, `https://github.com/eejd/rs-arle/archive/v0.5.8-local02/rs-arle-0.5.8-local02.tar.gz`) |

The version a build of this fork reports (`arle --doctor`, the `version` field of `arle --doctor
--json`, the startup banner) is `CARGO_PKG_VERSION` of the `cli` crate, `0.5.8` in this source;
`arle --version` does not exist in 0.5.8. The port stamps `0.5.8+local<NN>` (`0.5.8+local02` for the newest tag) into `crates/cli/Cargo.toml`
and `Cargo.lock` at build time (docs/development-model.md, "Version strings of `-local` builds");
there is no stamp commit here, so a build made from the fork alone reports `0.5.8`.

## Upstreaming plan

Preparation only. **Nothing is sent to `acupof-ai/arle`** (no issue, no PR, no push) until the topics have
soaked in production under quantbench **and** agentbench (eejd/py-quantbench#220) and have had an
independent review; ADR-0008 forbids upstream contact before that. Each candidate below has a prep branch
on this fork, `prep/upstream-<topic>`, cut on upstream `main` at `af5fa54cd` (2026-09-24, the head when the
fork was created and still on 2026-10-22), so that "applies and passes there" is measured, not assumed. The
prep branches are cuts for the future PR, not the fork's own history (they are not merged into `local`); they
carry no `UPSTREAM.md`, no evidence directory and nothing that names this fork.

| Candidate | Upstream cut | Prep branch (tip, commits) | Size vs upstream `main` | Rebase from `local` |
|---|---|---|---|---|
| Tool-call argument typing | One PR: the net result of `topic/boolean-tool-arguments` (#4) and `topic/schema-typed-arguments` (#5). #4's intermediate `BooleanArguments` post-pass is not sent: #5 replaces it and keeps #4's schema walker. | `prep/upstream-schema-typed-arguments` (`065f23da8`, 1 commit: `fix(chat): type tool-call arguments by the request's tool schema`; the tree is the one of the earlier two-commit tip `1aa688a8a`, tag `archive/prep-schema-typed-arguments-1aa688a8a`) | 6 files, +680/-49 (about 340 of the added lines are tests) | Applies with a 3-way merge and **3 files in conflict, resolved by hand**: `crates/chat/src/lib.rs` (upstream's dead-code sweep shortened the `pub use protocol::{..}` list: only `ToolSchemas` is added to it), `crates/chat/src/tool.rs` (upstream dropped the restating doc comments; the new block is placed next to them without the restating lines), `crates/infer-server/src/coordinator.rs` (two hunks: a line added next to upstream's `let think_open = reasoning_seed(..)`, and the new test module beside `dp_failure_domain_tests`). |
| `--served-model-name` | One PR of four commits, one concern each: the id of a hub snapshot; the flag with the `served_model_id` helper and its CLI unit tests; the docs; the tests of the helper and the end-to-end `arle serve` test. | `prep/upstream-served-model-name` (`db87af2a1`, 4 commits, tree identical to the earlier tip `dbd937f43`, tag `archive/prep-served-model-name-dbd937f43`) | 11 files, +304/-6 (132 of the added lines are in the test commit, 13 in the docs commit) | Not a textual rebase: upstream replaced the per-backend `router_*` functions with a registry and one `router_for_backend`, so the 8 `config.served_model_id(model_id)` sites of the `local` version collapse to **one line** after `build(..)`, and `model_id_from_path` is already `pub`. Two docs hunks (`docs/http-api.md`, `docs/troubleshooting.md`) are smaller there: upstream already documents "any string" for `model`, so the correction of the stale `404 model_not_found` text is not part of the upstream cut. |

**Checked on the prep branches** (2026-10-22, MacPorts Rust 1.99.0; upstream `main` pins 1.98.0 in `rust-toolchain.toml`, so the lint
result is close to but not the CI's; re-run on 1.98.0 before sending: it was not installed on 2026-10-22, there is no `rustup` on the host and MacPorts ships only 1.99.0), no `port` command, `RUSTC_WRAPPER=` empty:

| | prep tool-argument typing | prep served-model-name |
|---|---|---|
| tests | `cargo test -p chat -p infer-server -p agent`: chat 5, infer-server 42, agent 1: all pass | upstream `ci.yml` backend lane, `cargo test -p infer-api -p arle -p cli --profile release-fast --no-default-features --features cpu,no-cuda,cli`: all pass, including 3 `served_model_name`, 3 `model_id` and 2 `served_model_id` unit tests and an end-to-end `tests/serve_model_name.rs` (starts `arle serve --backend cpu` on a minimal tokenizer, reads `GET /v1/models`; adds a `tempfile` dev-dependency to the root crate and one line to `Cargo.lock`) |
| lint | `cargo clippy -p chat -p infer-server -p agent --all-targets`: no warning | the three CPU lanes of `ci.yml` with `-D warnings` (`-p infer-api .. --features no-cuda --lib`, `-p cli .. --features no-cuda`, `-p arle .. --features cpu,no-cuda,cli --bin arle`): clean. The CUDA lanes (`cuda,no-cuda,nccl,deepep`) were typechecked on the `local` version of the topic only (`CUDARC_CUDA_VERSION=12080`), not on the prep branch |
| format | `rustfmt --check` on the touched files: clean | same |
| behaviour | live checks of the `local` versions (see the topic rows); not re-run on the prep branches | same |

The prep branches carry no private names: both were rebuilt on 2026-10-22 (force-pushed to this fork's `prep/*` only, old tips kept as the two `archive/prep-*` tags) so that no commit's patch or message names the fork's validation tooling or a revision hash of the model used in the measurements. Checked with `git log -p af5fa54cd..<tip>` and the commit messages and authors, grepping for the tool names, the revision, the host and user names and the fork's name: no match (the one match left in the tool-argument branch is the upstream function name `render_deepseek_v4_tool_arguments` in a hunk header). **Repeat that rebuild and that grep on any branch cut later**: a squash or a fixup of the test commits is what removes such names, and a `git log -p` grep is the check. The archive tags still carry the earlier commits and are never to be sent.

**Prerequisites before any upstream contact** (each is open until it is ticked in this file):

1. Production soak: quantbench **and** agentbench (py-quantbench#220) on a deployed `arle-local` that carries the topic.
2. Independent review of the series as it will be sent (the fork's own reviews covered the `local` versions).
3. Rebase onto the then-current upstream `main` (it moves; the prep branches are dated above), re-run the table above, re-run
   `clippy` on the pinned toolchain.
4. Conventions read from upstream's `CONTRIBUTING.md` and `.github/` on `main`: Commitizen messages (`<type>(<scope>): <subject>`),
   one logical change per PR, docs updated in the same PR for a CLI change (`docs/http-api.md`, `docs/troubleshooting.md`,
   `CHANGELOG.md` `## [Unreleased]`), the PR template (surface area, stability/compatibility, docs updated, validation, benchmark
   evidence: "No performance claim" for both candidates), CI = `ci.yml` (fmt scoped to changed files, clippy `-D warnings` CPU and
   `cuda,no-cuda` lanes, `cargo test` backend and device-neutral lanes), `metal-ci.yml` (macos-14: `cargo test -p infer-api -p cli -p arle`)
   and `cargo deny`. Upstream `main` no longer has `scripts/check_repo_hygiene.py` that the template and `v0.5.8` mention.
5. Licence and sign-off: the repo is MIT (`LICENSE`, "Copyright (c) 2026 cklxx"); no CLA, DCO or `Signed-off-by` requirement appears in
   `CONTRIBUTING.md`, `README.md` or `.github/` (searched 2026-10-22); `.github/CODEOWNERS` assigns everything to `@cklxx`.
   The commit author on the prep branches is `Eric DeWitt <edewitt@mac.com>`; whether that is the identity for the public record is the
   owner's call.
6. PR texts for upstream are written from scratch in upstream's terms: no fork issue numbers, no `eejd`, no MacPorts or host details;
   the measurements restated (tool_arg 16/20 -> 20/20 with the string-typed cases 0/6 -> 6/6; `/v1/models` listing the revision
   hash -> the named id) with their commands.
7. Questions to put to the maintainers with the PRs, not decided here: whether they want the request `model` matched
   (the fork keeps accept-any, which their docs on `main` state), and the flag spelling `--served-model-name` (SGLang's and vLLM's).
