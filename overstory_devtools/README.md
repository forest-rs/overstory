# overstory_devtools

Snapshot adapters and model helpers for Overstory inspectors.

This crate deliberately stays between the target UI and any eventual devtools
frontend. It captures owned inspection snapshots, builds cheap lookup helpers,
and projects the retained element tree into outline rows. It does not borrow or
mutate the target `Ui`.

The longer-term direction is a protocol boundary that can work in-process first
and out-of-process later. Overstory owns domain snapshots; Spoor should carry
time-series telemetry, session envelopes, and transport.
