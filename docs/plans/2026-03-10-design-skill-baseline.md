# Design Skill — Baseline Test Results (RED)

## Scenario

"Design the UI for a session health dashboard" — dispatched to sonnet without the design skill.

## Checklist

| Criteria | Result | Notes |
|----------|--------|-------|
| Feeling statement | NO | Jumped straight into layout and data model analysis |
| Research other tools | NO | Only examined existing codebase. Zero external references |
| Critique loop | NO | No decisions challenged. No "what would you remove?" |
| Specific visual decisions | YES | Used exact Catppuccin hex values, pixel sizes, CSS properties |
| Edge cases / states | YES | Empty, single, many, auto-worker, long names, scrolling |
| Controller patterns | YES | Referenced existing focus rings, status dots, workspace modes |

## Key Observations

1. **Technically competent, philosophically empty.** The design is a reasonable solution with good specs, but no intentionality about how it should *feel*. No north star guiding decisions.

2. **No external perspective.** The agent only looked inward at the codebase. It didn't consider how Datadog, Grafana, k9s, or any monitoring tool solves status-at-a-glance. The design reinvents without standing on shoulders.

3. **No critique = first draft accepted.** The agent presented its first idea as the final design. No simplification pass, no "does this element earn its place?" The filter bar, for example — does a health dashboard need filtering, or is that scope creep?

4. **Specificity was strong.** Despite missing the process, the agent produced exact hex values, pixel sizes, and layout specs. This suggests the "specify or it didn't happen" principle may not be the hardest gap to close — agents already do this when they read the codebase.

5. **Rationalizations observed:** None explicit — the agent didn't rationalize skipping steps because it didn't know the steps existed. It simply designed the way it naturally would: analyze data → propose layout → detail specs.

## Conclusion

The skill needs to enforce the *process* (feeling → research → derive → critique) more than the *output format*. Agents already produce specific designs when they have codebase context. What they miss is the philosophical grounding and external perspective that elevates adequate design into intentional design.
