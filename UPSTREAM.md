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

Both topics are merged into `local` with `--no-ff` (`Merge topic/<name> into local (#N)`). Nothing
here is posted to `acupof-ai/arle` before the quantbench + agentbench soak (ADR-0008); neither topic
has an upstream PR yet. Neither was rebased onto upstream `main` (`af5fa54`, 1104 commits past the
base): both touch `crates/infer-server/src/coordinator.rs`, where a rebase needs a textual
resolution (#4's PR body measured two trivial hunks for its first commit and did not re-check the
later ones).

| Topic branch | Merged | What | Origin | Upstream status |
|---|---|---|---|---|
| `topic/boolean-tool-arguments` (head `7c489ba38`) | eejd/rs-arle#4, merge `3d0640e64`; fixes #1 | Qwen3.5 writes a boolean tool argument as Python `True`/`False` (its chat template renders non-container arguments with Jinja's `string` filter), which the server returned as the string `"True"` (quantbench `tool_arg` 16/20). `chat::BooleanArguments` retyped string arguments of declared-boolean parameters after the parse (schema read through `type`, `anyOf`/`oneOf`/`allOf` and local `$ref`, node limit 128, depth 8). Touches `crates/chat`, `crates/infer-server`. **Superseded in code by the next row:** `BooleanArguments` no longer exists on `local` (`git grep BooleanArguments` is empty); the merge stays in history and #5 keeps #4's schema walker. | authored here (eejd/rs-arle#1 has the evidence) | not-submitted |
| `topic/schema-typed-arguments` (head `f2b0ed4a0`, stacked on `topic/boolean-tool-arguments`) | eejd/rs-arle#5, merge `af29d047a`; fixes #2 (and #1 again, in the parse) | Tool-call arguments are typed by the request's tool schema inside the parse: `chat::ToolSchemas` (parameters declared `string` or `boolean`) is passed into `parse_tool_calls`, `openai_parse_tool_calls`, `StreamingToolCalls::new` and `parse_native_function_block`. A declared `string` parameter keeps its exact text (`12345`, `3.50`, `1e3`, `true`, `null` were returned as number/boolean/null), a declared `boolean` accepts `true`/`false` in any case (`True`); everything else is typed as before. All callers migrated (`infer-server` OpenAI and Anthropic, stream and non-stream; the `arle run` agent in `crates/agent`). Touches `crates/chat`, `crates/infer-server`, `crates/agent`. Subsumes #4's post-pass. The merge also brings `docs/evidence/live-2026-10-20/` (the live probe scripts and results of #1, #2, #4 and #5, scrubbed of host paths and names; README there). Live, Qwen3.5-4B-4bit, 4 API paths: `tool_arg` 20/20 (installed 0.5.8: 16/20) and `strnum` 6/6 (installed 0.5.8: 0/6). | authored here (eejd/rs-arle#2) | not-submitted; the evidence commit `f2b0ed4a0` is to be dropped when cutting the topic for upstream |

## Tags

| Tag | Commit | Contents | Checked |
|---|---|---|---|
| `v0.5.8-local01` (annotated, tag object `67e31e4ed4ddeb2f170e27bba6e14ce61d3c6751`) | `af29d047a58bcb50f29426b2983d81af79507f42` (`local` after the merge of #5, 2026-10-21) | upstream `v0.5.8` (tree of `5caecf6a1`) + `UPSTREAM.md` + `topic/boolean-tool-arguments` + `topic/schema-typed-arguments`. `Cargo.lock`, `Cargo.toml` and every crate manifest are byte-identical to `v0.5.8`'s (`git diff v0.5.8 v0.5.8-local01 -- '*.toml' '*.lock'` is empty), so the port's registry crate list is unchanged. The tag's `UPSTREAM.md` is the one before this file's topic and tag rows; it is documentation only, the rows were added afterwards. | live probes of #5 at `f47eca1dc` (code identical to the tag; see the evidence directory); the port `arle-local` builds the tag's archive (sha256 `c6fc2da4c9e78af8ac2b56b324a094740d010962f2c00325b6b6512bc5931483`, 16987835 bytes, `https://github.com/eejd/rs-arle/archive/v0.5.8-local01/rs-arle-0.5.8-local01.tar.gz`) |

The version a build of this fork reports (`arle --doctor`, the `version` field of `arle --doctor
--json`, the startup banner) is `CARGO_PKG_VERSION` of the `cli` crate, `0.5.8` in this source;
`arle --version` does not exist in 0.5.8. The port stamps `0.5.8+local01` into `crates/cli/Cargo.toml`
and `Cargo.lock` at build time (docs/development-model.md, "Version strings of `-local` builds");
there is no stamp commit here, so a build made from the fork alone reports `0.5.8`.
