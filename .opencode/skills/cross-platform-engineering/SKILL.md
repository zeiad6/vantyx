---
name: Cross-Platform Engineering
description: Keep code buildable and testable on Linux, Windows, and macOS by auditing paths, shells, terminals, native toolchains, linking, packaging, and CI matrices without claiming unverified platform support.
---

# Cross-Platform Engineering

## When to use

- Adding filesystem, process, terminal, path, locale, signal, or dynamic-linking behavior.
- Introducing a native dependency, C/C++ toolchain, SDK, installer, or packaging step.
- Setting up or changing a CI matrix for multiple operating systems or architectures.
- Reviewing a release claim such as "supports Windows" or "works on macOS."

Do not claim platform support from a successful cross-compile alone. Execution evidence is required.

## Required tools

- Native or cross toolchains (`rustup target`, MSVC/MinGW, Xcode/Command Line Tools, GCC/Clang, Zig).
- `file`, `ldd`/`otool`/`dumpbin`, `nm`, `objdump`, and archive tools.
- CI runners or VMs for each claimed platform; Wine only for limited compatibility checks.
- Shell and path test fixtures, including CRLF, Unicode, spaces, long paths, and symlinks.
- Installer/package tools for the target platform when packaging is in scope.

## Safety constraints

- Do not claim a platform is supported unless the build and relevant runtime tests were executed there and recorded.
- Do not add platform-specific hacks without a comment, a test, and a documented fallback.
- Preserve line endings and file encodings; do not let an editor rewrite unrelated files.
- Do not assume `/tmp`, `/bin/sh`, POSIX permissions, case-sensitive paths, or symlink privileges.
- Do not ship unsigned binaries where signing/notarization is required by policy.
- Keep secrets and signing identities out of CI logs and repository files.

## Workflow

1. **Define the support matrix.** List OS, architecture, minimum version, libc/runtime, toolchain, and support tier (verified, best-effort, unsupported). Record the evidence required for each tier.

2. **Audit platform-sensitive code.**
   ```sh
   rg -n '/bin/|/tmp|sh -c|chmod|chown|ln -s|mktemp|sed -i|export ' src
   rg -n 'std::path|PathBuf|OsStr|to_str|to_string_lossy|canonicalize' src
   rg -n 'cfg\(|target_os|target_arch|target_family|windows|unix' src
   ```
   Check path separators, case sensitivity, drive letters, UNC paths, long paths, symlinks, hard links, permissions, line endings, locale, and executable suffixes.

3. **Replace shell assumptions with APIs.** Use `std::process::Command` with argument arrays, `std::path::Path`/`PathBuf`, `std::env::temp_dir`, and explicit encoding/decoding. Avoid string concatenation for paths and shell quoting for arguments.

4. **Abstract native toolchains and linking.** Record how the compiler/linker is discovered, how flags are passed, and how runtime libraries are found. Support an explicit override such as `CC`, `CXX`, or a project-specific variable. Verify RPATH/RUNPATH, DLL search paths, frameworks, and static vs dynamic linking.

5. **Handle terminal behavior.** Detect `isatty`; disable ANSI escapes, progress bars, and interactive prompts when output is piped. Test Windows console code pages, UTF-8 output, and signal/ctrl-c behavior.

6. **Write platform-neutral tests.** Use temporary directories with unique names, clean up best-effort, and skip unsupported features with a clear reason. Never report a skipped test as a pass.

7. **Build on real runners.**
   ```sh
   cargo build --workspace
   cargo test --workspace
   cargo clippy --workspace --all-targets -- -D warnings
   ```
   Repeat with the platform's native toolchain. For C/C++ components, test both available compilers (for example GCC and Clang) and record diagnostics differences.

8. **Set up a CI matrix.** Include OS, architecture, toolchain, and feature combinations. Cache dependencies, upload test artifacts, and fail on skipped required jobs. Mark unverified platforms as allowed failures only with an owner and a follow-up date.

9. **Validate packaging and runtime.** Install the produced artifact on a clean machine/container. Check dynamic libraries, data files, config paths, uninstall behavior, code signing, and notarization. Do not test only from the build directory.

10. **Document support and limitations.** Update the support matrix, known issues, and platform-specific instructions. State exactly which commands were run and on which image/version.

## Validation and acceptance criteria

- The support matrix states verified, best-effort, and unsupported platforms.
- No new POSIX-only shell or path assumption is introduced.
- Paths use platform APIs; process arguments use arrays, not shell strings.
- Tests cover CRLF, spaces, Unicode, long paths, and missing permissions where relevant.
- CI runs on real runners for every claimed platform, or the claim is removed.
- Native libraries and runtime dependencies resolve on a clean install.
- Skipped tests are visible with reasons and are not counted as passes.
- Packaging and signing requirements are satisfied for the release tier.

## Expected output

1. Support matrix with evidence status.
2. Platform-sensitive code audit with file:line references.
3. Abstraction/guard changes and tests.
4. CI matrix configuration.
5. Build/test evidence per platform and toolchain.
6. Packaging/linking verification on a clean environment.
7. Known limitations and unverified surfaces.

## Troubleshooting

- **Fails only on Windows:** check path separators, drive/UNC paths, CRLF, file locking, executable suffix, console encoding, and case-insensitive collisions.
- **Fails only on macOS:** check SDK version, deployment target, architecture (arm64/x86_64), frameworks, and notarization.
- **Missing shared library:** inspect `ldd`/`otool -L`/`dumpbin /dependents`, RPATH, and the install layout; do not hardcode a build-machine path.
- **C compiler missing:** skip with a clear message or fail the required job; never report a false pass.
- **Symlink test unsupported:** gate on platform capability and document the skip.
- **CI passes but users fail:** test the packaged artifact on a clean image, not only the source tree.
- **Line-ending churn:** configure `.gitattributes` and editor settings; verify generated files explicitly.

## Examples

```sh
# Cross-target build checks
rustup target list --installed
cargo build --target x86_64-pc-windows-gnu
cargo build --target aarch64-apple-darwin

# Inspect linkage
file target/release/app
ldd target/release/app
otool -L target/release/app
dumpbin /dependents target/release/app.exe

# Platform-sensitive inventory
rg -n '/tmp|/bin/sh|Command::new|canonicalize|to_string_lossy' src
```

```yaml
# CI matrix sketch
strategy:
  fail-fast: false
  matrix:
    include:
      - os: ubuntu-latest
        target: x86_64-unknown-linux-gnu
      - os: macos-latest
        target: aarch64-apple-darwin
      - os: windows-latest
        target: x86_64-pc-windows-msvc
```

## Related skills and review

- Pair with `compiler-engineering` for target-specific codegen and ABI.
- Pair with `security-audit-engineering` for path confinement, signing, and installer risk.
- Pair with `performance-benchmarking` before claiming performance parity across platforms.
- Reviewer checks: real platform evidence, CI matrix, path/process APIs, clean-install test, and honest support tiers.
