# Military doctrine sources

The primary sources tsk's mission model draws on, with links. This file is a source
list, not a summary of doctrine. The reasoning, and the places tsk deviates, are in
[docs/domain/mission-model.md](../domain/mission-model.md).

Doctrine is advisory here. It is used as a body of tested vocabulary for describing
work, not as an authority tsk is obliged to follow. `mission-model.md` records each
deviation as a deliberate choice.

## Where the repo cites doctrine

| File | What it takes from doctrine |
|---|---|
| `docs/domain/mission-model.md` | Campaign, major operation, mission, objective, task, role, function. Mission command. Standing and contingent commitments, steady-state, the mission essential task list |
| `docs/domain/ubiquitous-language.md` | Plan and Planning, from ADP 5-0 |
| `docs/domain/domain-model-overview.md` | Task types as flags, the rejection of fixed echelons |

## US joint doctrine

The Joint Chiefs of Staff publish the joint publication series. Entry points:

- [Joint Doctrine Library](https://www.jcs.mil/Doctrine/)
- [Joint Doctrine Publications](https://www.jcs.mil/doctrine/joint-doctine-pubs/)
- [3-0 Operations Series](https://www.jcs.mil/Doctrine/Joint-Doctrine-Pubs/3-0-Operations-Series/),
  which holds JP 3-0. Cited for the principle of objective and for campaign and major
  operation.
- [5-0 Planning Series](https://www.jcs.mil/Doctrine/Joint-Doctrine-Pubs/5-0-Planning-Series/),
  which holds JP 5-0. Cited for the principle of objective and for mission analysis,
  the source of specified, implied and essential tasks.
- [Capstone Series](https://www.jcs.mil/Doctrine/Joint-Doctrine-Pubs/Capstone-Series/),
  which holds JP 1.

### JP 1 is two different publications, and the repo's citations mean the older one

`mission-model.md` quotes JP 1 for **role** ("the broad and enduring purposes for which
the Services and the combatant commands were established in law") and for **function**
("the broad, general, and enduring role for which an organization is designed, equipped,
and trained"). Those definitions come from JP 1, *Doctrine for the Armed Forces of the
United States*, the 2013 capstone publication.

JP 1 Volume 1, *Joint Warfighting*, was issued on 27 August 2023 and is a different
document with a different subject. A bare "JP 1" citation is now ambiguous, so any new
reference should name the edition.

- [JP 1, Doctrine for the Armed Forces of the United States (2013)](https://dml.armywarcollege.edu/wp-content/uploads/2022/12/JP-1-Doctrine-for-the-Armed-Forces-of-the-US-2013.pdf),
  hosted by the Army War College digital library
- [Reporting on the 2023 JP 1 Volume 1, Joint Warfighting](https://defensescoop.com/2023/09/13/us-military-publishes-new-joint-warfighting-doctrine/)

## UK defence doctrine

Cited for the split between **standing commitments**, the enduring non-discretionary
tasks, and **contingent commitments**, held at readiness against a possible deployment.
This is the source for tsk's use of "standing".

- [UK Defence Doctrine (JDP 0-01)](https://www.gov.uk/government/publications/uk-defence-doctrine-jdp-0-01)
- [JDP 0-01, UK Defence Doctrine, 6th edition, PDF](https://assets.publishing.service.gov.uk/media/63776f4de90e0728553b568b/UK_Defence_Doctrine_Ed6.pdf)
- [JDP 01, UK Joint Operations Doctrine, PDF](https://assets.publishing.service.gov.uk/government/uploads/system/uploads/attachment_data/file/389775/20141209-JDP_01_UK_Joint_Operations_Doctrine.pdf)
- [Joint Doctrine Publications collection](https://www.gov.uk/government/collections/joint-doctrine-publication-jdp)

## US Army doctrine

ADP 5-0, *The Operations Process*, is the source for the Plan and Planning entries in
the ubiquitous language, including planning as continuous rather than a phase, and the
definition of planning as "the art and science of understanding a situation, envisioning
a desired future, and laying out effective ways of bringing that future about".

- [ADP 5-0, The Operations Process, PDF](https://armypubs.army.mil/epubs/DR_pubs/DR_a/ARN18126-ADP_5-0-000-WEB-3.pdf)
- [ADP 5-0 record page](https://armypubs.army.mil/ProductMaps/PubForm/Details.aspx?PUB_ID=1007409)
- [Army Doctrine Publications index](https://armypubs.army.mil/ProductMaps/PubForm/ADP.aspx)

## Terminology

Definitions quoted as "doctrine's term" generally trace to the DoD Dictionary rather
than to a single publication. **Steady-state** and **standing joint task force** both
come from there.

- [DoD Terminology Program](https://www.jcs.mil/doctrine/dod-terminology-program/)

## What has not been verified from this repository

The quoted phrases in `mission-model.md` were gathered in earlier design sessions. They
have not been checked against primary text from inside the Claude Code cloud sandbox,
because its egress proxy blocks both `jcs.mil` and the `irp.fas.org` mirror. The links
above resolve and were returned by search against the publishers' own domains, but the
PDFs behind the `jcs.mil` links were not opened here.

Two consequences. A session running locally, outside the sandbox, can verify the quotes
and should. Until then, treat the quoted definitions as accurate to the best of the
sessions that recorded them, and the edition attributions as the weaker part: the JP 1
ambiguity above was found by checking publication history, not by reading either
document.

## Not doctrine

`mission-model.md` also cites one operations-research paper modelling campaign
objectives as axes, each a totally ordered set with precedence constraints. It is
marked in that file as not doctrine. The citation is incomplete in the repo, with no
author, title or link recorded, and tracing it is outstanding.
