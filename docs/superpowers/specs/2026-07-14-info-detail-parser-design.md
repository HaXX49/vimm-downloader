# Current Detail-Page Parsing Design

## Problem

Vimm's current `/vault/{id}` markup differs from the HTML captured for the
original detail parser. The current page has no `h1` or `.vaultTable`: its title
is exposed through Open Graph metadata and a base64 canvas, the system appears
in navigation, and metadata uses an unclassed three-column table. Ratings are
separate rows rather than one combined row. As a result, `info` retains media
JSON but prints empty strings and numeric defaults for most game metadata.

The media JSON also changed shape. `GoodDate` replaced `VerifiedDate`, format
options use numeric alt values, and `Zipped`, `AltZipped`, and `AltZipped2` are
scalar KiB values. The existing parser treats the primary value as bytes and
does not associate alternative sizes and labels correctly.

## Design

### Compatible metadata parsing

The detail parser will support both current and legacy pages. It will obtain:

- the title from `meta[property='og:title']`, falling back to `h1` and then the
  base64 title canvas;
- the system slug from the active `/vault/{slug}` submenu link;
- the current metadata table by locating the table containing the download
  version selector, while retaining `.vaultTable` support;
- regions from flag `title` attributes, and players/year from row values;
- current ratings from the separate Graphics, Sound, Gameplay, and Overall
  rows, including the vote count, while retaining the combined legacy parser;
- the verified date from `#data-date`, legacy metadata, or media JSON;
- the game serial from page metadata or the selected/first media entry.

Unavailable data remains represented by the existing empty-string or zero
sentinels in `GameDetail`, preserving its public Rust and serialized JSON shape.
The human CLI will render those sentinels as `N/A`. Publisher will therefore be
shown as unavailable when the live page does not provide it.

### Media and format mapping

Media deserialization will accept legacy `VerifiedDate` and current `GoodDate`,
normalizing either to `YYYY-MM-DD`. Size fields will continue accepting null,
string, number, and array representations.

For current pages, the three size properties correspond to alt values 0, 1,
and 2 and contain KiB, so they will be multiplied by 1024 before populating
`Format::zipped_size_bytes`. Legacy array-shaped fixtures will remain supported
by indexing the primary size array when it contains one value per mirror.

Format options will be associated by order/alt value rather than comparing the
numeric HTML option value to a mirror name. Stable user-facing keys will come
from normalized option labels (`ciso`, `nkit.iso`, `rvz`), preserving config and
`--format` behavior. Single-format pages without a selector will retain their
mirror-derived key and label.

### CLI output

Human `info` output will use a shared unavailable-value formatter for empty
strings, a zero year, and an entirely absent rating. Archive sizes will be
formatted using binary thresholds with the existing `KB`, `MB`, and `GB`
labels, so 6632 KiB is displayed as approximately `6.48 MB`. JSON output will
remain the unchanged `GameDetail` serialization.

## Testing

- Add a sanitized Pokemon Emerald fixture representing the current single-
  format markup and assert title, `GBA`, both regions, players, year, four
  ratings, votes, verification date, and the corrected byte size.
- Add a sanitized current GameCube fixture and assert stable format keys,
  labels, descriptions, alt values, and all three corrected sizes.
- Keep the legacy single- and multi-format fixtures passing; update their
  expected sizes to reflect the documented KiB-to-byte conversion.
- Add CLI formatter tests covering `N/A` and KB/MB/GB presentation.
- Run `cargo test` and `cargo clippy -- -D warnings` offline.

## Compatibility and Scope

No public types or command-line arguments change. No additional live request is
introduced, and automated tests remain network-free. Checksums and other newly
visible site fields remain outside this bugfix.
