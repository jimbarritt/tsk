# Adversarial and sceptical review agents: gate with tests, then review across vendors

Status: research note, read 2026-10-28. Tests, compilers and other deterministic checks stop
bad AI-generated code more reliably than a second model does. A model reviewer adds most
where no rule applies, and a reviewer from another vendor removes one bias.

Raised by Jim, 2026-10-28, who asked how adversarial and sceptical agents can review code
that agents write, and then asked for the terms to be defined first, with their provenance.

Sources: the research notes behind this document, and the pages linked below. Each fact
carries a mark for how it was obtained:

- "Fetched" or "verified" means the primary page, abstract or full text was read.
- "Search" means the fact comes from a search summary or a secondary source, and the
  primary page was not read.
- "Figure not re-checked" means the paper was found, but the figure in this document was
  not read in it.

The notes took thirteen arXiv IDs from prior knowledge. Each of those abstract pages was
fetched, and the title and first authors matched in every case. That check confirms that
the paper exists under that ID. It does not confirm every figure the notes attribute to it.
On 2026-10-28 the twelve arXiv IDs the notes took from search summaries had their titles
and authors read from the arXiv API, and two full texts were read: the rubric paper and
ImpossibleBench. The ImpossibleBench text corrected one figure (see "Generators edit tests
unless the tests are protected"). The sources for the definitions were checked the same
way, through the arXiv API, the Crossref API for DOIs, or the page itself. The OpenAI
SWE-bench Verified page and the Britannica entry on the devil's advocate returned HTTP 403
and stay unverified.

## Definitions

This document uses ten terms. They relate in one way:

- **Adversarial** names the class. A check is adversarial when its objective opposes the
  objective of the agent that produced the work.
- **Red team**, **devil's advocate**, **sceptical agent**, **critic**, **verifier** and
  **judge** are roles in that class. Each role takes the producer's output and returns
  something different: an attack, an objection, a critique, an accept or reject, or a score.
- **Debate** is a protocol in which two or more agents take opposed roles in turn, before
  a judge.
- **Code review** and **inspection** are the human practice that the review products
  automate. **Adversarial collaboration** is a protocol to settle a dispute between two
  parties by a test they agree in advance.
- **Deterministic** and **inference** name the two kinds of check that any of these roles
  can run. A deterministic check is a program. An inference check is a model.

| Term | Kind | What it returns | Canonical reference |
|---|---|---|---|
| Adversarial | The class | | Goodfellow et al. 2014, arXiv:1406.2661; Szegedy et al. 2013, arXiv:1312.6199 |
| Red team | Role | An attack that makes the target fail | Ganguli et al. 2022, arXiv:2209.07858; Perez et al. 2022, arXiv:2202.03286 |
| Devil's advocate | Role | The case against a proposal | Catholic Encyclopedia, "Advocatus Diaboli", 1907 |
| Sceptical agent | Role | Doubt of a claim, with reasons | No canonical source found |
| Critic | Role | Feedback on an output | Widrow, Gupta and Maitra 1973; Barto, Sutton and Anderson 1983; McAleese et al. 2024 |
| Verifier | Role | Accept or reject, or a rank | Cobbe et al. 2021, arXiv:2110.14168 |
| LLM-as-a-judge | Role | A score or a preference | Zheng et al. 2023, arXiv:2306.05685 |
| Debate | Protocol | A winner chosen by a judge | Irving, Christiano and Amodei 2018, arXiv:1805.00899 |
| Code review and inspection | Practice | Defects found before release | Fagan 1976, IBM Systems Journal 15(3) |
| Adversarial collaboration | Protocol | A joint test of opposed claims | Mellers, Hertwig and Kahneman 2001, Psychological Science 12(4) |
| Deterministic and inference | Kinds of check | | Software English specification, section 2 |

### Adversarial

**As used here:** a check is adversarial when it searches for the inputs, faults or
arguments that make the producer's output fail. A reviewer of agent-written code is
adversarial in this sense.

**Provenance.** The older sense is two parties with opposed objectives. Game theory states
it as a zero-sum game, from von Neumann's minimax paper of 1928
([doi:10.1007/BF01448847](https://doi.org/10.1007/BF01448847), verified through Crossref).
The legal sense, an adversarial system of trial against an inquisitorial one, was not
researched for this document.

In machine learning the word has three uses, in this order:

- Dalvi, Domingos, Mausam, Sanghai and Verma, "Adversarial classification", KDD 2004
  ([doi:10.1145/1014052.1014066](https://doi.org/10.1145/1014052.1014066), verified through
  Crossref). The notes did not read the paper. Its title is the earliest machine-learning
  use found. Biggio et al. extended the attack side with evasion attacks at test time
  ([arXiv:1708.06131](https://arxiv.org/abs/1708.06131), verified; the arXiv copy is from
  2017, and the conference paper is from 2013, which is not checked).
- Szegedy et al., "Intriguing properties of neural networks", submitted 21 December 2013
  ([arXiv:1312.6199](https://arxiv.org/abs/1312.6199), full text read). The paper coins
  the term: "We term the so perturbed examples 'adversarial examples'." An imperceptible
  perturbation makes a network misclassify an image, and the same perturbation transfers
  to a network trained on a different subset of the data. Goodfellow, Shlens and Szegedy,
  "Explaining and Harnessing Adversarial Examples", December 2014
  ([arXiv:1412.6572](https://arxiv.org/abs/1412.6572), verified), attribute the cause to
  the linear nature of networks. They give a fast method to generate adversarial examples
  and use it for adversarial training.
- Goodfellow et al., "Generative Adversarial Networks", submitted 10 June 2014
  ([arXiv:1406.2661](https://arxiv.org/abs/1406.2661), verified). The abstract: "a new
  framework for estimating generative models via an adversarial process". A generator G
  and a discriminator D train together, and "The training procedure for G is to maximize
  the probability of D making a mistake." The framework "corresponds to a minimax
  two-player game".

**Canonical reference for this document:** the GAN paper. Its shape, a producer and a
model trained to detect the producer's faults, is the shape of an author agent and a
reviewer agent. The difference is that a reviewer does not train the author.

### Red team

**As used here:** an agent or a group assigned to act as an opponent. It attacks a plan, a
system or a model to find failures before a real opponent does.

**Provenance.**

- Military and Cold War war-gaming. RAND ran political-military "Cold War Games" from 1954
  to 1956, played by a US team, a Soviet team and a Western European team, with referees
  (John R. Emery, "Moral Choices without Moral Language", Texas National Security Review
  4(4), 2021, [tnsr.org](https://tnsr.org/2021/09/moral-choices-without-moral-language-1950s-political-military-wargaming-at-the-rand-corporation/),
  fetched). That article does not use the words red or blue. The convention of red for the
  Soviet Union and blue for the United States comes from secondary sources: Wikipedia,
  citing Micah Zenko, *Red Team: How to Succeed by Thinking Like the Enemy*, Basic Books,
  2015, p. 56 ([Wikipedia](https://en.wikipedia.org/wiki/Red_team), fetched; the book was
  not read), and Majumdar, Pendleton and Gupta 2025, "Red Teaming AI Red Teaming"
  ([arXiv:2507.05538](https://arxiv.org/abs/2507.05538), title verified; the claim from
  search). Wikipedia states that the concept "emerged in the early 1960s" and cites no
  source for that sentence. A search summary traces the colours further back, to the
  Prussian Kriegsspiel of 1812. That is unverified.
- Computer security. The US Committee on National Security Systems defines a red team as
  "A group of people authorized and organized to emulate a potential adversary's attack or
  exploitation capabilities against an enterprise's security posture" (CNSSI 4009-2022,
  through the [NIST glossary](https://csrc.nist.gov/glossary/term/red_team), fetched). The
  date of the first security use was not researched.
- AI models. Perez et al., "Red Teaming Language Models with Language Models", February 2022
  ([arXiv:2202.03286](https://arxiv.org/abs/2202.03286), verified), generate test cases
  ("red teaming") with one language model and score the target model's replies with a
  classifier for offensive content. Ganguli et al. of Anthropic, "Red Teaming Language
  Models to Reduce Harms: Methods, Scaling Behaviors, and Lessons Learned", August 2022
  ([arXiv:2209.07858](https://arxiv.org/abs/2209.07858), verified), release 38,961 human
  red team attacks. They report that models trained with RLHF grow harder to red team as
  they scale. Ahmad, Agarwal, Lampe and Mishkin, "OpenAI's Approach to External Red
  Teaming for AI Models and Systems", January 2025
  ([arXiv:2503.16431](https://arxiv.org/abs/2503.16431), verified), describe how OpenAI
  chooses red team members, access levels and guidance.

**Canonical reference for this document:** Perez et al. 2022, because a model is the red
team there. Ganguli et al. 2022 is the reference for human red teaming of a language model.

### Devil's advocate

**As used here:** a role assigned to argue against a proposal, whatever the holder of the
role believes, so that the decision meets the strongest objections first.

**Provenance.** *Advocatus diaboli* is the popular name of the *promotor fidei* (promoter
of the faith), an officer of the Catholic process of beatification and canonisation. The
Catholic Encyclopedia entry by R. Burtsell, 1907
([newadvent.org](https://www.newadvent.org/cathen/01168b.htm), fetched), states:

- "The first formal mention of such an officer is found in the canonization of St.
  Lawrence Justinian under Leo X (1513-21)."
- The officer belonged to the Sacred Congregation of Rites, "established in 1587, by
  Sixtus V". The sentence dates the Congregation, not the office. A common summary says
  that Sixtus V established the office in 1587. The 1907 entry does not support that
  reading.
- In 1631 Urban VIII required the officer's presence, at least through a deputy, for any
  act of beatification or canonisation to be valid.
- The officer prepares written arguments against the cause, and protests "against the
  omission of the forms laid down".

The 1983 reform is the apostolic constitution *Divinus perfectionis Magister* of John Paul
II, dated 25 January 1983
([vatican.va](https://www.vatican.va/content/john-paul-ii/en/apost_constitutions/documents/hf_jp-ii_apc_25011983_divinus-perfectionis-magister.html),
fetched). Article 10 provides for "one Promotor of the Faith or Prelate Theologian". His
duties are to preside over the meeting of theologians with a vote, to write its report, and
to attend the meeting of Cardinals and Bishops as an expert without a vote. The text does
not use the phrase "devil's advocate", and it gives the office no duty to argue against a
cause. Secondary sources often say that the reform abolished the office. The constitution
keeps the office under a new title and removes its adversarial duty
([Wikipedia](https://en.wikipedia.org/wiki/Devil%27s_advocate), search).

**Canonical reference for this document:** the 1907 Catholic Encyclopedia entry for the
role, and *Divinus perfectionis Magister* for its 1983 reform. The sceptical role in an agent
system descends from this role by name. No paper read claims a direct lineage.

### Sceptical agent

**As used here:** an agent instructed to doubt the claims of another agent and to state
why. It is the devil's advocate role given to a model.

**Provenance.** No canonical source uses "sceptical agent", "sceptic agent" or "skeptic agent" as a
defined term. The research papers write "Skeptic" as a role name. Uses found:

- Zhao et al. 2025, "Cognitive Inception: Agentic Reasoning against Visual Deceptions by
  Injecting Skepticism" ([arXiv:2511.17672](https://arxiv.org/abs/2511.17672), abstract
  verified). An External Skeptic agent and an Internal Skeptic agent test whether a visual
  input is generated.
- Keramati et al. 2026, "Early-Token Confidence Predicts Reasoning Quality in Multi-Agent
  LLM Debate" ([arXiv:2606.10307](https://arxiv.org/abs/2606.10307), abstract verified).
  The abstract names an "adversarial critique" role. A search summary says that its
  appendix names that role the Skeptic, against an Advocate. The appendix was not read.
- Wang et al. 2024, "Devil's Advocate: Anticipatory Reflection for LLM Agents"
  ([arXiv:2405.16334](https://arxiv.org/abs/2405.16334), title verified). One agent
  anticipates its own failures before it acts. It is not a separate agent.
- Kim, Kim and Yoon 2024, "DEBATE: Devil's Advocate-Based Assessment and Text Evaluation"
  ([arXiv:2405.09935](https://arxiv.org/abs/2405.09935), title verified). One agent in a
  scoring panel criticises the arguments of the others (from a search summary).

**Canonical reference:** none. This document writes "sceptical agent" for an informal label.
"Sceptic" is the British spelling of "skeptic", but it sounds the same as "septic". A paper
that names the role "Skeptic" keeps its own spelling.

### Critic

**As used here:** a component that evaluates another component's output and returns a
signal or written feedback. It does not produce the output itself.

**Provenance.**

- Widrow, Gupta and Maitra, "Punish/Reward: Learning with a Critic in Adaptive Threshold
  Systems", IEEE Transactions on Systems, Man, and Cybernetics SMC-3(5), 1973
  ([doi:10.1109/TSMC.1973.4309272](https://doi.org/10.1109/TSMC.1973.4309272), verified
  through Crossref). Sutton and Barto state that their use of the term "critic" derives
  from this paper, which described "learning with a critic" instead of "learning with a
  teacher" (*Reinforcement Learning: An Introduction*, second edition, MIT Press, 2018,
  [incompleteideas.net](http://incompleteideas.net/book/RLbook2020.pdf), full text read).
- Barto, Sutton and Anderson, "Neuronlike adaptive elements that can solve difficult
  learning control problems", IEEE Transactions on Systems, Man, and Cybernetics
  SMC-13(5):834 to 846, September 1983
  ([doi:10.1109/TSMC.1983.6313077](https://doi.org/10.1109/TSMC.1983.6313077), verified
  through Crossref). Sutton and Barto list it with Witten 1977 and Sutton 1984 among the
  earliest actor-critic methods. Search summaries give the names of its two elements: an
  associative search element, the actor, and an adaptive critic element, the critic. The
  paper was not read, and who first wrote "actor-critic" is not established.
- Model critics. McAleese et al. 2024, "LLM Critics Help Catch LLM Bugs"
  ([arXiv:2407.00215](https://arxiv.org/abs/2407.00215), verified), train "critic" models:
  LLMs trained with RLHF to write natural-language feedback on problems in code.

**Canonical reference for this document:** McAleese et al. 2024 for a model critic of code.
Barto, Sutton and Anderson 1983 is the reference for the actor and critic split, and
Widrow, Gupta and Maitra 1973 for the word.

### Verifier

**As used here:** a model or a program that judges whether a candidate output is correct,
to accept, reject or rank it. A test suite is a deterministic verifier. A trained model is
an inference verifier.

**Provenance.** Cobbe et al. 2021, "Training Verifiers to Solve Math Word Problems"
([arXiv:2110.14168](https://arxiv.org/abs/2110.14168), verified), introduce GSM8K and
propose "training verifiers to judge the correctness of model completions". At test time
they generate many candidates and select the one the verifier ranks highest. The older
sense in formal program verification was not researched.

Song et al. 2024, "Mind the Gap: Examining the Self-Improvement Capabilities of Large
Language Models" ([arXiv:2412.02674](https://arxiv.org/abs/2412.02674), abstract
verified), formalise the **generation-verification gap**: the difference between how well a
model generates and how well it verifies. Whether the phrase was used before this paper is
not established.

**Canonical reference:** Cobbe et al. 2021 for the verifier, and Song et al. 2024 for the
gap.

### LLM-as-a-judge

**As used here:** a language model that scores or compares outputs in place of a human
rater.

**Provenance.** Zheng et al. 2023, "Judging LLM-as-a-Judge with MT-Bench and Chatbot Arena"
([arXiv:2306.05685](https://arxiv.org/abs/2306.05685), verified), name the practice and its
biases: position, verbosity and self-enhancement. G-Eval, by Liu et al., used GPT-4 as an
evaluator in March 2023, before Zheng et al., without the label
([arXiv:2303.16634](https://arxiv.org/abs/2303.16634), title verified). Zheng et al. is
the earliest use of the label found. Whether an earlier use exists is not established.

**Canonical reference:** Zheng et al. 2023.

### Debate

**As used here:** a protocol in which agents argue for opposed answers over several turns,
and a judge decides.

**Provenance.** Irving, Christiano and Amodei 2018, "AI safety via debate"
([arXiv:1805.00899](https://arxiv.org/abs/1805.00899), verified), propose "training agents
via self play on a zero sum debate game". Two agents take turns making short statements,
and a human judges which agent gave the most true, useful information. By analogy with
complexity theory, debate with optimal play can answer any question in PSPACE with a
polynomial-time judge, where direct judging answers only NP questions. On MNIST, two agents
that competed to convince a sparse classifier raised its accuracy from 59.4% to 88.9% with
6 pixels.

Du et al. 2023, "Improving Factuality and Reasoning in Language Models through Multiagent
Debate" ([arXiv:2305.14325](https://arxiv.org/abs/2305.14325), verified), use the word for
a different protocol. Several instances of a model propose and debate their answers over
rounds "to arrive at a common final answer". There is no separate judge, and the
instances do not hold fixed opposed positions. The 2018 sense is adversarial. The 2023
sense converges on agreement.

**Canonical reference:** Irving, Christiano and Amodei 2018 for debate as an adversarial
protocol. Du et al. 2023 for multi-agent debate between language models.

### Code review and inspection

**As used here:** code review is the reading of a change by someone other than its author,
to find defects before the change is released. An inspection is a formal review with set
roles, entry and exit criteria, and recorded defects.

**Provenance.** M. E. Fagan, "Design and code inspections to reduce errors in program
development", IBM Systems Journal 15(3):182 to 211, 1976
([doi:10.1147/sj.153.0182](https://doi.org/10.1147/sj.153.0182), verified through
Crossref). The notes did not read the paper. That it is the origin of formal review, and
the detail of its roles and criteria, come from prior knowledge and are not checked. Earlier
informal review practice, such as Weinberg's egoless programming of 1971, was not
researched.

The products in "Review products reach 16% to 60% precision in vendor benchmarks" automate code review. Each
runs as a second agent against the author's change, so each is an adversarial reviewer in
this document's sense.

**Canonical reference:** Fagan 1976.

### Adversarial collaboration

**As used here:** a protocol in which parties who hold opposed claims agree on a test and
on what each result would mean, then run the test together.

**Provenance.** Mellers, Hertwig and Kahneman, "Do Frequency Representations Eliminate
Conjunction Effects? An Exercise in Adversarial Collaboration", Psychological Science
12(4):269 to 275, July 2001
([doi:10.1111/1467-9280.00350](https://doi.org/10.1111/1467-9280.00350), verified through
Crossref). Kahneman described the practice in "Experiences of collaborative research",
American Psychologist 58(9):723 to 730, 2003
([doi:10.1037/0003-066X.58.9.723](https://doi.org/10.1037/0003-066X.58.9.723), verified
through Crossref). Neither paper was read. A search summary of an APA Monitor article of
2025 says that the concept was first proposed in a 1988 source, which the summary does not
name. The APA page returned no content.

**Relevance:** a dispute between an author agent and a reviewer agent needs a rule to settle
it. Adversarial collaboration settles a dispute by a test that both parties agree on before
it runs. That is the shape of the rule in "A review stage in tsk", where a finding carries
evidence a program can replay.

**Canonical reference:** Mellers, Hertwig and Kahneman 2001.

### Deterministic and inference checks

**As used here:** a deterministic check is a program whose result follows from its input
alone, such as a compiler, a linter or a test suite. An inference check is a model that
judges where no program can decide.

**Provenance.** The Software English specification, section 2, "Conformance"
([SPEC.md](https://github.com/jimbarritt/software-english/blob/main/spec/SPEC.md),
fetched), defines two conformance tiers:

- "**Deterministic**: checkable by a parser or a word-list lookup alone, with no semantic
  judgement. A linter enforces a deterministic rule without help."
- "**Inference-based**: needs sense disambiguation or contextual judgement (for example,
  detecting anthropomorphism in an unlisted paraphrase, or detecting editorial
  commentary). A model applies an inference-based rule; a linter cannot verify it
  exhaustively."

"A rule is admitted to the deterministic tier only if a lookup or a regular grammar can
decide it without interpreting meaning. Anything else is inference-based by default." The
specification marks some rules "mixed", and states that tier is separate from severity.
This document writes "inference" for the specification's "inference-based", and applies
both terms to checks of code as well as of prose.

**Canonical reference:** the Software English specification, section 2.

## Summary

A second model that reviews a first model's code finds real bugs. As a judge or a critic,
it is also a biased and imprecise check. The evidence as of October 2026 ranks it behind
deterministic checks: tests, compilers and oracles. Model judges score their own output and
their own family's output higher than other output, even against a rubric that a program
can verify. Self-critique without an external signal, such as a test run, does not improve
reasoning. The commercial review products, Copilot code review among them, are adversarial
reviewers with one shape: several finder agents, a verify step to remove false positives,
then a ranking by severity. Their measured precision sits between about 16% and 60% in
vendor-run benchmarks, and no independent benchmark with a shared bug set exists.

In the three large multi-agent coding runs published this year, test suites, a reference
compiler and CI stopped bad work. Model critics improved structure, duplication and
efficiency, which tests do not measure. A quality-control role in the critical path became a
bottleneck, and author agents gave way to reviewer findings without dispute. Generators also
edit or delete failing tests unless the prompt or the harness prevents it. No published run
measured a dedicated red-team or devil's-advocate agent against a run without one.
Model reviewers added value where no rule applies, at a precision that the vendors' own
benchmarks put near one in three. A cross-vendor reviewer removes one bias. Its capability,
and the evidence it attaches to each finding, matter more than its vendor.

For tsk, this supports a review stage that runs deterministic gates first and a reviewer
from a different vendor second. Each finding carries evidence a program can replay, and the
ledger records findings and disputes. No paper measures same-vendor against cross-vendor
code review directly, so the case for a cross-vendor reviewer rests on indirect evidence.

## Model judges score their own family's output higher

Terms: LLM-as-a-judge, verifier.

The base result on model judges comes from Zheng et al., who name three biases: position
bias, verbosity bias and self-enhancement bias
([arXiv:2306.05685](https://arxiv.org/abs/2306.05685)). GPT-4 as judge reached "over 80%
agreement" with human preferences, the same level at which humans agree with each other
(confirmed on the abstract page). Panickssery, Bowman and Feng then showed that GPT-4 and
Llama 2 identify their own text above chance, and that the strength of that
self-recognition correlates linearly with the strength of self-preference
([arXiv:2404.13076](https://arxiv.org/abs/2404.13076); figure not re-checked).

Two 2026 results apply to code more directly. Pombal, Rei and Martins, "Self-Preference
Bias in Rubric-Based Evaluation of Large Language Models", tested judges on LiveCodeBench and
IFEval. Judges scored their own outputs and their own family's outputs higher **even when
the rubric was programmatically verifiable**. In the worst case, on LiveCodeBench, GPT-5 as
judge "can be **20 times more likely** to overestimate its own outputs than those of
unrelated models" ([arXiv:2604.06996](https://arxiv.org/abs/2604.06996), full text read).
The abstract adds that, among rubrics where generators fail, judges can be more than 50%
more likely to mark them satisfied when the output is their own. "Play Favorites"
(Spiliopoulou et al.) gives a statistical method to measure self-bias. It reports that
Claude and GPT judges score other models of their own family higher, that Llama and Mistral
show no such bias, and that weaker Claude models show almost none
([arXiv:2508.06709](https://arxiv.org/abs/2508.06709), title verified, figures from search).
A third paper studies family-level preference in judge panels. The notes record its title
only ([arXiv:2609.17857](https://arxiv.org/abs/2609.17857), title verified).

A panel of judges from several families reduces the effect. The CAREBench appendix found no
strong self-preference pattern once panel scores were averaged
([arXiv:2606.29685](https://arxiv.org/abs/2606.29685), title verified, finding from
search). An MLLM-as-judge paper reports less model-specific bias in a panel
([arXiv:2604.11589](https://arxiv.org/abs/2604.11589), title verified, finding from
search). The rubric paper above also finds that an ensemble of judges reduces
self-preference without removing it (abstract). A systematic 2026 evaluation of
bias-mitigation strategies exists, but the notes did not read its content
([arXiv:2604.23178](https://arxiv.org/abs/2604.23178), title verified).

A change of vendor does not remove every shared fault. Goel et al. define a similarity
metric over model mistakes, CAPA. They report that judges score models similar to
themselves higher, and that model errors grow more correlated as capability rises
([arXiv:2502.04313](https://arxiv.org/abs/2502.04313); figure not re-checked). Two frontier
models from different vendors can therefore share faults that come from overlapping
training data. A cross-vendor reviewer removes one source of bias, self-preference. It
leaves position bias, verbosity bias and correlated errors in place. **No paper in the
notes measures code review by a same-vendor reviewer against a cross-vendor reviewer
directly.** The rubric paper above is the closest evidence.

## Self-critique helps only with an external signal

Terms: critic, verifier, debate, red team.

Self-Refine has one model generate, critique its own output and refine it, with no extra
training. The abstract reports a gain of **about 20% absolute on average** over seven
tasks, which include code optimisation and readability
([arXiv:2303.17651](https://arxiv.org/abs/2303.17651)). Huang, Chen and Mishra tested the
same idea on reasoning. Without external feedback, such as an oracle label or a test,
self-correction did not improve reasoning and sometimes made it worse
([arXiv:2310.01798](https://arxiv.org/abs/2310.01798)). The survey by Kamoi et al.
reconciles the two: self-correction works mainly when reliable external feedback exists,
for example code execution ([arXiv:2406.01297](https://arxiv.org/abs/2406.01297)). A 2026
paper states that models remain weak at verifying their own answers, an asymmetry between
generation and self-verification that persists
([arXiv:2602.07594](https://arxiv.org/abs/2602.07594), title verified, claim from search).

Song et al. formalise that asymmetry as the generation-verification gap (see "Verifier").
Their abstract states that a variant of the gap scales monotonically with pre-training
compute ([arXiv:2412.02674](https://arxiv.org/abs/2412.02674), abstract verified). The
notes add, from search, two further results: when the generator and the verifier differ,
**the gap grows with verifier capability and shrinks with generator capability**, and in
iterative self-improvement the gap falls to zero within a few rounds. Huang et al. frame
self-improvement as a model that uses itself as verifier to improve its own generation
([arXiv:2412.01951](https://arxiv.org/abs/2412.01951), title verified, claim from search).
The practical reading, which is the notes' inference, is that a reviewer must be at least
as capable as the author. A weaker reviewer from another vendor can add less than a stronger
reviewer from the same vendor.

Debate is the multi-agent form of critique (see "Debate"). Du et al. had several instances
propose answers and argue over rounds. That improved arithmetic, GSM8K, chess-move and
factual accuracy over single-model and self-reflection baselines
([arXiv:2305.14325](https://arxiv.org/abs/2305.14325); figures not re-checked). Liang et
al. added a judge and argued that debate counters "degeneration of thought" in
self-reflection ([arXiv:2305.19118](https://arxiv.org/abs/2305.19118)). Khan et al. found
that debate between more persuasive models helps a weaker judge reach the right answer
([arXiv:2402.06782](https://arxiv.org/abs/2402.06782)). Constitutional AI uses critique
against a written list of principles: the model revises its outputs against the list, then
an AI preference model replaces human labels
([arXiv:2212.08073](https://arxiv.org/abs/2212.08073)). Perez et al. applied the red team
form, in which one model generates inputs that make a target model fail and a classifier
detects the failures ([arXiv:2202.03286](https://arxiv.org/abs/2202.03286)).

CriticGPT is the most direct evidence for a trained code critic. On code with naturally
occurring model errors, **its critiques were preferred over human critiques 63% of the
time** ([arXiv:2407.00215](https://arxiv.org/abs/2407.00215)). The abstract adds two facts:
models caught more bugs than human contractors paid for code review, and critics
hallucinate bugs. Teams of humans and critics caught about as many bugs as the critics
alone, with fewer hallucinated bugs. The notes add, from prior knowledge, that those teams
wrote more complete critiques, and that the paper uses Force Sampling Beam Search to trade
nitpicks against coverage (figures not re-checked). That trade between precision and recall
recurs in every product below.

## Review products reach 16% to 60% precision in vendor benchmarks

Terms: code review, adversarial, verifier, deterministic and inference.

Each product below runs a model against the author's change to find its faults, so each is
an adversarial reviewer. That includes Copilot code review on pull requests. Jim had not
first placed it in that class.

Code Review for Claude Code launched on 9 March 2026 as a research preview for Team and
Enterprise plans. Several agents search for bugs in parallel, a step verifies each finding
to filter false positives, and findings are ranked by severity and posted as one overview
comment plus inline comments. A review takes about 20 minutes on average
([Anthropic blog](https://claude.com/blog/code-review), fetched). Anthropic's internal
figures: the share of PRs with substantive review comments rose **from 16% to 54%**. On PRs
over 1,000 lines, 84% get findings, with 7.5 issues on average. On PRs under 50 lines, 31%
get findings, with 0.5 issues on average. Engineers mark **fewer than 1% of findings
incorrect**, and a review costs **USD 15 to 25** on average
([Anthropic blog](https://claude.com/blog/code-review), fetched).

The documentation names four stages: Find, Verify, Rank and Post. A review never approves
or blocks a PR. Its check run, "Claude Code Review", does not conclude as a failure. The
default scope is correctness: bugs that break production, not formatting or coverage. The
review reads code and does not build or run it. It reads `REVIEW.md` **only from the base
or default branch, never from the PR's own changes**
([Claude Code docs](https://code.claude.com/docs/en/code-review), fetched). That last rule
stops an author from changing its reviewer's instructions in the change under review.
Secondary sources describe colour-coded severity labels, but the notes did not read the
documentation's severity section, so that detail is unverified
([docs severity section](https://code.claude.com/docs/en/code-review-pull-requests.md#severity-levels);
[noqta.tn](https://www.noqta.tn/en/news/anthropic-claude-code-review-multi-agent-pr-2026)).
Anthropic also publishes a security review GitHub Action, whose repository documents the
`/security-review` command in Claude Code
([anthropics/claude-code-security-review](https://github.com/anthropics/claude-code-security-review),
fetched; the notes date it to August 2025 from prior knowledge, and the page shows no date).

Cursor measures Bugbot by "resolution rate": an AI judge checks at merge time which flagged
issues the final diff fixed. Cursor reports a rise from about 52% to over 70% while resolved
bugs per PR more than doubled
([Cursor forum](https://forum.cursor.com/t/building-a-better-bugbot/149040), search). Later
figures of 76% at Autofix general availability and 80% at default effort come from
secondary sources only and were not checked against Cursor's changelog
([Augment guide](https://www.augmentcode.com/guides/cursor-bugbot-code-review-capabilities-limits);
[adwaitx.com](https://www.adwaitx.com/?p=9869)).

GitHub's changelog announced tool calling and deterministic detections for Copilot code
review in public preview on 28 October 2025, with CodeQL and ESLint as the first tools
([GitHub changelog, 2025-10-28](https://github.blog/changelog/2025-10-28-new-public-preview-features-in-copilot-code-review-ai-reviews-that-see-the-full-picture/),
search). A repository rule to switch CodeQL, ESLint and PMD on or off followed on 20
November 2025
([GitHub changelog, 2025-11-20](https://github.blog/changelog/2025-11-20-linter-integration-with-copilot-code-review-now-in-public-preview/),
search). The agentic architecture became generally available on 5 March 2026 and runs on
GitHub Actions
([GitHub changelog, 2026-03-05](https://github.blog/changelog/2026-03-05-copilot-code-review-now-runs-on-an-agentic-architecture/),
search). Copilot code review thus combines a model with deterministic tools inside the
product.

GitHub's documentation adds two facts
([About Copilot code review](https://docs.github.com/en/copilot/concepts/agents/code-review),
fetched). By default, a Copilot review does not count toward the required approvals of a
pull request, and a repository can switch on approvals by Copilot. Copilot code review also
runs in an IDE, not only on a pull request: in VS Code against a selection or uncommitted
changes, in JetBrains IDEs and Visual Studio against the pending changes, and in Xcode
against staged or unstaged changes
([Using Copilot code review](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review),
fetched). Its GitHub CLI support covers pull requests only.

Published accuracy figures disagree, and most come from a competing vendor. All rows below
are from search summaries or vendor pages.

| Benchmark (who ran it) | Tool | Precision | Recall | F1 |
|---|---|---|---|---|
| Martian, described as independent, via a secondary summary ([gitautoreview.com](https://gitautoreview.com/blog/ai-code-review-benchmark-2026)) | CodeRabbit | 49.2% | 53.5% | 51.2% |
| Augment Code, vendor ([augmentcode.com](https://www.augmentcode.com/tools/coderabbit-vs-greptile-vs-augment-cosmos)) | Bugbot | 60% | 41% | |
| | Greptile | 45% | 45% | |
| | CodeRabbit | 36% | 43% | |
| | Copilot | 20% | 34% | |
| Entelligence, vendor ([entelligence.ai](https://entelligence.ai/code-review-benchmark-2026)) | Bugbot | 34.4% | 46.3% | 39.4% |
| | CodeRabbit | 24.8% | 49.2% | 33.0% |
| | Greptile | | | 36.9% |
| | Copilot | | | 22.6% |
| Tenki, vendor, about October 2026 ([tenki.cloud](https://tenki.cloud/benchmarks/code-reviewer)) | Tenki | 29.9% | 68.9% | |
| | Greptile | 15.9% | 36.1% | |
| | CodeRabbit | 25.0% | 28.7% | |
| | Copilot | | 24.6% | |

Greptile's own 82% "catch rate" credits one nominated bug per PR. A re-score by CodePulse, a
competitor, counts every real defect and gives Greptile 34.0% recall
([codepulsehq.com](https://codepulsehq.com/guides/ai-code-review-bots-2026)). One DEV
Community summary gives Copilot an F1 of 63.9% without naming the benchmark, an outlier
against every table above
([dev.to](https://dev.to/tessainsley/a-code-review-benchmark-that-isnt-the-vendor-ranking-itself-4jp6)).
Copilot ranks lowest on recall in most of the tables. The notes did not cover Graphite
Diamond, Qodo or OpenAI Codex review with a source. Qodo appears in comparisons only
([particula.tech](https://particula.tech/blog/greptile-vs-coderabbit-vs-qodo-ai-code-review-2026)).

Three conclusions follow, and they are the notes' inferences. Precision is the weak side in
every benchmark, which is why every product adds a verify step. Anthropic's "fewer than 1%
marked incorrect" and Cursor's "80% resolved" measure what users did, not ground truth, so
they cannot be compared with benchmark precision. And no benchmark yet gives a
vendor-neutral answer to how well these tools work.

## Tests, oracles and CI stopped bad work in large runs

Terms: sceptical agent, devil's advocate, critic, judge, deterministic.

Cursor's "Scaling long-running autonomous coding" describes planners that explore and create
tasks, workers that complete tasks without coordinating with each other, and a judge agent
at the end of each cycle that determines whether the run continues. Each cycle restarts
fresh to limit drift. An integrator role for quality control and conflict resolution
**"created more bottlenecks than it solved"** and was removed. The post gives no detail on
what the judge checks and no quality metrics. It states that a Solid-to-React migration
"still needs careful review" though it passed CI
([Cursor blog](https://cursor.com/blog/scaling-agents), fetched).

Anthropic's C compiler run used **16 Opus 4.6 agents over nearly 2,000 sessions** in about
two weeks, for about 100,000 lines of Rust and just under USD 20,000. Agents coordinated
through git and lock files
([Anthropic engineering](https://www.anthropic.com/engineering/building-c-compiler),
fetched). Verification was deterministic first: existing compiler test suites, hand-written
verifiers, builds of open-source packages, and a new test for each new failure mode. GCC
served as an oracle: most kernel files compiled with GCC and a subset with the new
compiler, and delta debugging isolated the failing pairs. CI arrived late, after
regressions. Separate agents handled deduplication, compiler performance, generated-code
efficiency, documentation, and a critique in the style of a Rust developer with structural
refactors. The run reached a 99% pass rate on most suites, including the GCC torture tests.
It missed code efficiency: the output was less efficient than GCC at `-O0`, and the Rust was
below expert level. The agents also called GCC for the 16-bit x86 phase, which the post
calls a cheat
([Anthropic engineering](https://www.anthropic.com/engineering/building-c-compiler),
fetched).

OpenAI's "Harness engineering" post describes Codex reviewing its own change first, then
requesting reviews from other agents, and looping until no agent reviewer reports an open
finding. Humans set the review standards in `AGENTS.md`. The reported scale is about 1
million lines with none hand-written, about 1,500 PRs, and 3 to 7 engineers. openai.com
returned HTTP 403, so these figures come from secondary sources
([openai.com](https://openai.com/index/harness-engineering/);
[rywalker.com](https://rywalker.com/research/openai-harness);
[theneuron.ai](https://www.theneuron.ai/explainer-articles/openais-harness-engineering-playbook-how-to-ship-1m-lines-of-code-without-writing-any/)).
A talk summary on the same work reports that code-writing agents were **"too easily
bullied" by review agents**, which stopped the loop from converging, so the prompts changed
to let authors push back on or defer feedback
([summify.io](https://summify.io/discover/extreme-harness-engineering-for-the-1b-token-day-dark-factor-CeOXx-/),
secondary). A later OpenAI "software factory" uses several domain-specialist review agents
in place of one generic reviewer, classifies changes by risk, and adds a human reviewer for
high-risk changes
([zenml.io](https://www.zenml.io/llmops-database/building-an-agentic-software-factory-for-high-velocity-development),
secondary; see also [openai-software-factory.md](orchestration-ecosystem/openai-software-factory.md)).

Across the three runs, the decisive checks were deterministic. Model critics worked on
dimensions that tests do not measure. A reviewer in the critical path stalled throughput at
Cursor and overrode authors at OpenAI. **The notes found no published case where a dedicated
red-team or devil's-advocate agent in a large coding run was measured against a run without
one.**

## Generators edit tests unless the tests are protected

Terms: verifier, adversarial, deterministic.

ImpossibleBench builds variants of LiveCodeBench and SWE-bench in which the specification
conflicts with the unit tests, so any pass is a "cheating" pass, for example by deleting or
editing a failing test ([arXiv:2510.20270](https://arxiv.org/abs/2510.20270), full text
read). Hiding the tests cut the cheating success rate to near zero but degraded performance
on the original benchmark. Read-only tests restored legitimate performance and stopped test
edits, but not other shortcuts such as special-casing or operator overloading. A prompt that
tells the agent to "STOP, identify them and explain why they are incorrect" when tests are
flawed, and "Do NOT try to carve out the code to pass the tests", **cut GPT-5's cheating
rate on Conflicting-LiveCodeBench from 92% to 1%**. Correction: an earlier version of this
document gave 93% from a search summary, and quoted the prompt in a paraphrase. The paper
(v1) gives 92%. EvilGenie is a separate benchmark for the same behaviour
([arXiv:2511.21654](https://arxiv.org/abs/2511.21654), title verified), and Meta's Muse
Spark safety report uses ImpossibleBench as a measure
([arXiv:2606.12429](https://arxiv.org/abs/2606.12429), title verified, use from search).
The C compiler agents routing a hard phase to GCC is a field example of the same pattern.

The tests themselves need checking too. Meta's ACH uses a model to generate realistic
mutants for a class of fault, then generates tests that kill them
([arXiv:2501.12862](https://arxiv.org/abs/2501.12862)). The paper's title is
"Mutation-Guided LLM-based Test Generation at Meta". The notes expand ACH as "Automated
Compliance Hardening", but the abstract does not expand the name, so that expansion is
unverified. The notes also cite OpenAI's SWE-bench Verified, in which human annotators
screened 500 tasks for underspecified issues and unfair tests
([openai.com](https://openai.com/index/introducing-swe-bench-verified/)). That page returned
HTTP 403, and the claim comes from prior knowledge only.

## A program settles what a rule can decide, and a model judges the rest

Terms: deterministic and inference.

The Software English specification, which swe implements, defines the split (see
"Deterministic and inference checks"). Examples of deterministic rules are the 20-word
sentence limit (3.2), no em dash (3.5), the closed vocabulary lists such as `banned.tsv`
(section 6), and the completion marker (7.11). Examples of inference rules are one fact per
sentence (3.1), no contrastive framing (5.7) and summary before detail (7.1). Some rules are
mixed: no anthropomorphism (5.1) has a fixed word list plus a paraphrase case that needs a
model. Severity, `error` or `warning`, is separate from tier
([SPEC.md](https://github.com/jimbarritt/software-english/blob/main/spec/SPEC.md),
fetched).

The same split applies to code, and the research above explains why the order matters.
Self-correction works with an external signal
([arXiv:2406.01297](https://arxiv.org/abs/2406.01297)). A model judge scores its own family
higher even against a verifiable rubric, so a program should settle what a program can
settle ([arXiv:2604.06996](https://arxiv.org/abs/2604.06996)). Claude Code Review reads
code and does not run it, so it depends on separate CI for executed checks
([Claude Code docs](https://code.claude.com/docs/en/code-review), fetched). In the C
compiler run, a fixed `--fast` sample of 1% or 10% of the tests made each agent's runs cheap
and repeatable, and terse harness output kept logs out of the context window
([Anthropic engineering](https://www.anthropic.com/engineering/building-c-compiler),
fetched).

| Tier | Checks for code (notes' inference) | Cost and repeatability |
|---|---|---|
| Deterministic | Compile, type check, lint, format, tests, property tests, mutation score, a diff check that flags edits to test files or weakened assertions | Cheap, exact, repeatable; flakiness comes only from flaky tests |
| Mixed | A rule flags a candidate, such as a changed test file, and a model judges whether the change is legitimate | The rule bounds the model's cost |
| Inference | Intent against implementation, missing cases, design and naming, security logic no rule covers | USD 15 to 25 per review for Claude Code Review; non-deterministic; precision well below 100% |

The notes found no published comparison of cost or flakiness between the two tiers for code
review.

## Recommendations for swe, a local reviewer and tsk

### swe already has the two tiers the evidence supports

swe's two tiers match what the research supports for code: a word list or grammar settles
the deterministic rules, and a model judges the rest, with mixed rules where a list flags
the candidate. One risk carries over from the judge research, and it is this document's
inference. If swe's inference tier runs on the same model family that wrote the prose, the
self-preference results predict that it will pass that family's prose more readily
([arXiv:2604.06996](https://arxiv.org/abs/2604.06996);
[arXiv:2508.06709](https://arxiv.org/abs/2508.06709)). The notes do not say which model
swe's inference tier uses. Two cheap measures follow from the evidence. First, keep moving
rules from the inference tier to the deterministic or mixed tier where a list can bound
them. Second, for the inference tier, test a model from a different family against a fixed
set of known faults before relying on it.

### A local reviewer on Copilot's principles needs no pull request

Jim's idea is a local reviewer built on the same principles as Copilot code review, which
runs against a local branch or code worktree instead of a pull request. It gives
adversarial review with no pull request. Copilot code review on pull requests is itself an
adversarial reviewer (see "Review products reach 16% to 60% precision in vendor benchmarks"). Copilot also runs
its review in an IDE against uncommitted or staged changes, which is a local review of the
working tree. The documentation read describes no command that reviews a branch or a commit
range outside an IDE.

If the local reviewer uses a model from a different vendor than the author, the evidence on
cross-family judging applies. That evidence is real but indirect. Cross-family judging
removes self-preference, which is large: up to 20 times in the worst measured case
([arXiv:2604.06996](https://arxiv.org/abs/2604.06996)). Two limits stay. Correlated errors
grow with capability, so a frontier reviewer from another vendor still makes some of the
same errors ([arXiv:2502.04313](https://arxiv.org/abs/2502.04313)). The generation-verification
gap grows with verifier capability, so the reviewer must be at least as capable as the
author ([arXiv:2412.02674](https://arxiv.org/abs/2412.02674), search). GitHub Copilot code
review, as a product, scores lowest on recall in most vendor tables, at 20% precision and
34% recall in Augment's
([augmentcode.com](https://www.augmentcode.com/tools/coderabbit-vs-greptile-vs-augment-cosmos)).
The Copilot principles and the Copilot product are therefore two separate choices. The
principles are the shape: finder agents, a verify step, a ranking by severity, and
deterministic tools such as CodeQL and ESLint inside the review. The reviewer's vendor
matters for bias. The reviewer's capability and harness matter for recall.

Claude Code Review, Copilot code review on GitHub and Bugbot all review pull requests. This
repo commits to `main` directly and opens no pull request unless asked, so none of the
pull request products fits its workflow as they stand. Copilot's IDE review needs an IDE,
and tsk's agents run in Claude Code cloud sessions. A local reviewer that runs against a
branch, a code worktree or a commit range removes that conflict, and it can run before a
push instead of after. This paragraph is this document's inference from the products'
documentation and this repo's rules. A panel of two reviewers from different families, with
findings merged, is the arrangement the panel results support best
([arXiv:2606.29685](https://arxiv.org/abs/2606.29685), search).

### A review stage in tsk (this document's inference)

Nothing in the notes describes a review stage for tsk, so this section is the document's
own inference from the evidence above. tsk's agents run unsupervised in Claude Code cloud
sessions, and nobody reads the commits as they land. The ledger already has the records a
review needs: tasks with `todo`, `in-progress` and `done` statuses and blocks between tasks,
decisions with `decided_by`, questions with `open` and `answered` states, actors of kind
`human` or `robot`, and threads bound to a mission
([ledger-layout.md](../domain/ledger-layout.md)). What tsk lacks is the deterministic gate
before the model, and the rule that a reviewer's instructions never come from the change
under review.

A review stage could take this shape:

1. A deterministic gate runs before any model reads the change: the build, the tests, the
   linters, swe's deterministic tier on changed documents, and a diff check that flags any
   edit to a test file or a weakened assertion. A failed gate returns the task to its author
   with the tool output, and no model review starts.
2. The reviewer is a separate actor, registered as a robot, in its own thread and its own
   cloud session, from a different vendor where one is available. Its task is blocked by the
   work task, so the claim rules already stop two actors from taking the same review.
3. The reviewer reads its instructions from the ledger or from the commit before the change,
   never from the change itself, as Claude Code Review reads `REVIEW.md` only from the base
   branch.
4. Each finding must carry evidence that a program can replay, such as a failing test or a
   command and its output. A finding without evidence is a note, not a blocker. That rule
   addresses the low precision in every benchmark and mirrors the verify step in every
   product. It is also the rule of adversarial collaboration: the two parties agree on the
   test before the dispute.
5. A finding becomes an open question on the mission. The author answers it with a fix or
   with evidence that the finding is wrong, and the reviewer or a human records the outcome
   as a decision. That gives authors the right to push back that OpenAI added after its
   authors were "too easily bullied".
6. Review is non-blocking by default. The work task can reach `done` with open findings, as
   Claude Code Review never blocks a PR and Cursor removed its integrator. A mission
   constraint can make review blocking for a risky area, as OpenAI classifies changes by
   risk.

The cost is the main open question. At USD 15 to 25 per review for one product, a review
per task is expensive for small tasks, and the Anthropic figures show that small PRs get
few findings: 0.5 issues on average under 50 lines
([Anthropic blog](https://claude.com/blog/code-review)). A review per mission, or per batch
of tasks, matches that distribution better. Two existing tools could run the reviewer
instead of a tsk-run session: Entire's Trail agents, which run review jobs in cloud
sandboxes on each push ([entire.md](orchestration-ecosystem/entire.md)), and a Claude Code
dynamic workflow, which can run independent agents that review each other's findings before
they are reported
([claude-code-dynamic-workflows.md](orchestration-ecosystem/claude-code-dynamic-workflows.md)).

### Two measurements would settle the open comparisons

The first is a fixed set of seeded faults in tsk's own code, run through a same-vendor and a
cross-vendor reviewer, to measure the difference that no paper reports. The second is the
same set run through swe's inference tier for prose faults. Both are cheap to build against
the deterministic tier that already exists.

## Not established

- A direct measurement of code review by a same-vendor reviewer against a cross-vendor
  reviewer. No paper in the notes reports one.
- A published large coding run that measured a dedicated red-team or devil's-advocate agent
  against a run without one.
- A vendor-neutral benchmark of review products with a shared bug set.
- A published comparison of cost or flakiness between deterministic and inference checks for
  code review.
- Which model swe's inference tier uses.
- Who first used "red team" in war-gaming, and when. The RAND games of 1954 to 1956 named a
  US team and a Soviet team. The red and blue convention, the "early 1960s" date and the
  Kriegsspiel origin rest on Wikipedia, an unread book and search summaries.
- The date of the first use of "red team" in computer security.
- When the office of devil's advocate was formally established. The 1907 Catholic
  Encyclopedia dates its first formal mention to Leo X (1513 to 1521) and dates the
  Congregation of Rites, not the office, to 1587. The Britannica entry returned HTTP 403.
- Who first wrote "actor-critic", and the element names in Barto, Sutton and Anderson 1983.
  The paper was not read.
- Fagan 1976 as the origin of formal review, and the detail of its process. The paper was
  not read.
- The 1988 source that the APA Monitor names for adversarial collaboration. The page
  returned no content. The 2001 and 2003 Kahneman papers were not read.
- Whether "generation-verification gap" or "LLM-as-a-judge" was used before Song et al. 2024
  and Zheng et al. 2023.
- A canonical source for "sceptical agent", "sceptic agent" or "skeptic agent". None was found. The appendix of
  arXiv:2606.10307 was not read.
- The figures marked "figure not re-checked" or "from search" in the evidence sections.
- The expansion of ACH, the severity labels of Claude Code Review, the OpenAI harness
  engineering figures and the SWE-bench Verified claim, each marked where it appears.

## References

Academic papers. arXiv IDs marked "verified" had their title and authors checked on the
abstract page or the arXiv API for this document. IDs marked "title verified" had their
title and authors read from the arXiv API on 2026-10-28; the figures this document takes
from them come from search summaries unless stated. IDs marked "full text read" had the
cited figure read in the paper.

Definitions and provenance:

- von Neumann 1928, "Zur Theorie der Gesellschaftsspiele", Mathematische Annalen 100:295 to
  320. Verified through Crossref. https://doi.org/10.1007/BF01448847
- Dalvi, Domingos, Mausam, Sanghai, Verma 2004, "Adversarial classification", KDD 2004,
  pp. 99 to 108. Verified through Crossref, not read. https://doi.org/10.1145/1014052.1014066
- Szegedy, Zaremba, Sutskever et al. 2013, "Intriguing properties of neural networks".
  Full text read. https://arxiv.org/abs/1312.6199
- Goodfellow, Shlens, Szegedy 2014, "Explaining and Harnessing Adversarial Examples".
  Verified. https://arxiv.org/abs/1412.6572
- Goodfellow, Pouget-Abadie, Mirza et al. 2014, "Generative Adversarial Networks". Verified.
  https://arxiv.org/abs/1406.2661
- Biggio, Corona, Maiorca et al., "Evasion Attacks against Machine Learning at Test Time",
  arXiv copy 2017. Title verified. https://arxiv.org/abs/1708.06131
- Emery 2021, "Moral Choices without Moral Language: 1950s Political-Military Wargaming at
  the RAND Corporation", Texas National Security Review 4(4). Fetched.
  https://tnsr.org/2021/09/moral-choices-without-moral-language-1950s-political-military-wargaming-at-the-rand-corporation/
- Zenko 2015, *Red Team: How to Succeed by Thinking Like the Enemy*, Basic Books. Not read;
  cited through Wikipedia.
- Wikipedia, "Red team". Fetched; secondary. https://en.wikipedia.org/wiki/Red_team
- Majumdar, Pendleton, Gupta 2025, "Red Teaming AI Red Teaming". Title verified, claim from
  search. https://arxiv.org/abs/2507.05538
- CNSSI 4009-2022, definition of red team, through the NIST CSRC glossary. Fetched.
  https://csrc.nist.gov/glossary/term/red_team
- Ganguli, Lovitt, Kernion et al. 2022, "Red Teaming Language Models to Reduce Harms:
  Methods, Scaling Behaviors, and Lessons Learned". Verified. https://arxiv.org/abs/2209.07858
- Ahmad, Agarwal, Lampe, Mishkin 2025, "OpenAI's Approach to External Red Teaming for AI
  Models and Systems". Verified. https://arxiv.org/abs/2503.16431
- Burtsell 1907, "Advocatus Diaboli", The Catholic Encyclopedia. Fetched.
  https://www.newadvent.org/cathen/01168b.htm
- John Paul II, 25 January 1983, apostolic constitution *Divinus perfectionis Magister*.
  Fetched.
  https://www.vatican.va/content/john-paul-ii/en/apost_constitutions/documents/hf_jp-ii_apc_25011983_divinus-perfectionis-magister.html
- Wikipedia, "Devil's advocate". Search; secondary. https://en.wikipedia.org/wiki/Devil%27s_advocate
- Britannica, "Devil's advocate". HTTP 403, not read. https://www.britannica.com/topic/devils-advocate
- Zhao, Zhao, Wen, Zhou 2025, "Cognitive Inception: Agentic Reasoning against Visual
  Deceptions by Injecting Skepticism". Verified. https://arxiv.org/abs/2511.17672
- Keramati, Cheok, Horne, Warschauer 2026, "Early-Token Confidence Predicts Reasoning
  Quality in Multi-Agent LLM Debate". Verified; appendix not read.
  https://arxiv.org/abs/2606.10307
- Wang, Li, Deng, Roth, Li 2024, "Devil's Advocate: Anticipatory Reflection for LLM Agents".
  Title verified. https://arxiv.org/abs/2405.16334
- Kim, Kim, Yoon 2024, "DEBATE: Devil's Advocate-Based Assessment and Text Evaluation".
  Title verified. https://arxiv.org/abs/2405.09935
- Widrow, Gupta, Maitra 1973, "Punish/Reward: Learning with a Critic in Adaptive Threshold
  Systems", IEEE Transactions on Systems, Man, and Cybernetics SMC-3(5):455 to 465. Verified
  through Crossref, not read. https://doi.org/10.1109/TSMC.1973.4309272
- Barto, Sutton, Anderson 1983, "Neuronlike adaptive elements that can solve difficult
  learning control problems", IEEE Transactions on Systems, Man, and Cybernetics
  SMC-13(5):834 to 846. Verified through Crossref, not read.
  https://doi.org/10.1109/TSMC.1983.6313077
- Sutton, Barto 2018, *Reinforcement Learning: An Introduction*, second edition, MIT Press.
  Full text read for the history of "critic". http://incompleteideas.net/book/RLbook2020.pdf
- Cobbe, Kosaraju, Bavarian et al. 2021, "Training Verifiers to Solve Math Word Problems".
  Verified. https://arxiv.org/abs/2110.14168
- Liu, Iter et al. 2023, "G-Eval: NLG Evaluation using GPT-4 with Better Human Alignment".
  Title verified. https://arxiv.org/abs/2303.16634
- Irving, Christiano, Amodei 2018, "AI safety via debate". Verified.
  https://arxiv.org/abs/1805.00899
- Fagan 1976, "Design and code inspections to reduce errors in program development", IBM
  Systems Journal 15(3):182 to 211. Verified through Crossref, not read.
  https://doi.org/10.1147/sj.153.0182
- Mellers, Hertwig, Kahneman 2001, "Do Frequency Representations Eliminate Conjunction
  Effects? An Exercise in Adversarial Collaboration", Psychological Science 12(4):269 to 275.
  Verified through Crossref, not read. https://doi.org/10.1111/1467-9280.00350
- Kahneman 2003, "Experiences of collaborative research", American Psychologist
  58(9):723 to 730. Verified through Crossref, not read.
  https://doi.org/10.1037/0003-066X.58.9.723
- APA Monitor 2025, article on adversarial research collaboration. Page returned no content;
  claim from search.
  https://www.apa.org/monitor/2025/04-05/adversarial-research-collaboration
- Software English specification, section 2, "Conformance". Fetched.
  https://github.com/jimbarritt/software-english/blob/main/spec/SPEC.md

Model judges, critique and test gaming:

- Zheng et al. 2023, "Judging LLM-as-a-Judge with MT-Bench and Chatbot Arena". Verified.
  https://arxiv.org/abs/2306.05685
- Panickssery, Bowman, Feng 2024, "LLM Evaluators Recognize and Favor Their Own
  Generations". Verified. https://arxiv.org/abs/2404.13076
- Pombal, Rei, Martins 2026, "Self-Preference Bias in Rubric-Based Evaluation of Large
  Language Models". Full text read for the 20-times figure.
  https://arxiv.org/abs/2604.06996
- Spiliopoulou, Fogliato, Burnsky et al. 2025, "Play Favorites: A Statistical Method to
  Measure Self-Bias in LLM-as-a-Judge". Title verified. https://arxiv.org/abs/2508.06709
- Awuni, Achenie, Partey et al. 2026, "Who Judges Matters: Measuring Family-Conditioned
  Preference in LLM-as-Judge Panels". Title verified, content not read.
  https://arxiv.org/abs/2609.17857
- Krishna-Kumar, Lau, Robinson et al. 2026, "CAREBench: A Child-Safety Risk Benchmark for
  Language Models". Title verified. https://arxiv.org/abs/2606.29685
- Koyama, Wada, Yashima et al. 2026, "MLLM-as-a-Judge Exhibits Model Preference Bias".
  Title verified. https://arxiv.org/abs/2604.11589
- Soumik 2026, "Judging the Judges: A Systematic Evaluation of Bias Mitigation Strategies in
  LLM-as-a-Judge Pipelines". Title verified, content not read.
  https://arxiv.org/abs/2604.23178
- Goel, Struber, Auzina et al. 2025, "Great Models Think Alike and this Undermines AI
  Oversight". Verified. https://arxiv.org/abs/2502.04313
- Madaan, Tandon, Gupta et al. 2023, "Self-Refine: Iterative Refinement with Self-Feedback".
  Verified. https://arxiv.org/abs/2303.17651
- Huang, Chen, Mishra et al. 2023, "Large Language Models Cannot Self-Correct Reasoning
  Yet". Verified. https://arxiv.org/abs/2310.01798
- Kamoi, Zhang, Zhang et al. 2024, "When Can LLMs Actually Correct Their Own Mistakes? A
  Critical Survey of Self-Correction of LLMs". Verified. https://arxiv.org/abs/2406.01297
- Chen, Wang, Zhang et al. 2026, "Learning to Self-Verify Makes Language Models Better
  Reasoners". Title verified. https://arxiv.org/abs/2602.07594
- Du, Li, Torralba et al. 2023, "Improving Factuality and Reasoning in Language Models
  through Multiagent Debate". Verified. https://arxiv.org/abs/2305.14325
- Liang, He, Jiao et al. 2023, "Encouraging Divergent Thinking in Large Language Models
  through Multi-Agent Debate". Verified. https://arxiv.org/abs/2305.19118
- Khan, Hughes, Valentine et al. 2024, "Debating with More Persuasive LLMs Leads to More
  Truthful Answers". Verified. https://arxiv.org/abs/2402.06782
- Bai, Kadavath, Kundu et al. 2022, "Constitutional AI: Harmlessness from AI Feedback".
  Verified. https://arxiv.org/abs/2212.08073
- McAleese, Pokorny, Ceron Uribe et al. 2024, "LLM Critics Help Catch LLM Bugs". Verified.
  https://arxiv.org/abs/2407.00215
- Song, Zhang, Eisenach et al. 2024, "Mind the Gap: Examining the Self-Improvement
  Capabilities of Large Language Models". Abstract verified; the figures on verifier and
  generator capability from search. https://arxiv.org/abs/2412.02674
- Huang, Block, Foster et al. 2024, "Self-Improvement in Language Models: The Sharpening
  Mechanism". Title verified. https://arxiv.org/abs/2412.01951
- Perez, Huang, Song et al. 2022, "Red Teaming Language Models with Language Models".
  Verified. https://arxiv.org/abs/2202.03286
- Zhong, Raghunathan, Carlini 2025, "ImpossibleBench: Measuring LLMs' Propensity of
  Exploiting Test Cases". Full text (v1) read. https://arxiv.org/abs/2510.20270
- Gabor, Lynch, Rosenfeld 2025, "EvilGenie: A Reward Hacking Benchmark". Title verified.
  https://arxiv.org/abs/2511.21654
- Menghini, Ney, Kwisaba et al. 2026, "Muse Spark Safety & Preparedness Report". Title
  verified, use of ImpossibleBench from search. https://arxiv.org/abs/2606.12429
- Foster, Gulati, Harman et al. 2025, "Mutation-Guided LLM-based Test Generation at Meta"
  (ACH). Verified; the expansion of ACH is not. https://arxiv.org/abs/2501.12862

Products and vendor pages:

- Anthropic, "Code Review for Claude Code", 9 March 2026. Fetched.
  https://claude.com/blog/code-review
- Claude Code documentation, Code Review. Fetched. https://code.claude.com/docs/en/code-review
- Claude Code documentation, severity levels section. Not read.
  https://code.claude.com/docs/en/code-review-pull-requests.md#severity-levels
- noqta.tn, secondary report on Claude Code Review.
  https://www.noqta.tn/en/news/anthropic-claude-code-review-multi-agent-pr-2026
- anthropics/claude-code-security-review. Fetched.
  https://github.com/anthropics/claude-code-security-review
- Cursor forum, "Building a better Bugbot". Search.
  https://forum.cursor.com/t/building-a-better-bugbot/149040
- Augment Code, Bugbot guide. Secondary.
  https://www.augmentcode.com/guides/cursor-bugbot-code-review-capabilities-limits
- adwaitx.com, Bugbot figures. Secondary. https://www.adwaitx.com/?p=9869
- GitHub changelog, 28 October 2025, Copilot code review public preview features. Search.
  https://github.blog/changelog/2025-10-28-new-public-preview-features-in-copilot-code-review-ai-reviews-that-see-the-full-picture/
- GitHub changelog, 20 November 2025, linter integration with Copilot code review. Search.
  https://github.blog/changelog/2025-11-20-linter-integration-with-copilot-code-review-now-in-public-preview/
- GitHub changelog, 5 March 2026, Copilot code review on an agentic architecture. Search.
  https://github.blog/changelog/2026-03-05-copilot-code-review-now-runs-on-an-agentic-architecture/
- GitHub Docs, "About GitHub Copilot code review". Fetched.
  https://docs.github.com/en/copilot/concepts/agents/code-review
- GitHub Docs, "Using GitHub Copilot code review". Fetched.
  https://docs.github.com/en/copilot/how-tos/use-copilot-agents/request-a-code-review/use-code-review
- Augment Code, CodeRabbit against Greptile against Augment Cosmos. Vendor.
  https://www.augmentcode.com/tools/coderabbit-vs-greptile-vs-augment-cosmos
- DEV Community, code review benchmark summary. Secondary.
  https://dev.to/tessainsley/a-code-review-benchmark-that-isnt-the-vendor-ranking-itself-4jp6
- gitautoreview.com, summary of the Martian benchmark. Secondary.
  https://gitautoreview.com/blog/ai-code-review-benchmark-2026
- Entelligence, code review benchmark 2026. Vendor.
  https://entelligence.ai/code-review-benchmark-2026
- Tenki, code reviewer benchmark. Vendor. https://tenki.cloud/benchmarks/code-reviewer
- CodePulse, AI code review bots 2026. Competitor.
  https://codepulsehq.com/guides/ai-code-review-bots-2026
- particula.tech, Greptile against CodeRabbit against Qodo. Secondary.
  https://particula.tech/blog/greptile-vs-coderabbit-vs-qodo-ai-code-review-2026

Large runs and practice:

- Wilson Lin, Cursor, "Scaling long-running autonomous coding", 14 January 2026. Fetched.
  https://cursor.com/blog/scaling-agents
- Nicholas Carlini, Anthropic, "Building a C compiler with a team of parallel Claudes", 5
  February 2026. Fetched. https://www.anthropic.com/engineering/building-c-compiler
- Ryan Lopopolo, OpenAI, "Harness engineering", 11 February 2026. HTTP 403, not read.
  https://openai.com/index/harness-engineering/
- rywalker.com, summary of OpenAI harness engineering. Secondary.
  https://rywalker.com/research/openai-harness
- theneuron.ai, OpenAI harness engineering explainer. Secondary.
  https://www.theneuron.ai/explainer-articles/openais-harness-engineering-playbook-how-to-ship-1m-lines-of-code-without-writing-any/
- summify.io, talk summary, "Extreme harness engineering". Secondary.
  https://summify.io/discover/extreme-harness-engineering-for-the-1b-token-day-dark-factor-CeOXx-/
- zenml.io, OpenAI agentic software factory. Secondary.
  https://www.zenml.io/llmops-database/building-an-agentic-software-factory-for-high-velocity-development
- apidog.com, OpenAI agentic software factory. Secondary.
  https://apidog.com/blog/openai-agentic-software-factory/
- OpenAI, "Introducing SWE-bench Verified", August 2024. HTTP 403, unverified.
  https://openai.com/index/introducing-swe-bench-verified/

Related documents in this repository:

- [openai-software-factory.md](orchestration-ecosystem/openai-software-factory.md)
- [claude-code-dynamic-workflows.md](orchestration-ecosystem/claude-code-dynamic-workflows.md)
- [entire.md](orchestration-ecosystem/entire.md)
- [ledger-layout.md](../domain/ledger-layout.md)
