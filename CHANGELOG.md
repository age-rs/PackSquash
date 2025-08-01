# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic
Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Compression

- The generated ZIP files may now include a customizable comment string via the
  new `zip_comment` option. This comment is limited to 65535 US-ASCII characters
  and must not contain some special character sequences used internally by the
  ZIP format for delimitation purposes. Also, it is guaranteed to be placed at
  the end of the ZIP file. (_Thanks to a Discord user who suggested this idea a
  long time ago!_)
  - While it is also possible to attach text notes to a ZIP file by adding a
    file with a well-known name to it (and that is required for non-text data,
    or content exceeding 65535 characters), comment strings are typically
    displayed more prominently in user interfaces, and easier for programs to
    access. This makes them potentially more suitable for important user-facing
    notices and file tracking metadata.
- PackSquash now features a dedicated optimization algorithm for compressed
  compound NBT tag files, such as structures found in data packs, that may
  slightly reduce their sizes.
  - Compressed NBT data is recompressed using Zopfli. If the result is larger
    than the original, PackSquash will automatically fall back to a
    metadata-removing Gzip structure rebuilding algorithm, which offers minimal
    space savings while ensuring that file sizes never increase.
  - A new file-specific option, `nbt_compression_iterations`, allows controlling
    the number of compression iterations.
- Introduced a `sort_json_object_keys` option for JSON files, allowing control
  over whether JSON object keys are sorted alphabetically.
  - This option is enabled by default, typically yielding a slight improvement
    in compression efficiency with a negligible performance impact.
  - However, in rare cases, sorting keys may be inappropriate: it can negatively
    affect compressibility, or break compatibility with mods that, contrary to
    JSON specification recommendations, depend on key ordering.
  - PackSquash is also capable of detecting some cases where sorting keys could
    lead to a exceedingly high resource usage, disabling key sorting accordingly
    no matter the value of this option.

### Changed

#### Compression

- The single-color texture downsizing optimization introduced in v0.4.0 is now
  disabled by default until the necessary features to better automatically
  determine its default value are implemented, as it caused issues with e.g.
  more common than expected single-color font textures.
- The `bad_entity_eye_layer_texture_transparency_blending` quirk is no longer
  enabled by default for packs targeting Minecraft 24w40a (1.20.2) or newer, as
  the [related bug](https://bugs.mojang.com/browse/MC-235953) has been fixed in
  newer versions. (While the bug was technically resolved in 24w39a, that
  snapshot shares its resource pack format version with earlier snapshots, so
  PackSquash can't distinguish between them.)
- Very large textures are now slightly less aggressively compressed by default
  when falling back to non-Zopfli compression. This adjustment improves
  execution time by 30% while increasing pack size by only 1%, based on tests
  with a small pack corpus, offering a more sensible default tradeoff.
  - In most cases, it is still possible to achieve the previous behavior by
    increasing `image_data_compression_iterations` to a value that maintains the
    same non-Zopfli compression level, while preventing Zopfli from being used.
- Updated OxiPNG, bringing the image compression improvements made upstream.

#### Distribution

- The official binaries have been slimmed down by not including code to show
  stack backtraces on panic, which never worked because such binaries don't
  contain the necessary debug symbol data.
- The official Linux binaries that rely on system `glibc` now use the more
  efficient [`DT_RELR` relocation
  format](https://rfc.archlinux.page/0023-pack-relative-relocs/), reducing their
  size by approximately 5%, with no drawbacks aside from requiring `glibc`
  version 2.36 or later.
- [SLSA](https://slsa.dev/) v1.0 attestations conforming to at least the level 2
  of the build track are now generated for the official PackSquash binary
  artifacts.
  - These attestations allow security-conscious users to assert that they were
    generated on GitHub Actions runners with a well-defined build process, and
    that they have not been tampered with since they were generated.
  - The [`gh attestation
    verify`](https://cli.github.com/manual/gh_attestation_verify) and
    [`slsa-verifier`](https://github.com/slsa-framework/slsa-verifier) tools can
    be used to verify these attestations with the generated provenance data.
  - The PackSquash Docker images include SLSA v0.2 attestations, generated by
    [Docker
    BuildKit](https://docs.docker.com/build/attestations/slsa-provenance/).
  - Due to the numerous technical advantages of SLSA attestations, in addition
    to their decentralized and free nature, the PackSquash project will most
    likely not pursue signing binaries with code signing certificates.

#### Performance

- The statically-linked PackSquash CLI Linux binaries now use the
  [`mimalloc`](https://github.com/microsoft/mimalloc) memory allocator, instead
  of the default `musl` C library allocator.
  - This brings the performance of such binaries roughly in line with those
    packaged for Debian, which dynamically link against `glibc`. Previously,
    slowdowns of 5x or more could be expected, depending on the number of
    threads used.
  - Because they use statically-linked binaries, the PackSquash GitHub
    Action and Docker container also saw performance improvements. (Thanks
    _@xMikux_ for reporting the performance differences!)
- The Zopfli compressor, used for DEFLATE compression of textures and other
  files in the generated ZIPs, has been significantly optimized.
  - In tests with the `recompress_compressed_files` option enabled (and all
    other options set to their default values) on a sample realistic pack,
    execution time improved by 15%.
- Updated `libspng` to v0.7.4, bringing decoding speed and stability
  improvements for ARM CPUs that support NEON extensions.

#### Protection

- Texture files can now be protected to make them harder to view outside of
  Minecraft via the new file-specific `png_obfuscation` option. This protection
  is independent of the already available ZIP layer protection, so it can be
  used alongside it or not, and can be applied to a subset of pack files.
  - This protection will not work for resource packs targeting Minecraft 1.12.2
    or older. By default, PackSquash will force it to be disabled for such
    versions via the new `png_obfuscation_incompatibility` quirk.
- PackSquash now adds an extra layer of protection when
  `size_increasing_zip_obfuscation` is enabled on a small subset of pack files,
  as far as it safe to do so due to the inner workings of Minecraft. (Thanks to
  a Discord user for bringing this idea to my attention)
  - Select textures may optionally be more protected by changing the new
    `may_be_atlas_texture` PNG-specific option, but it is advised that you only
    do this if you have detailed knowledge of how the game processes textures,
    as otherwise the game may not load the pack correctly.
- The `ogg_obfuscation_incompatibility` quirk now applies by default to resource
  packs targeting all Minecraft versions from snapshot 24w13a (1.20.5) onward.
  This is due to an internal change introduced in Minecraft 1.20.5-pre1 which
  broke compatibility with files generated using the feature. As a result, the
  `ogg_obfuscation` option no longer has any effect for most packs that target
  up-to-date game versions. (Thanks _@pau101_ and _@mrkinau_ for bringing this
  topic to my attention!)
  - Any packs containing affected audio files that need to work with Minecraft
    1.20.5-pre1 or later should be reprocessed to remove this protection, as the
    latest game versions can no longer play these files.

#### Fixed

- Packs generated with the `zip_spec_conformance_level` option set to
  `disregard` now reliably work on Minecraft clients running Java 22 or newer.
  (Thanks _@mrkinau_ and _@mihannnik_ for reporting the issue!)
- Data packs targeting Minecraft 24w14a (1.21) or newer no longer have their
  functions and structure `.nbt` files skipped by default.
  ([#327](https://github.com/ComunidadAylas/PackSquash/issues/327))
- Shaders that depend on `#moj_import`ed or parent-defined preprocessor
  variables to be syntactically correct or expand to the intended source code
  will no longer cause PackSquash to fail or change their semantics.
  - Since PackSquash still can't resolve `#moj_import` directives, this fix came
    at the cost of falling back to not doing any source transformations
    (minification, transformation) when this directive is used. Note that
    PackSquash will never be able to resolve these directives in the general
    case, because shaders can import shaders from other packs, including the
    default game pack, which PackSquash doesn't have access to.
  - If PackSquash cannot parse a shader, the parsing error is now considered
    tentative, causing a fallback to no source transformation, if the shader
    contains a `#moj_import` directive, as it cannot be ruled out that the
    shader source would be valid if the `#moj_import` is expanded.
  - In the future, we look forward to improve PackSquash `#moj_import` expansion
    capabilities. At least, we should be able to better handle the common case
    of imports of other shaders from the same pack.
- Include shaders now do not need to be parsable as translation units,
  statements or expressions. Failure to parse them as such will result in a
  tentative syntax error to be shown, but such an error will not be fatal and
  PackSquash will fall back to no source transformation.
- Include shaders consisting of a list of statements no longer have only their
  first instruction transformed when minifying or prettifying, which broke the
  semantics of the shader code.
- Fragment, vertex, and include shaders located outside
  `assets/minecraft/shaders/core` and `assets/minecraft/shaders/program` are now
  processed, enhancing compatibility with resource packs targeting Minecraft
  24w34a (1.21.2) or newer. (Thanks _@swrds_ for reporting this issue!)
- The UTF-8 BOM is no longer automatically stripped from properties files, as
  they should not be encoded in UTF-8 to begin with, and carrying on processing
  with mismatched encodings may cause mojibake.
- Processing input PNG files with colors in palette format should no longer
  sometimes cause ARM binaries to crash on CPUs that support NEON extensions.
  (Thanks _@lucian929_ for reporting the issue!)
- Font files for the `ttf` font provider may now have extensions other than
  `.ttf`, namely `.otf`, `.ttc`, and `.otc`, for Minecraft 1.13 onwards, when
  support for OpenType format extensions was added.
- ZIP files containing `.hex` files for `unihex` font providers, introduced in
  snapshot 23w17a (Minecraft 1.20), are now copied over by default for resource
  packs targeting supporting client versions.
- The `postcredits.txt` file at `assets/minecraft/texts` is now correctly
  processed as a credits text file for packs targeting 1.18-pre2 or later.
  ([#333](https://github.com/ComunidadAylas/PackSquash/issues/333))
- JSON files with arrays and objects nested beyond 128 levels deep can now be
  processed. (Thanks _@ic22487_ for reporting this issue!)
  - While such deeply nested JSON structures pose interoperability problems,
    they have valid uses when creatively scaling certain vanilla resource pack
    features, as some game distributions support them.
- Function files in data packs with filenames consisting only of their extension
  are no longer incorrectly ignored. (Thanks _@ChenCMD_ for reporting this issue
  and providing a fix!)

#### User experience

- Some options file deserialization error messages have been shortened and made
  aware of the context in which they appear to suggest likely helpful corrective
  actions.
- Migrated embedded help links from Firebase Dynamic Links to the project
  website under its own domain to ensure continued functionality after Firebase
  Dynamic Links is discontinued on August 25, 2025.

#### Security

- PackSquash now sanitizes file modification times for storage as Squash Times
  in the generated ZIP files with a stronger AES-256 encryption algorithm.
- The encryption key length has been increased from 128 bits to 256 bits. This
  key is now derived using the HKDF-SHA256 algorithm applied to all collected
  non-volatile system IDs, unless a specific system ID is set via the
  `PACKSQUASH_SYSTEM_ID` environment variable. If only volatile system IDs are
  available, the key is derived from those instead. This updated scheme aligns
  more closely with documented modern best practices for entropy-based key
  derivation.
- On Linux targets, DMI product UUIDs and serial numbers are now included in the
  candidate system IDs, potentially bringing the entropy of the derived
  encryption keys to parity with Windows.

#### Documentation

- The PackSquash logo has been updated with a [KawaiiLogos-style
  design](https://github.com/SAWARATSUKI/KawaiiLogos/blob/main/README_EN.md).
  (Thanks to _@amberisfrozen_ for your beautiful original artwork!)

### Removed

#### Internal

- Dropped build-time dependencies on `time` and `git2` in favor of gathering
  version metadata with more lightweight methods.

## [0.4.0] - 2023-06-25

### Added

#### Compression

- Added a single-color texture downsizing optimization to make textures composed
  of pixels of a single color as small as possible while maintaining their
  maximum mipmap level. (Thanks to _@alumina6767_ for bringing this idea to my
  attention!)
  - A new `downsize_if_single_color` file-specific option has been added to
    control whether this optimization is applied or not.
  - Downsizing single-color textures can have negative side effects if such
    textures are used with custom shaders or custom fonts. We are aware of such
    limitations and look forward to making PackSquash handle such cases better
    in the future, but feel free to let us know if you are affected by them: it
    can be useful to assess the impact of these limitations and discover new
    ones we were not aware of.
- The macOS resource fork ZIP folder (i.e., the `__MACOSX` folder) has been
  added to the set of files and folders ignored by PackSquash when
  `ignore_system_and_hidden_files` is set to `true` (the default value).

#### Distribution

- macOS binaries are now universal, containing native code for both Intel and
  Apple Sillicon based devices.
  ([#41](https://github.com/ComunidadAylas/PackSquash/issues/41))
  - As a result, newer macOS devices with Apple Sillicon (e.g., ARM) CPUs should
    run PackSquash faster.
  - The performance on Intel-based devices should not be affected by this
    change, as the universal binaries still contain optimized x86 code.
- Published PackSquash Docker images at the GitHub Container Registry.
  ([#111](https://github.com/ComunidadAylas/PackSquash/pull/111), thanks
  _@realkarmakun_ for your PR!)

#### Protection

- Audio files can now be protected to make them harder to play outside of
  Minecraft via the new file-specific `ogg_obfuscation` option. This protection
  is independent of the already available ZIP layer protection, so it can be
  used alongside it or not, and can be applied to a subset of pack files.
  - This protection will not work for resource packs targeting Minecraft 1.13.2
    or older. By default, PackSquash will force it to be disabled for such
    versions via the new `ogg_obfuscation_incompatibility` quirk.

#### Documentation

- Included a contributors section in the project readme, following the [All
  Contributors
  specification](https://github.com/all-contributors/all-contributors).
- Created a changelog file.
- Added a repository pull request template.
- Added a GitHub Sponsors button to the repository.

#### Internal

- Configured [Renovate](https://docs.renovatebot.com/) for automated dependency
  updates.
- Added GitHub Codespaces configuration files to allow interested parties to
  quickly spin up an environment suitable for PackSquash development.

### Changed

#### Compression

- Reworked the audio file processing code to be much faster and more effective.
  This is a major change that included the following smaller changes:
  - Removed the dependency on GStreamer in favor of combining several
    specialized libraries to accomplish the necessary audio processing tasks.
    This greatly samplifies PackSquash distribution and installation, removes
    points of failure related to the availability of GStreamer components, and
    makes the audio processing code more efficient. It also enabled the use of
    the audio processing software mentioned below.
  - Integrated [OptiVorbis](https://github.com/OptiVorbis/OptiVorbis), a novel
    solution for lossless optimization and validation of the generated Ogg
    Vorbis files.
  - Migrated to [`vorbis_rs`](https://github.com/ComunidadAylas/vorbis-rs), a
    novel set of bindings for the best-in-breed encoder available, based on the
    reference Vorbis encoder implementation and the aoTuV and Lancer patchsets.
  - The default encoder bitrate management parameter selection has been
    optimized for much better performance and space efficiency without
    compromising audio quality.
  - Empty audio files (i.e., containing only complete silence) are now replaced
    with a minimal Ogg Vorbis file with no audio data, which significantly
    minimizes their size with no adverse effects for the vast majority of
    purposes. This optimization can be disabled if needed via the new
    `empty_audio_optimization` option.
  - The default encoder bitrate management parameters and sampling frequency are
    now different for mono and stereo sounds. The rationale for this change is
    that mono sounds tend to be shorter and mixed with other world sounds, so a
    drop in quality is less noticeable in them than in persistent, music-like
    stereo sounds.
  - PackSquash now falls back to outputting the input audio file if resampling
    did not help to reduce the file size, as long as channel mixing or pitch
    shifting are not requested. This input audio file will be optimized (and
    protected if requested) if the `two_pass_vorbis_optimization_and_validation`
    option is enabled (its default value).
- Revised shader processing code to partially fix long-standing preprocessor
  directive support issues. (Related issue:
  [#187](https://github.com/ComunidadAylas/PackSquash/issues/187))
  - Switched to a new GLSL preprocessor and parser, which is much faster (its
    author claims about 10x faster) and allows to accept files containing
    preprocessor directives outside of external declaration position by
    expanding them before parsing.
  - `minify_shader` has been superseded by a new
    `shader_source_transformation_strategy` option, which now also allows
    prettifying shader source code.
  - Include shaders (i.e., shaders with a `.glsl` extension) can now be
    standalone statements and expressions.
  - Properly optimizing shaders in the context of Minecraft is an interesting
    and complex problem that we are working on. Even with this partial fix, you
    may find that some shaders still do not work as-is with PackSquash. For more
    information on the known shortcomings and current state of affairs, please
    read [this GitHub issue
    comment](https://github.com/ComunidadAylas/PackSquash/issues/187#issuecomment-1499365532).
- Some internal PNG compression settings have been tweaked for better
  performance and compression on average. In addition, PackSquash now uses the
  cutting-edge `oxipng` raw pixel API, which allows PackSquash to avoid encoding
  intermediate textures.

#### Fixed

- PackSquash no longer fails to process some audio files when using the default
  options. This is a consequence of the new default encoder bitrate management
  parameter selection.
- Audio files with more than two audio channels are no longer accepted, as they
  don't work with Minecraft.
- Fixed "write zero" I/O errors happening for every pack file when the spooling
  buffer size is set to zero. (Thanks to _darkchroma_ for reporting this issue
  on Discord)
- PackSquash now accepts legacy font `.bin` files anywhere in a resource pack,
  like Minecraft does.
- Fixed D-Bus machine ID fallback lookups not working as intended on Linux
  platforms.

#### Distribution

- AppImages have been dropped in favor of statically-linked binaries as a
  distribution method for Linux platforms. Due to the removal of the dependency
  on GStreamer, AppImages are no longer considered a better overall means of
  distro-agnostic distribution. We don't expect the PackSquash CLI to depend on
  external libraries and thus switch back to AppImages anytime soon, but the
  future GUI may benefit from them.

#### User experience

- The options file deserialization error messages have been made more verbose so
  that they are more useful for troubleshooting syntax errors.
- PNG files with unnecessary trailing bytes at the end are no longer rejected;
  instead, such bytes are now silently discarded. These files do not follow the
  PNG specification and may cause interoperability issues, but experience has
  shown that popular closed-source programs (e.g., some versions of Photoshop)
  generate these files, and users can have a cumbersome time identifying and
  fixing them.
  - To achieve a better balance between problem linting capabilities and
    usability, future versions of PackSquash will display a non-fatal warning
    for the troublesome files, so that pack authors can improve the technical
    quality of their pack if they want to.

#### Documentation

- The PackSquash logo has been redesigned to give it a more modern, cleaner
  look. (Thanks to _@MiguelDreamer_ for your work on this!)
- The repository README and other pieces of documentation have been updated and
  proofread.
  - This included updating the Discord username of the main PackSquash author,
    since Discord is phasing out username discriminants.
- Revamped the repository issue templates.

#### Internal

- Slightly optimized the space efficiency of the repeated entry lookup in legacy
  language files by using [radix
  trees](https://en.wikipedia.org/wiki/Radix_tree) instead of hashmaps.
- Renamed the `packsquash-cli` package to `packsquash_cli` to better align with
  [documented Rust crate naming
  conventions](https://github.com/rust-lang/api-guidelines/discussions/29).
- Lots of third-party dependency updates, including prominent libraries such as
  `imagequant`, `oxipng` and `zopfli`. In particular, several `oxipng` updates
  brought significant performance and compression improvements.
  - Some dependency changes addressed minor public security advisories.
- Replaced some unmaintained CI step actions with faster, more up-to-date
  equivalents.
- The official PackSquash web presence has been moved to the `aylas.org` domain,
  under the easy to remember subdomain `packsquash.aylas.org`.
  - We were able to afford this domain, for which we pay 10.1 €/year, thanks to
    our financial contributors.

#### Options

- The following options were removed from options files:
  - `open_files_limit`
  - `minimum_bitrate` (superseded by `target_bitrate_control_metric`)
  - `maximum_bitrate` (superseded by `target_bitrate_control_metric`)
  - `minify_shader` (superseded by `shader_source_transformation_strategy`)
- The following options were added to options files:
  - `two_pass_vorbis_optimization_and_validation`
  - `empty_audio_optimization`
  - `bitrate_control_mode`
  - `target_bitrate_control_metric`
  - `ogg_obfuscation`
  - `downsize_if_single_color`
  - `shader_source_transformation_strategy`
  - `is_top_level_shader`
- The `ogg_obfuscation_incompatibility` quirk has been added to the set of
  quirks accepted by the `work_around_minecraft_quirks` option.

### Removed

#### Performance

- The `open_files_limit` option has been removed because it was hard to use
  properly. PackSquash will instead automatically try to increase the open files
  limit when needed, and fall back to using fewer threads if the maximum
  attainable limit would not support the desired level of concurrency.

#### Internal

- Dropped build-time dependency on `vergen` in favor of directly gathering
  version metadata from Git during build scripts.
- Dropped the dependency on `atty` in favor of new Rust standard library
  methods.
- Removed pretty panic printing logic. With GStreamer out of the equation,
  panics should be much less likely to happen, so the significant binary size
  taken by this logic was no longer warranted.

[Unreleased]: https://github.com/ComunidadAylas/PackSquash/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/ComunidadAylas/PackSquash/compare/v0.3.1...v0.4.0
