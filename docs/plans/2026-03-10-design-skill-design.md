# Design Skill

## Overview

Standalone skill for UI/UX design. Invoked when you need to design how something should look, feel, and behave before implementation. Can follow brainstorming or be invoked directly. Ends by invoking writing-plans.

## Design Philosophy

### General

**Beauty is alignment.** Form and function so matched the result feels inevitable.

**Elegance is complexity made invisible.** The user feels ease, never the hard problem underneath.

**Craft is invisible detail.** Spacing, transitions, alignment — details nobody notices consciously, but everyone feels. That feeling is trust.

**Restraint is courage.** Stop before the design starts explaining itself.

**Timelessness over trendiness.** Proportion and hierarchy don't age. Trends do.

### The Controller

**Calm control.** Orchestrating agents should feel like conducting — powerful, composed, unhurried. The interface makes complex orchestration feel simple, not hide the complexity.

**The tool disappears.** The best state is when you forget you're using an interface and just work.

**Terminal-native, not terminal-cosplay.** Dense, keyboard-first, no hand-holding. But what terminals would look like if redesigned today.

## Process

1. **Define the feeling** — One sentence. What should this feel like to use? This is the north star.
2. **Research** — Find 2-3 apps/tools that solve a similar UX problem. For each: what works, what doesn't, why.
3. **Derive the design** — From feeling + research, make specific decisions:
   - Layout and spatial relationships
   - Information hierarchy (what gets attention first, second, third)
   - Interaction model (keyboard/mouse, transitions)
   - Visual treatment (Catppuccin Mocha tokens, typography, spacing)
   - States (empty, loading, error, populated, edge cases)
4. **Critique loop** — Challenge each decision:
   - Does this serve the feeling?
   - Is there a simpler way?
   - What would you remove and still have it work?
   - Does it feel cohesive with the rest of the app?
5. **Present design** — Section by section, get approval.
6. **Write design doc** — Save to `docs/plans/YYYY-MM-DD-<topic>-design.md`, commit.
7. **Invoke writing-plans**.

## Design Lenses

When making any design choice, run it through these:

- **Eye movement** — Where does the eye land first? Is that the most important thing?
- **Negative space** — Is the emptiness intentional? Does it create grouping, breathing room, or focus?
- **Visual weight** — Which elements feel heavy? Does the weight distribution match the information hierarchy?
- **Contrast as communication** — Color, size, and weight differences tell the user what matters. Is the contrast saying the right things?
- **Edge cases as design inputs** — 1 item? 50 items? 200-character name? An error? These reveal whether the design works.
- **Motion as meaning** — Every animation answers "what just happened?" If it doesn't communicate state change, remove it.
- **Information density vs. cognitive load** — Density works when there's clear hierarchy. Dense + flat = overwhelming. Dense + structured = powerful.
- **The glance test** — If someone sees this for half a second, what do they understand?
- **Consistency as trust** — Same spacing, colors, patterns repeating predictably. The user stops thinking about the interface and starts trusting it.

## Principles

1. **Feeling is the north star** — Every decision traces back to the feeling statement.
2. **Research before inventing** — Always look at how others solved it first.
3. **Coherence with the whole** — New designs must feel like they belong in The Controller.
4. **Remove until it breaks** — Don't ask "is this simple enough?" Ask "what happens if I remove this?" If nothing, remove it.
5. **Specify or it didn't happen** — "Clean layout with good spacing" is worthless. Say which elements, what spacing, which tokens.

## Hard Gates

1. Do NOT skip research. Even for small designs.
2. Do NOT skip the critique loop.
3. Do NOT produce implementation code. Writing-plans handles implementation.
4. Do NOT present a design without a feeling statement.

## Design Doc Format

```
# <Feature> Design

## Feeling
One sentence.

## Research
For each reference (2-3):
- What it is
- What works and why
- What doesn't or wouldn't fit

## Design

### Layout
Spatial relationships, positioning, sizing.

### Hierarchy
What gets attention first. Typography, color, spacing.

### Interactions
Keyboard, mouse, transitions, animations.

### Visual Treatment
Catppuccin Mocha tokens, spacing values, typography.

### States
Empty, loading, populated, error, edge cases.

## Critique
Decisions challenged and why they survived. What was removed.
```

Scaled to complexity.
