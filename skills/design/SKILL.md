---
name: design
description: "Use when you need to design how something should look, feel, and behave — for new UI features, redesigns, or any visual/interaction work before implementation"
---

# Designing User Interfaces

## Overview

Feeling-first UI/UX design. Start from how the experience should feel, research how others solved the problem, derive specific visual and interaction decisions, then critique until every choice earns its place.

<HARD-GATE>
Do NOT skip research. Even for small designs.
Do NOT skip the critique loop.
Do NOT produce implementation code. Writing-plans handles implementation.
Do NOT present a design without a feeling statement.
</HARD-GATE>

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

You MUST follow these steps in order:

1. **Define the feeling** — One sentence. What should this feel like to use? This is the north star that every subsequent decision is measured against. The feeling must be emotional, not functional. "Calm awareness" is a feeling. "Situational awareness with quick access" is a product requirement — rewrite it.

2. **Research** — Find 2-3 apps/tools that solve a similar UX problem. Use web search. For each:
   - What it is and what problem it solves
   - What works well and why — be specific about the design choices, not just "it looks clean"
   - What doesn't work or wouldn't fit The Controller — and what you'd change
   - What design lesson you're taking from it into your design

3. **Derive the design** — From feeling + research, make specific decisions about:
   - Layout and spatial relationships
   - Information hierarchy (what gets attention first, second, third)
   - Interaction model (keyboard/mouse, transitions)
   - Visual treatment (Catppuccin Mocha tokens, typography, spacing)
   - States (empty, loading, error, populated, edge cases)
   Be specific. "Clean layout with good spacing" is worthless. Say which elements, what spacing, which tokens.

4. **Critique loop** — Challenge each decision:
   - Does this serve the feeling?
   - Is there a simpler way?
   - What would you remove and still have it work?
   - Does it feel cohesive with the rest of the app?
   - Apply design lenses explicitly: call out eye movement, negative space, glance test, etc. by name.
   Revise until every decision survives. The critique MUST result in at least one thing being removed or simplified. If nothing was cut, you weren't critical enough — go again.

5. **Present design** — Walk through section by section, get user approval.

6. **Write design doc** — Save to `docs/plans/YYYY-MM-DD-<topic>-design.md`, commit.

7. **Invoke writing-plans** — Hand off to implementation planning.

**Terminal state is invoking writing-plans.** Do NOT invoke any other skill.

## Design Lenses

Apply these by name in the critique loop:

- **Eye movement** — Does the eye land on the most important thing first?
- **Negative space** — Is emptiness intentional (grouping, breathing room)?
- **Visual weight** — Does weight distribution match hierarchy?
- **Contrast as communication** — Are color/size differences saying the right things?
- **Edge cases as design inputs** — 1 item? 50? 200-char name? Error?
- **Motion as meaning** — Does animation communicate state change? If not, remove it.
- **Density vs. cognitive load** — Dense + structured = powerful. Dense + flat = overwhelming.
- **The glance test** — Half-second view: what do they understand?
- **Consistency as trust** — Same patterns repeating predictably.

## Design Doc Format

Output as: `# <Feature> Design` with sections: **Feeling** (one sentence), **Research** (2-3 references with lessons), **Design** (Layout, Hierarchy, Interactions, Visual Treatment, States), **Critique** (lenses applied by name, what was removed). Scale each section to complexity.

## Red Flags — STOP and Revise

- Designing without a feeling statement
- Feeling statement describes a capability, not a feeling ("quick access to status" vs "calm awareness")
- No research references cited
- Research that just lists apps without analyzing specific design choices
- Vague specs ("good spacing", "clean layout", "nice colors")
- Skipping states (empty, error, loading)
- No critique section in the design doc
- Critique where nothing was removed — you weren't critical enough
- Design lenses not explicitly applied by name
- Copying a reference wholesale instead of synthesizing
