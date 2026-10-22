# Changelog

This file records user-visible changes in each release.

## [Unreleased]

- Added `arle serve --served-model-name <NAME>`: the id `GET /v1/models` lists and responses carry as `model`. A Hugging Face cache snapshot path is now named by its repo (`<name>`) by default instead of its revision hash.
- Removed the HIP, Vulkan, and GGUF backends.
- Moved OPD training to [acupof-ai/arle-opd](https://github.com/acupof-ai/arle-opd): `arle train` is removed; `arle model download` and `arle serve --dspark-markov-init` remain.
- Removed the project history ledgers and process tooling (experience entries, plans, agenda, pre-registration and hygiene scripts, git hooks).
