# overstory_node_graph_demo

`overstory_node_graph_demo` is a focused native showcase for an element-based
node graph built with Overstory, `understory_node_graph`, and
`understory_view2d`.

Run it with:

```sh
cargo run -p overstory_node_graph_demo
```

The demo currently exercises:

- graph data, validation, derived layout, and orthogonal edge routing from
  `understory_node_graph`;
- viewport pan and zoom behavior from `understory_view2d`;
- Overstory elements for nodes, ports, labels, hit targets, tooltips, and
  interactions;
- dragging nodes and previewing edge creation without a separate canvas-item
  object model.

The long-term goal is for this demo to become a visually rich stress case for
animation, retained layout, hit testing, compositor-backed layers, and graph
editing ergonomics.

## License

Licensed under either Apache-2.0 or MIT, at your option. See the repository root
for license details.
