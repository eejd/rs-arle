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
contains; docs/development-model.md, ADR-0008 §2). No `local` tag exists yet and there is no
`arle-local` port: `local` carries no topic until one is merged.

## Adopted from other repos

None.

## Topic branches

None merged. Nothing here is posted to `acupof-ai/arle` before the quantbench + agentbench soak
(ADR-0008).
