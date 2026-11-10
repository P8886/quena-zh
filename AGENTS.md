# Local development

## Disk usage

The user has limited disk space. Keep only necessary build outputs and reusable
dependencies; do not accumulate historical build caches.

- Keep incremental compilation disabled and use the compact profiles in
  `Cargo.toml`. Do not override them unless the task specifically needs debugging.
- Reuse this checkout's `target/` and existing toolchain/dependency caches. Avoid
  new target directories, duplicate checkouts, or dependency installations per run.
- When `target/local` is already built, prefer focused Rust checks with
  `--profile local` to reuse its dependencies. Use the default development profile
  when a development session specifically needs it.
- Run focused checks first. Build a full app or installer when the user requests
  it or when lighter checks cannot validate the change.
- Record test commands, results and relevant source versions in
  `docs/testing-and-build-artifacts.md`. After testing, remove generated standalone
  test executables and temporary fixtures when idle; keep test source code and
  reusable compiled dependencies.
- Check disk usage after builds. Keep `target/` within 6 GiB by removing obsolete
  generated test outputs, incremental caches and temporary packaging files. Use
  Cargo's cleanup commands or hold its build locks; never remove files used by a
  running compiler or packager.
- Preserve source files, lockfiles, bundled plugin resources, current deliverables,
  installed apps and captured user data. Cleanup stays within verified generated
  build directories; do not sweep unrelated files or user folders.
