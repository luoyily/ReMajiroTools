# ReMajiroTools
ReMajiro's extension tools

Developer tools for [ReMajiro](https://github.com/luoyily/ReMajiro), a Rust implementation of the Majiro Engine. The binaries consume the `formats` and `vm` crates directly from the ReMajiro repository (pinned to tag `v0.1.0`), so the repo builds entirely on its own:

```sh
cargo build --release
```

Binaries land in `target/release/`.

## mjo-arc — archive extractor

Extracts entries from Majiro Engine `.arc` archives.

```sh
mjo-arc -i data.arc               # extract into ./output/data/
mjo-arc -i data.arc -l            # list entries only
mjo-arc -i data.arc -o ./out -v   # verbose extraction
```

| Flag | Meaning |
| --- | --- |
| `-i, --input <INPUT>` | Input `.arc` file |
| `-o, --output <OUTPUT>` | Output directory (default `./output`) |
| `-l, --list` | List entries only, without writing files |
| `-v, --verbose` | Verbose output |
| `--check` | Validate the archive index and entry ranges without loading file data |
| `-j, --decode-sjis` | Decode Shift-JIS filenames (default: auto) |

## mjo-ir — script IR converter

Converts Majiro `.mjo` scripts to a readable text IR and back. Its main purpose is producing translation annotations: every `TEXT_RENDER` line in the IR can carry an `@tr from -> to` tag, and the runtime applies these tags at startup without touching the original script bytes.

```sh
mjo-ir export -i start.mjo -o start.ir   # decode .mjo -> readable IR text
```

To translate a script, add an `@tr` line under the matching `TEXT_RENDER` instruction (by hand or scripted), then ship the `.ir` file in the patch (see below).

Two more subcommands exist mostly for development and testing: `import` re-assembles edited IR back into a `.mjo`, and `localize` bulk-attaches translations from an offset-keyed JSON catalog as `@tr` tags:

```sh
mjo-ir localize -i start.mjo -c text/start.json -o start.ir
```

### Patch workflow

1. `mjo-ir export -i start.mjo -o start.ir` to get readable IR.
2. Add an `@tr from -> to` line under each `TEXT_RENDER` you want to translate.
3. Drop the finished `<script>.ir` into the patch tree under `text/`, named after the script it patches. The engine mounts it at startup and applies the `@tr` translations over the original script.

## rct2png — image converter

Converts Majiro Engine `.rct` / `.rc8` images to PNG.

| Flag | Meaning |
| --- | --- |
| `-i, --input <INPUT>` | Input image file |
| `-o, --output <OUTPUT>` | Output PNG path |
| `-I, --info` | Print image metadata only |
| `-b, --batch` | Batch-convert a directory |
| `-f, --force` | Overwrite existing outputs |
| `-v, --verbose` | Verbose output |
| `--raw` | Export raw pixels: skip alpha-mask merging and export mask files too |
| `--check` | Decode and validate input without writing PNG files |

## Patch format

The engine mounts an optional patch tree from the game directory (the shallowest `patch.toml` found selects the patch root). Layout:

```text
patch/
├── patch.toml
├── text/                  # per-script text catalogs (<script>.json) and/or IR scripts (<script>.ir)
├── images/                # replacement PNGs keyed by the logical resource name
├── files/                 # generic file overrides, matched by game-relative path
└── fonts/                 # font files referenced from [localization]
```

`patch.toml`:

```toml
format = 1                      # manifest format, must be 1
id = "my-translation"           # optional; defaults to the patch folder name

[localization]
locale = "zh-Hans"              # optional
fonts = ["fonts/my-font.ttf"]   # optional, patch-relative paths

[presentation]
scale = 2                       # integer presentation scale
output_width = 2560             # optional output size override
output_height = 1440
```

Development/testing helper: the patch tree can alternatively hold offset-keyed JSON catalogs at `text/<script>.json`, one per script, named after the script file stem. Entries are keyed by the hex offset of the script's `TEXT_RENDER` instruction:

```json
{
  "format": 1,
  "code_crc32": "0x1A2B3C4D",
  "messages": {
    "0x4704": { "source": "「ハッピーバースデイ、兄さん」", "text": "「生日快乐，哥哥」" }
  }
}
```

`code_crc32` is an optional guard (number or hex string) verifying the catalog against the exact script build. `source` is optional context metadata; only `text` is applied.

Image overrides are plain PNGs under `images/`, named after the logical resource name of the image they replace — no raw or auxiliary format is needed, transparency included (the alpha channel is used as-is). They are layered on top of the original data at the patch's integer presentation scale, so an upscaled set only needs to cover the images worth replacing.

## Script IR text and `@tr` annotations

The IR text is the engine's human-editable script representation. The excerpt below shows the annotation style: a disassembly listing where each translated `TEXT_RENDER` carries an `@tr from -> to` annotation. The runtime applies `@tr` at startup — original script bytes are never mutated:

```text
0x0046CE: 0x0840 text "イリ＠入莉\0" ; TEXT_LINE
0x0046DD: 0x083A 0x02F1 ; SYS_06
0x0046E1: 0x0840 text "「ハッピーバースデイ、兄さん」\0" ; TEXT_LINE
0x004704: 0x0841 ; TEXT_RENDER
@tr from "「ハッピーバースデイ、兄さん」" -> "「生日快乐，哥哥」"
0x004706: 0x0842 hex 70 00 ; SYS_0E
0x00470C: 0x0842 hex 77 00 ; SYS_0E
0x004712: 0x083A 0x02F3 ; SYS_06
0x004716: 0x0801 text "3iri_0484_1\0" ; PUSH_STR
0x004726: 0x0810 0x812AFDF0 0x00000000 1 ; CALL_LOCAL
0x004732: 0x083A 0x02F4 ; SYS_06
0x004736: 0x0840 text "イリ＠入莉\0" ; TEXT_LINE
0x004745: 0x083A 0x02F5 ; SYS_06
0x004749: 0x0840 text "「あなたの誕生日と、\0" ; TEXT_LINE
0x004762: 0x0841 ; TEXT_RENDER
@tr from "「あなたの誕生日と、" -> "「你的生日，"
0x004764: 0x083A 0x02F6 ; SYS_06
0x004768: 0x0810 0xD175DFD4 0x00000000 0 ; CALL_LOCAL
0x004774: 0x083A 0x02F8 ; SYS_06
0x004778: 0x0801 text "3iri_0484_2\0" ; PUSH_STR
0x004788: 0x0810 0x812AFDF0 0x00000000 1 ; CALL_LOCAL
0x004794: 0x083A 0x02F9 ; SYS_06
0x004798: 0x0801 text "入莉\0" ; PUSH_STR
@tr from "入莉" -> "入莉"
0x0047A1: 0x0810 0x8E803141 0x00000000 1 ; CALL_LOCAL
0x0047AD: 0x083A 0x02FB ; SYS_06
0x0047B1: 0x0840 text "あなたのこれからの未来が、\0" ; TEXT_LINE
0x0047D0: 0x0841 ; TEXT_RENDER
@tr from "あなたのこれからの未来が、" -> "你今后的未来，"
0x0047D2: 0x0842 hex 6E 00 ; SYS_0E
0x0047D8: 0x083A 0x02FC ; SYS_06
0x0047DC: 0x0810 0xD175DFD4 0x00000000 0 ; CALL_LOCAL
0x0047E8: 0x083A 0x02FF ; SYS_06
0x0047EC: 0x0801 text "入莉\0" ; PUSH_STR
@tr from "入莉" -> "入莉"
0x004815: 0x0810 0x8E803141 0x00000000 1 ; CALL_LOCAL
0x004821: 0x083A 0x0302 ; SYS_06
0x004825: 0x0840 text "\u{3000}幸せなものでありますように――」\0" ; TEXT_LINE
0x00484C: 0x0841 ; TEXT_RENDER
@tr from "\u{3000}幸せなものでありますように――」" -> "\u{3000}愿它们充满幸福——」"
```

## Disclaimer

This project is provided for learning and research purposes only. It must not be used for commercial purposes. The developers are not affiliated with any game publisher or rights holder, and assume no responsibility for any actions taken by users of this software.
