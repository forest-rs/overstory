# Chicago — Design Tokens

Source of truth for the Chicago palette — a flat, beveled, square-cornered
homage to the mid-90s desktop. Parsed by `overstory_look_chicago::design_md`
into [`Seeds`]; the Rust defaults in `palette.rs` mirror this table, and a test
fails if they drift.

There is no motion table: period UI did not animate, so Chicago's durations are
fixed at zero.

## Colors

Hex is `#rrggbb` or `#rrggbbaa`. `light` is the classic gray chrome; `dark` is a
darker variant.

| role        | light     | dark      |
| ----------- | --------- | --------- |
| face        | #c0c0c0   | #3a3a3a   |
| window      | #ffffff   | #1e1e1e   |
| bevel-light | #ffffff   | #6a6a6a   |
| bevel-dark  | #808080   | #000000   |
| accent      | #000080   | #283a8c   |
| text        | #000000   | #f0f0f0   |
| muted       | #808080   | #a0a0a0   |
| on-accent   | #ffffff   | #ffffff   |

[`Seeds`]: https://docs.rs/overstory_look_chicago/latest/overstory_look_chicago/palette/struct.Seeds.html
