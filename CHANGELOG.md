## [unreleased]

### 🚀 Features

- *(rules)* See workloads whose pods were never created ([4ea285d](https://github.com/murat-akpinar/k8rs/commit/4ea285d34ca40c06bb2a589ba746561737b2833f))
- *(ci)* Scaffold the crate and the guards that must be seen red ([d42b5e2](https://github.com/murat-akpinar/k8rs/commit/d42b5e2aaac24eaebd56d39991a5c5611c7950e0)) — cargo init at 0.0.0, clippy.toml, deny.toml, justfile and the CI workflow. Two guards ship with their own proof: test-guard compares declared against listed tests, write-guard derives the write ban from kube's own Api surface.
- *(fixtures)* Sanitize before capture, and cover the two blind spots ([17d2556](https://github.com/murat-akpinar/k8rs/commit/17d2556c5e1ec36bf16e7792707ff8b583635b6e)) — The sanitizer lands before the first fixture, as required: payloads destroyed, references kept, and an object whose node identifiers are not the kind cluster's is refused rather than quietly rewritten. It is tested against a poisoned object in just check and in CI.
- *(fixtures)* Add certificate fixtures with pinned dates ([f2ad09d](https://github.com/murat-akpinar/k8rs/commit/f2ad09d455559a0ff3be690afd564ccaa846bf5d)) — Three self-signed client certificates for the C-series rules, generated locally with their keys deleted. The dates are pinned rather than relative: a fixture generated with -days 20 is a test that fails in three weeks, and the usual repair for that is to weaken the test. Snapshot carries now, so the test states the date it asks about.

### 🐛 Bug Fixes

- *(ci)* Let cargo-deny accept our own GPL licence ([2717bd2](https://github.com/murat-akpinar/k8rs/commit/2717bd2b7cfd8cccd785111652ee12f407dfc9fe)) — cargo-deny checks the root crate too, so the permissive-only policy written for dependencies rejected k8rs itself. The exception is scoped to this crate; a copyleft dependency is still refused.
- *(fixtures)* Sanitize every shape the capture produces, not just one object ([842a767](https://github.com/murat-akpinar/k8rs/commit/842a767f95fac9bc1522a8000c74c9344fdd896b)) — Found by auditing Phase 2 before running the capture — nothing was failing.

### 📚 Documentation

- Record the git remote and the history restart ([244f9ce](https://github.com/murat-akpinar/k8rs/commit/244f9cec080ba07a956b703b676f9aba629a616a))
- Close the four holes that let a green build lie ([3fc8b6d](https://github.com/murat-akpinar/k8rs/commit/3fc8b6dee11cdfa2033e6a2bb66feeb7179ee01f))
- Cover init containers and ship rule 10 in v1 ([8add7fb](https://github.com/murat-akpinar/k8rs/commit/8add7fbfee62ca99d6c2230b70f9b523a4cc15a0))
- Add the phase-close ritual to the workflow ([8333b66](https://github.com/murat-akpinar/k8rs/commit/8333b668eeb3453db3fed0a1398e043fcd0b5bda)) — A phase closes with the todo boxes checked and true, the security gate run, docs and changelog synced, CI green — and a plain statement that the context should be cleared. Clearing is the user's command; nothing here can issue it.

### ⚙️ Miscellaneous Tasks

- Init ([542deb0](https://github.com/murat-akpinar/k8rs/commit/542deb041294e191d6af55acc0e1aef3b246cfa9)) — The whole design phase: decision record, requirements, screens, build plan and the working rules. No code yet.
