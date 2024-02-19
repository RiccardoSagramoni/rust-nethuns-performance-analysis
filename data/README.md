# Tests

- `2023-11-06`: test con config originale di Nethuns (packetsize=2048, numpackets=4096)
- `2023-11-13`: test con config originale di Nethuns + LTO **fat**
- `2023-11-14`: test con config originale di Nethuns + LTO **thin**
- `2023-11-17`: test con nuova configurazione socket (packetsize=0, numpackets=1024) + rimozione Arc/HybridRc come wrapper attorno allo status flag atomico.


## Plot generation

enp recv

```rust
fn generate_layout(title: &str) -> Layout {
    Layout::new()
        // .height(500)
        // .width(500)
        .y_axis(
            Axis::new()
                .title(
                    Title::new("Throughput (Mpps)").font(Font::new().size(16)),
                )
                .color("black")
                .range(vec![14.6, 15.1])
                .dtick(0.05),
        )
        .x_axis(Axis::new().color("black").tick_font(Font::new().size(16)))
        .show_legend(false)
        .title(
            Title::new(format!("<b>{title}</b>").as_str())
                .font(Font::new().color("black").size(24)),
        )
        // .box_gap(0.5)
}
```

vale recv

``` rust
fn generate_layout(title: &str) -> Layout {
    Layout::new()
        // .height(500)
        // .width(500)
        .y_axis(
            Axis::new()
                .title(
                    Title::new("Throughput (Mpps)").font(Font::new().size(16)),
                )
                .color("black")
                .range(vec![22, 24])
                .dtick(0.2),
        )
        .x_axis(Axis::new().color("black").tick_font(Font::new().size(16)))
        .show_legend(false)
        .title(
            Title::new(format!("<b>{title}</b>").as_str())
                .font(Font::new().color("black").size(24)),
        )
        // .box_gap(0.5)
}
```

enp send

```rust
fn generate_layout(title: &str) -> Layout {
    Layout::new()
        .y_axis(
            Axis::new()
                .title(
                    Title::new("Throughput (Mpps)").font(Font::new().size(16)),
                )
                .color("black")
                .range(vec![14.25, 14.75])
                .dtick(0.05),
        )
        .x_axis(Axis::new().color("black").tick_font(Font::new().size(16)))
        .show_legend(false)
        .title(
            Title::new(format!("<b>{title}</b>").as_str())
                .font(Font::new().color("black").size(24)),
        )
}
```
