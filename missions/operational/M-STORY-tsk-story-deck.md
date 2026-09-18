# Mission: tsk story deck

| Field | Value |
|---|---|
| ID | M-STORY |
| Territory | agentic research |
| Assignee | cloud agent, Sonnet 5 |
| Blocked by | none |

## Objective

Kind: attainable

- A Marp deck exists at `docs/slide-decks/overview-for-engineers/`, 5 to 8 slides.
- It describes tsk as a product for an audience of other engineers and builders: the
  domain and the features, not tsk's own build history or the bootstrap journey.
- It leads with what is built today, and closes with one link forward to the broader
  vision, a holistic domain model of work, without elaborating that vision in depth.
- It carries visualisations: a mix of diagrams derived from tsk's existing domain docs
  (starting with the ubiquitous language) and new conceptual diagrams built to tell the
  story.
- The deck renders to output (HTML or PDF) with a single command run from inside
  `docs/slide-decks/overview-for-engineers/`, with no errors.
- Jim has reviewed and approved the rendered v1 deck.
- A follow-on admin mission exists, briefed, for keeping documentation updated going
  forward.

## Intelligence

- `docs/index.md`: entry point to tsk's design docs and user guide
- `docs/domain/ubiquitous-language.md`: domain terms, a source for both content and
  diagrams
- `docs/domain/domain-model-overview.md`: the domain model to walk through
- `docs/domain/mission-model.md`: mission, thread and actor concepts, if they belong in
  the feature story
- `docs/user-guide/`: what is actually built and usable today
- https://github.com/jimbarritt/ag-seminar: Jim's own repo, referenced by him for visual
  and narrative style inspiration for this kind of deck
- `docs/domain/mission-briefing-template.md`: the format for the follow-on admin
  mission's own briefing

## Decision authority

Jim approves the slide-by-slide narrative outline before any slide is built out in
detail (T-03). Jim reviews and approves the rendered v1 deck (T-06). Everything else
(Marp mechanics, diagram implementation, layout, tooling choices within the
constraints below) is the actor's to decide.

## Constraints

- Audience is other engineers and builders, not a general or non-technical audience.
- Content is tsk described as a product: the domain and the features. Not tsk's
  history, not the bootstrap journey.
- Leads with what is built today. The broader vision gets one closing link, not a
  section of its own.
- Short deck: 5 to 8 slides.
- All Marp tooling (`package.json`, dependencies, build scripts) lives inside
  `docs/slide-decks/overview-for-engineers/`. Nothing changes at the repository root.
  Use pnpm.

## Out of scope

- Describing tsk's build history or the M-BOOT bootstrap effort itself.
- A full roadmap or a detailed treatment of the broader vision. One link, no more.
- Variants for other audiences (non-technical, mixed). This mission produces the
  engineers/builders version only.
- Actually keeping the deck updated over time. T-07 sets up the mission that does that;
  it does not do the updating itself.

## Tasks

| ID | Task | Objective | Blocked by | Status |
|---|---|---|---|---|
| T-01 | Gather source material | A list of candidate domain concepts and shipped features to cover, drawn from the docs in Intelligence | none | TODO |
| T-02 | Review `ag-seminar` for style | Notes on visual and narrative style to carry into this deck | none | TODO |
| T-03 | Draft the narrative outline | A slide-by-slide outline (title, one-line content, which visual each needs) exists and Jim has approved it | T-01, T-02 | TODO |
| T-04 | Set up Marp tooling | `docs/slide-decks/overview-for-engineers/` has its own `package.json` with `@marp-team/marp-cli` as a pnpm dependency and a build script; running it renders the deck with no errors | none | TODO |
| T-05 | Build the deck | Slide content and visuals built to match the approved outline; deck renders cleanly | T-03, T-04 | TODO |
| T-06 | Jim reviews v1 | Jim has seen the rendered deck and approved it, or named changes | T-05 | TODO |
| T-07 | Set up the follow-on admin mission | A new administrative mission briefing exists in `missions/administrative/` for keeping documentation updated going forward. Its exact scope, deck-only versus general documentation, is that mission's own open decision, not settled here | T-06 | TODO |

**Essential task**: T-03. Building slides against an unapproved outline risks telling
the wrong story; the outline is where that risk is caught cheaply.

## Open decisions

- Whether the follow-on admin mission (T-07) is scoped to this deck alone or to
  documentation generally. Jim raised the possibility of the broader scope but deferred
  the decision to when that mission is itself briefed.
