# Runtime features

This page describes the behaviour of the emulation loop and the user interface
that goes beyond a plain instruction-by-instruction interpreter.

## Threaded emulation

The emulated machine runs on its own worker thread and the interface reads a
mirror of the machine (`src/fil.rs`). Drawing, audio and window events no longer
share a thread with the interpreter, so a slow frame in the interface does not
slow the console down.

The original single-threaded path is kept and is used automatically whenever
something needs direct access to the machine: the inspection panel, an open
context menu, a serial link and the local server. In those cases the worker
thread is stopped and the machine is handed back.

## Idle-loop fast-forward

The firmware spends most of its time waiting: it does a frame of work, then
spins until the next display or SysTick signal. The core recognises such a loop
and advances the clock instead of interpreting it.

A loop is only skipped when all of these hold together: a short backward
branch, no store during the whole iteration, and registers strictly identical
to the previous pass. Peripherals are still serviced at their usual granularity,
and the skip stops as soon as an interrupt can be taken (or after a bounded
number of cycles for waits that end on a plain polled-register change).
`WFI` is handled the same way.

**This changes the default behaviour of the emulator**: the same program now
reaches the same states, but the moment at which the firmware notices an event
can differ by up to the maximum skip length. The gain is a much lower CPU load.

Step-exact behaviour is available when you need it:

* automatically, while a serial link is open (a transfer protocol on the other
  end of a wire counts milliseconds);
* from code and tests, with `Cpu::step_exact()`, which turns the fast-forward off
  for good. `tests/features_tests.rs` compares both modes on a polling loop.

## Keep awake

The firmware counts idle time in a half-word and shuts the console down when it
reaches a threshold. The "keep awake" entry of the menu clears that count once a
second of console time, which is what a button press does, without the press.
Nothing is forced and no scene is short-circuited. The setting is off by
default and is saved with the game. The address used is the one measured on the
edition this was developed on; another edition may count elsewhere.

When the count is not known, a fallback catches the console after the fact by
returning from the sleep routine; it only delays the shutdown.

## Recovery points

Recovery points (one a minute, thinned out with age, dropped after twelve hours)
are written by a separate thread, so taking one no longer stops the console.
A point can be **locked**: it then escapes pruning and the age limit and stays
until it is deleted by hand (the interface asks for confirmation first). The
lock is stored in the index; indexes written by earlier versions load with
nothing locked. The rewind history keeps a snapshot every two seconds of console time.

## Scene numbers

The scene number shown in the interface is read from the scene descriptor
itself instead of being deduced from the entry's position in the table. On
editions whose first descriptor has an unreadable name the old rule shifted
every name by one (for example the home screen and the home-field screen were
swapped).
