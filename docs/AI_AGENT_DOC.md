# Coordination Without Omniscience

*A reader on robot and drone coordination, and on making policies inside the sandbox.*

Written by the assistant at the user's request, 2026-09-07. Everything here is a
summary of published work or a proposal; nothing is a decision. Where a chapter names
a sandbox component it refers to `docs/DESIGN.md` and the code as they stood on the
date above.

---

## Preface

Every chapter in this book is about the same sentence: *nobody sees the whole map and
nobody can talk to everybody.* Fifty years of robotics has attacked that sentence from
a dozen directions — control theory, economics, biology, game theory, machine
learning, command doctrine, rescue robotics — and each direction left behind a
vocabulary and a set of results. This book walks through them in an order that builds,
so that by the end a reader can look at the sandbox and recognise which shelf each part
came from.

The scenario behind the sandbox is a rescue. A flooded plain; one boat; five cheap
drones carried in a backpack; more people to reach than the boat can carry. The drones
go ahead to find survivors, and finding is not a matter of flying over and looking
down — people are under roofs, in debris, in culverts, and a drone that goes in to
look may not come out. The boat needs to know where the people are, which drones are
gone, and what the gone drones saw before they went. The arena in this repository is a
stepping stone toward that: two teams contesting scarce resources on a map nobody can
see whole, with the diep.io rules borrowed because they are simple and the author
likes them. The contest is a game mechanic. Nothing here is built for warfare, and
Chapter 13 is about rescue.

It is written to be read, not consulted. Chapters are short and each ends with a
reading list. The last two chapters turn from summary to proposal: where the sandbox
sits among all this, and what could be done with it that has not been done.

A note on words. "Robot", "drone", "agent", "vehicle" and "tank" are used
interchangeably when the distinction does not matter. "Coordination" means the joint
behaviour of several decision-makers; "control" means the behaviour of one.

---

# Part I — The Problem

## Chapter 1. The Shape Of The Problem

Start with the formal object, because it tells you why everything else exists.

A group of agents acting in a shared world, each seeing part of it, each choosing
actions, sharing one reward, is a **decentralised partially observable Markov decision
process [Dec-POMDP]**. Bernstein and colleagues showed in 2002 that solving one
optimally is NEXP-complete — not merely hard, but a full exponential harder than the
single-agent POMDP, which is itself PSPACE-hard. Two agents with a handful of states
and a horizon of ten are already beyond exact solution.

The reason is not the size of the world. It is that each agent must reason about what
every other agent might have seen and might therefore do, and that reasoning nests:
what I think you think I saw. Communication collapses the problem — Pynadath and Tambe
showed in 2002 that free, instantaneous communication reduces a Dec-POMDP to a single
large POMDP — which is precisely why the interesting cases are the ones where
communication is not free. Every constraint on the link puts the nesting back.

So the field is organised by the three ways of escaping the nesting:

1. **Structure.** Fix the form of the policy so that the search space is small.
   Flocking rules, potential fields, behaviour trees, doctrines. Part II.
2. **Decomposition.** Break the joint problem into pieces that can be solved
   separately and stitched: who does what, who goes where, who knows what.
   Parts III and IV.
3. **Learning.** Search the policy space by trial against a simulator instead of
   solving it. Part V.

Each escape trades optimality for tractability in a different currency, and much of the
literature is an argument about the exchange rate.

Two taxonomies are worth carrying. The first classifies the *architecture*:
centralised (one planner, everyone else executes), hierarchical (planners over
planners), distributed (peers, no centre), and holonic (units that are agents at one
scale and teams at another). The second, due to Gerkey and Matarić in 2004, classifies
the *allocation problem* along three axes — single- or multi-task robots, single- or
multi-robot tasks, instantaneous or time-extended assignment — and it is still the
standard way to say what kind of coordination problem you have before saying how you
solve it.

The sandbox sits at a specific point in both. Its architecture is hierarchical with a
range-limited centre: a control centre that plans but cannot see, five tanks that see
but cannot plan far. Its allocation problem is single-task robots on single-robot
tasks with time-extended assignment, which is the class where auctions and roles do
well. Chapter 14 returns to this.

**Reading.**
Bernstein, Givan, Immerman, Zilberstein, *The complexity of decentralized control of
Markov decision processes*, 2002.
Oliehoek and Amato, *A Concise Introduction to Decentralized POMDPs*, 2016.
[Gerkey and Matarić, *A formal analysis and taxonomy of task allocation in multi-robot
systems*, 2004](https://journals.sagepub.com/doi/10.1177/0278364904045564).
[Amato et al., *Decentralized control of partially observable Markov decision
processes*](https://www.ccs.neu.edu/home/camato/publications/DecPOMDP-survey-final.pdf).

---

# Part II — Control Without A Centre

## Chapter 2. Flocks, Fields And Consensus

The oldest escape from the nesting is to give every agent the same tiny rule and let
the group behaviour fall out of it.

Craig Reynolds's boids of 1987 are the founding example: separation, alignment,
cohesion, each a vector computed from neighbours within a radius, summed and applied.
Three rules and a radius produce flocks that split around obstacles and rejoin. The
physicists formalised it: Vicsek's model of 1995 — align with the mean heading of
neighbours plus noise — shows a phase transition from disorder to collective motion as
density rises or noise falls, and Couzin's 2002 model with concentric zones of
repulsion, orientation and attraction produces swarms, tori and polarised groups
depending only on the zone widths. The lesson from both is that group behaviour can be
a *phase* of a parameter, not a program, and that a small change in a radius can flip
the whole group from one phase to another.

Khatib's artificial potential fields of 1986 give the same idea a control-theoretic
form: the goal attracts, obstacles repel, the robot descends the gradient. The method
is cheap, reactive and composable, and it has one famous flaw — local minima, where the
attractions and repulsions cancel and the robot stops short of the goal. Every
potential-field system since has been an answer to that flaw: harmonic potentials,
navigation functions, random perturbation, or a planner layered above to pick
waypoints the field can reach.

Consensus is the third strand. Jadbabaie, Lin and Morse proved in 2003 that Vicsek's
alignment converges if the neighbour graph stays connected often enough; Olfati-Saber
and Murray in 2004 gave the general framework — each agent moves toward the average of
its neighbours, and the group converges to agreement at a rate set by the second
eigenvalue of the graph Laplacian. Ren and Beard's 2005 survey is the standard entry
point, and Olfati-Saber's 2006 flocking paper unites the strands: a potential for
spacing, a consensus term for velocity, a navigation term toward a goal, with proofs
that the flock forms and does not fragment.

Formation control inherits three families from this: **leader-follower** (one agent
holds the reference, the rest keep offsets from it — simple, and it fails when the
leader is lost), **virtual structure** (the formation is a rigid body every agent
tracks its slot in — robust, and it needs everyone to agree on the body's pose), and
**behaviour-based** (each agent sums weighted drives — the Reynolds inheritance,
flexible, and hard to prove anything about).

What this buys the sandbox is direct. The drives layer of a doctrine is a potential
field with named terms; the leash is a cohesion term with a dead zone; separation is
Reynolds's first rule. The point of carrying the theory is to know the failure modes in
advance: the local minimum where cohesion and avoidance cancel and a tank sits still,
and the fragmentation where a leash long enough to explore is a leash long enough to
partition the network.

**Reading.**
Reynolds, *Flocks, herds and schools*, SIGGRAPH 1987.
Vicsek et al., *Novel type of phase transition in a system of self-driven particles*,
1995. Couzin et al., *Collective memory and spatial sorting in animal groups*, 2002.
Khatib, *Real-time obstacle avoidance for manipulators and mobile robots*, 1986.
[Ren and Beard, *A survey of consensus problems in multi-agent
coordination*](http://www.et.byu.edu/~beard/papers/reprints/RenBeard05a.pdf), 2005.
Olfati-Saber, *Flocking for multi-agent dynamic systems*, 2006.
[Oh, Park, Ahn, *A survey of multi-agent formation
control*](https://www.sciencedirect.com/science/article/abs/pii/S0005109814004038), 2015.

## Chapter 3. Behaviours, Layers And Trees

The second way to structure a policy is not a field but a program with a particular
shape.

Rodney Brooks's subsumption architecture of 1986 was a rebellion against the
sense-model-plan-act pipeline. Behaviours are stacked; each is a complete loop from
sensors to motors; higher layers *subsume* lower ones by overriding their outputs. No
world model, no planner, and robots that walked across cluttered rooms while the
planners were still building maps. Ronald Arkin's motor schemas, collected in
*Behavior-Based Robotics* in 1998, softened the override into a sum: schemas run in
parallel and their vector outputs blend, which is Reynolds again with a robotics
vocabulary.

Pure reaction hit its ceiling on tasks that need memory and sequencing, and the field
settled on the **three-layer architecture** — Firby's reactive action packages, Gat's
1998 statement of the pattern, Bonasso's 3T. A deliberative layer plans slowly, a
sequencing layer selects which behaviour runs now, a reactive layer runs it at sensor
rate. The layers communicate by narrow interfaces and run at different clocks. Almost
every fielded autonomy stack since has this shape whether it says so or not, and the
sandbox has it explicitly: the control centre deliberates, the doctrine's stance list
sequences, the drives react.

The sequencing layer is where **behaviour trees** live. They came from game AI around
2005, and Colledanchise and Ögren's 2018 book made them a robotics standard. A tree of
control nodes — sequence, fallback, parallel, decorator — over leaf conditions and
actions, ticked at a fixed rate from the root. The properties that matter: a tree is
modular (subtrees compose without knowing about each other), reactive (every tick
starts at the root, so a higher-priority branch pre-empts a running one), and readable
(the tree *is* the documentation). Empirical comparisons against finite state machines
— Ghzouli and colleagues on real robotics repositories in 2020, and a controlled study
of mission comprehension and modification in 2025 — find trees easier to modify and
harder to get wrong as missions grow, at the cost of being less obvious for strictly
sequential tasks.

Above trees sit the **mission specification languages**. Linear temporal logic [LTL]
lets a mission be stated as a formula ("eventually reach A, always avoid B, whenever
C then next D") and compiled to a controller that provably satisfies it — Kress-Gazit,
Fainekos and Pappas in 2009 — at the cost of a state explosion that limits it to small
missions. Hierarchical task networks and PDDL planners decompose goals into ordered
actions and hand them to a tree or a machine to execute. A 2026 comparative survey of
these formalisms concludes what practitioners already knew: nobody uses one, and the
useful question is which formalism owns which layer.

Swarm-specific languages deserve a mention. Pinciroli and Beltrame's **Buzz** of 2016
is a scripting language whose primitives are swarm operations — neighbour queries,
dynamic sub-swarms, and *virtual stigmergy*, a replicated key-value store that
reaches consensus across the swarm over local links. It is the clearest existing
example of a language designed around the sentence this book opens with.

The sandbox's doctrine is a behaviour tree with the branches flattened into an ordered
list of guarded stances, which is the special case of a fallback node over sequences of
one condition and one action. That is a deliberate narrowing, not an oversight: a
flat list is what a small language model can be asked to edit, and the general tree is
one refactor away if the flat list runs out.

**Reading.**
Brooks, *A robust layered control system for a mobile robot*, 1986.
Arkin, *Behavior-Based Robotics*, MIT Press 1998.
Gat, *On three-layer architectures*, 1998.
[Colledanchise and Ögren, *Behavior Trees in Robotics and AI: An
Introduction*](https://arxiv.org/abs/1709.00084), 2018.
[Ghzouli et al., *Behavior trees in action*](https://www.cse.chalmers.se/~bergert/paper/2020-sle-behaviortrees.pdf), 2020.
[*Effects of specifying robotic missions in behavior trees and state
machines*](https://www.sciencedirect.com/science/article/abs/pii/S2590118425000164), 2025.
[*Formalisms for robotic mission specification and execution: a comparative
analysis*](https://arxiv.org/html/2603.15427v2), 2026.
[Pinciroli and Beltrame, *Buzz: an extensible programming language for self-organizing
heterogeneous robot swarms*](https://arxiv.org/abs/1507.05946), 2016.

## Chapter 4. Swarms

A swarm is a group large enough that individuals do not matter. That definition sounds
flippant and is exact: swarm methods are the ones that still work when any member may
fail, that scale without re-planning, and that make no member special.

The biological source is **stigmergy** — coordination through marks left in the
environment. Ants lay pheromone; the trail is the plan; no ant holds it. Engineered
stigmergy replaces pheromone with shared data that decays, and Buzz's virtual
stigmergy (Chapter 3) shows it working over radio. The attraction for a
communication-limited team is that a mark is written once and read by whoever passes,
which is a message with no recipient and no deadline.

Rubenstein, Cornejo and Nagpal's Kilobots of 2014 are the canonical demonstration: a
thousand cheap robots, each knowing only distance to neighbours, self-assembling into
a shape from a seed and a gradient. The result is not that the shape formed but that
it formed with local rules only, and that the failure modes — a robot stuck, a gradient
propagating wrong — were absorbed by the rest.

The military rediscovered swarms through DARPA's **OFFSET** program, run from 2017 to
2021 across six field experiments. Its framing is the one that matters for this book:
the unit of work was the **swarm tactic**, a reusable behaviour that implements a
commander's intent, and the program's stated goals were tools to *generate* tactics
quickly, *evaluate* them, and *integrate* the good ones. The final experiment at Fort
Campbell ran over three hundred air and ground platforms, mixed virtual agents with
physical ones in the same mission, and commanded them through sketch tablets, phones
and virtual reality. The program also produced sober findings about congestion —
hundreds of platforms in an urban block spend much of their time in each other's way —
which is the swarm version of the local minimum.

What the sandbox takes from swarms is the idea of a tactic as a named, evaluable unit,
and the discipline of designing for members that drop out. What it does not take is
scale: five tanks is a team, not a swarm, and the coordination pressures the design
wants only exist because each member matters.

**Reading.**
Rubenstein, Cornejo, Nagpal, *Programmable self-assembly in a thousand-robot swarm*,
Science 2014.
[DARPA, *OFFensive Swarm-Enabled Tactics*](https://www.darpa.mil/research/programs/offensive-swarm-enabled-tactics).
[*OFFSET swarms take flight in final field experiment*](https://www.darpa.mil/news/2021/offset-swarms-take-flight), 2021.
[*Congestion analysis for the DARPA OFFSET CCAST swarm*](https://arxiv.org/pdf/2307.16788), 2023.
[Chung et al., *DARPA OFFSET: a vision for advanced swarm systems*](https://ieeexplore.ieee.org/document/10876037/), 2025.

---

# Part III — Deciding Who Does What

## Chapter 5. Task Allocation And Markets

Once policies have shape, the next question is assignment: which agent takes which
job. This is the best-developed corner of the field because it reduces, often exactly,
to problems economists and operations researchers had already solved.

The simplest case — each robot one task, each task one robot, assign now — is the
linear assignment problem, and the Hungarian algorithm solves it optimally in cubic
time given a central table of costs. Everything else in the chapter is about what to
do when there is no central table.

**Markets** are the standard answer. Reid Smith's contract net protocol of 1980 has a
manager announce a task, contractors bid, the manager awards. Auctions generalise it:
robots bid their cost to do a task, the lowest bid wins, and the bids themselves carry
the private information — position, battery, current load — that a central planner
would have had to collect. Sequential single-item auctions are greedy and fast;
combinatorial auctions handle tasks whose value depends on bundling and are expensive
to clear. The 2022 survey by Seenu and colleagues is a good map of the variants.

The step that matters for a range-limited team is removing the auctioneer. Choi,
Brunet and How's **consensus-based bundle algorithm [CBBA]** of 2009 has each robot
build a bundle of tasks it would like, then run a consensus over local links on who
holds the winning bid for each task, resolving conflicts by a fixed rule. It provably
converges under a connected graph and degrades rather than breaks under an
intermittent one, and it has been the base for a decade of variants — including work
specifically on UAVs with limited communication range.

**Roles** are a coarser currency than tasks and often a better one. Lynne Parker's
ALLIANCE of 1998 gave each robot *motivational* variables — impatience rising while a
task goes undone, acquiescence rising while the robot fails at it — that switch roles
without any negotiation at all. A robot that watches a teammate not doing the job
eventually takes it. The mechanism is robust to lost messages because it needs none;
it only needs observation. The sandbox's `Role` vocabulary and the design's interest in
role entropy sit squarely in this tradition: a fixed, small set of roles is what makes
"who took which role, and when" a countable thing.

**Reading.**
Smith, *The contract net protocol*, 1980.
[Gerkey and Matarić, 2004](https://journals.sagepub.com/doi/10.1177/0278364904045564)
(the taxonomy, again).
[Choi, Brunet, How, *Consensus-based decentralized auctions for robust task
allocation*](https://dl.acm.org/doi/10.1109/tro.2009.2022423), 2009.
Parker, *ALLIANCE: an architecture for fault tolerant multirobot cooperation*, 1998.
[Seenu et al., *Market approaches to the multi-robot task allocation problem: a
survey*](https://link.springer.com/article/10.1007/s10846-022-01803-0), 2022.
[*A systematic literature review on multi-robot task
allocation*](https://dl.acm.org/doi/10.1145/3700591), 2024.
[*Decentralized dynamic task allocation for UAVs with limited communication
range*](https://arxiv.org/pdf/1809.07863).

## Chapter 6. Moving Together

Assignment says where each agent should be. Motion says how they get there without
colliding, and exploration says where to go when nobody has said.

**Multi-agent path finding** on a grid is NP-hard in general and solved in practice by
two families. Prioritised planning orders the agents and plans each around the paths
already fixed — fast, incomplete, and fine when the space is open. Conflict-based
search, due to Sharon and colleagues in 2015, plans everyone independently, finds the
first collision, and branches on which agent must yield, searching the tree of
constraints for an optimal joint plan. It is the standard for warehouses and its
bounded-suboptimal variants scale to hundreds of agents.

In continuous space with no central planner the tool is **velocity obstacles**: the set
of velocities that would collide with a neighbour within a horizon, computed from
relative position and velocity. Van den Berg's reciprocal velocity obstacles of 2008
and ORCA of 2011 split the avoidance between the two agents so that both adjusting
does not overshoot, and they run at thousands of agents in real time. This is the
right tool for a tank threading a nest full of shapes, and it composes with a
potential field as a final filter on the commanded velocity.

**Exploration** of unknown space has one dominant idea. Yamauchi's frontier-based
exploration of 1997 defines a frontier as the boundary between known-free and unknown
cells and sends the robot to the nearest one; his 1998 extension to multiple robots
has each share its map and pick frontiers independently. Everything since is a better
choice of frontier: by expected information gain, by travel cost, by assigning
frontiers through an auction so that two robots do not pick the same one. Coverage
control — Cortés, Martínez, Karatas and Bullo in 2004 — is the continuous cousin: each
robot moves to the centroid of its Voronoi cell weighted by where coverage is wanted,
and the team spreads to an optimal partition with local rules.

**Pursuit, tracking and patrol** form a family with its own taxonomy, laid out by
Robin and Lacroix in 2016: coverage, search, surveillance, patrolling, observation,
pursuit-evasion. Persistent surveillance minimises the *idleness* of points — time
since last visit — and with connectivity constraints becomes a scheduling problem
where the patrol route must also keep the team in radio contact. Multi-target tracking
with several observers is data association plus assignment: which detection is which
target, and which observer should follow it.

For the sandbox: the scout's exploration drive is frontier-based over the world model;
target choice is an assignment; the nest is a coverage problem for the screen; and the
question "who watches the flank" is patrolling with idleness. None of this needs
inventing.

**Reading.**
Sharon, Stern, Felner, Sturtevant, *Conflict-based search for optimal multi-agent
pathfinding*, 2015.
Van den Berg, Lin, Manocha, *Reciprocal velocity obstacles*, 2008; *Reciprocal n-body
collision avoidance* (ORCA), 2011.
Yamauchi, *A frontier-based approach for autonomous exploration*, 1997; *Frontier-based
exploration using multiple robots*, 1998.
Cortés, Martínez, Karatas, Bullo, *Coverage control for mobile sensing networks*, 2004.
[Robin and Lacroix, *Multi-robot target detection and tracking: taxonomy and
survey*](https://link.springer.com/article/10.1007/s10514-015-9491-7), 2016.
[*Multi-robot persistent surveillance with connectivity
constraints*](https://arxiv.org/pdf/1909.07703).

---

# Part IV — Knowing Together

## Chapter 7. Belief, Fusion And Rumour

An agent that cannot see the whole map holds a belief, and a team holds several. The
question of this chapter is how to merge them without lying to yourself.

The foundation is estimation. A Kalman filter holds a mean and a covariance; in
information form it holds the inverse, and information from independent sources
simply adds. That additivity is what makes **decentralised data fusion [DDF]**
attractive: each node runs its own filter and exchanges information increments with
neighbours, and if every increment is counted exactly once the network converges on
what a central filter would have computed.

The catch is the word *once*. On any graph with a cycle, an increment that A sends to B
can come back to A through C, and A will add it again. This is **double counting**,
also called data incest or **rumour propagation**, and its effect is an estimate that
grows more confident than the evidence warrants — a track whose uncertainty shrinks
because three teammates repeated one sighting to each other. In a team it is worse
than noise, because confident and wrong is what gets a tank killed.

Exact remedies exist for trees (channel filters that subtract what was already sent
down each edge) and fail on general graphs. The general remedy is **covariance
intersection**, due to Julier and Uhlmann in 1997: when two estimates have unknown
correlation, take a convex combination of their information matrices with a weight
chosen to minimise the result's size. The fused estimate is guaranteed consistent — it
never claims more certainty than it has — at the cost of being pessimistic when the
sources were in fact independent. Inverse covariance intersection and the factor-graph
DDF of recent years recover some of that lost information when the structure of the
dependence is known.

The sandbox's design already encodes the lesson. `ingest_scan` and `ingest_belief`
are separate methods because a scan is your own evidence and a belief is hearsay with
an age and a source, and a model that cannot tell them apart will let teammates inflate
each other. The uncertainty radius that grows with track age is the scalar version of
a covariance that grows under prediction. A world model that used naive addition and
one that used covariance intersection would be a clean pair of experiments — the
first should show rumour propagation and the second should not — and the design's
`Raw` message variant exists so both can be on the same team.

**Reading.**
Julier and Uhlmann, *A non-divergent estimation algorithm in the presence of unknown
correlations*, 1997.
Durrant-Whyte and Henderson, *Multisensor data fusion*, Springer Handbook of Robotics.
[*Decentralized data fusion with inverse covariance
intersection*](https://www.researchgate.net/publication/314194756_Decentralized_data_fusion_with_inverse_covariance_intersection).
[Dagan and Ahmed, *Exploiting structure for optimal multi-agent Bayesian
decentralized estimation*](https://arxiv.org/pdf/2307.10594), 2023.
[*Distributed data fusion: neighbors, rumors, and the art of collective
knowledge*](https://www.researchgate.net/publication/305735903_Distributed_Data_Fusion_Neighbors_Rumors_and_the_Art_of_Collective_Knowledge).

## Chapter 8. Talking On A Budget

If communication were free the problem would be a POMDP. It is not free, so the
literature splits into three questions: *when* to talk, *what* to say, and *how to
stay in range*.

**When.** The Dec-POMDP with communication [Dec-POMDP-Com] adds a message action and a
cost. Roth, Simmons and Veloso's 2005 work is the reference: each agent maintains the
set of joint beliefs the team could hold, and communicates only when its local
observation would change the team's chosen action — the *value* of a message is the
improvement in the joint decision it causes, and a message below cost is not sent.
This is the principle behind every event-triggered scheme since: silence is
information too, because a teammate who has not spoken has seen nothing worth saying.

**What.** Bandwidth-limited belief sharing chooses a compression. Sending a map is
expensive; sending a track set is cheap; sending an *intent* — what I plan to do — is
cheapest and often most valuable, because it lets the receiver plan against it without
needing to know why. The sandbox's `BeliefMsg` union names exactly these payloads, and
its `encode(budget, interest)` method is the design's statement that choosing what to
send under a byte cap is the whole job of a world model. The recent "goal-oriented" or
"semantic" communication literature in wireless systems is converging on the same
view from the other side: transmit what changes the receiver's decision, not what
describes the world.

**How to stay in range.** A team that moves can lose its network. The graph-theoretic
handle is the second-smallest Laplacian eigenvalue, the **Fiedler value** or algebraic
connectivity: positive if and only if the graph is connected, larger when more robustly
so. Zavlanos and Pappas showed in the late 2000s how to keep it positive with a
distributed control law that pulls agents back before a link breaks, and later work
maintains a lower bound under motion and sensing uncertainty via distributed
optimisation. The alternative is to give up on continuous connectivity: **relay
chains** place agents purely to carry messages, and **intermittent rendezvous** plans
for teams to separate, explore, and meet at scheduled points to merge maps — recent
work formulates the schedule as a job-shop problem inside a Dec-POMDP and combines
scheduled with opportunistic meetings.

The sandbox's central question — does a tank ever choose to be the relay — is the
Fiedler-value question asked of a policy instead of a controller. Every method in this
chapter is a candidate answer, and the design's comms radii were chosen so that no
answer is free.

**Reading.**
[Roth, Simmons, Veloso, *Decentralized communication strategies for coordinated
multi-agent policies*](https://link.springer.com/chapter/10.1007/1-4020-3389-3_8), 2005.
Zavlanos and Pappas, *Distributed connectivity control of mobile networks*, 2008.
[*Connectivity maintenance for multi-robot systems under motion and sensing
uncertainties*](https://arxiv.org/abs/2012.09808).
[*Swarm relays: distributed self-healing ground-and-air connectivity
chains*](https://arxiv.org/pdf/1909.10496).
[*Communication-constrained multi-robot exploration with intermittent
rendezvous*](https://arxiv.org/abs/2309.13494), 2023.
[*Toward goal-oriented communication in multi-agent systems: an
overview*](https://arxiv.org/pdf/2508.07720), 2025.

---

# Part V — Learning To Coordinate

## Chapter 9. Multi-Agent Reinforcement Learning

The third escape is to stop designing the policy and search for it.

Single-agent reinforcement learning assumes a stationary environment. With several
learners the environment contains the others, and it moves as they learn — the
non-stationarity problem, which breaks the convergence guarantees and, in practice,
the training. The field's answer is **centralised training with decentralised
execution [CTDE]**: during training a critic sees everything, including the other
agents' actions; at execution each agent runs a policy that sees only its own
observation. The critic is scaffolding, thrown away at deployment.

Two families dominate. **Value decomposition** — VDN in 2018, QMIX the same year,
QPLEX after — learns a joint action-value as a monotonic mix of per-agent values, so
that each agent can pick its own greedy action and the joint pick is still greedy for
the team. **Centralised critics** — MADDPG in 2017, COMA in 2018, MAPPO in 2022 — train
actor policies against a critic with global information. MAPPO's finding, that a
carefully tuned on-policy method matches the off-policy specialists, has made it the
default baseline. The standard testbeds are the StarCraft Multi-Agent Challenge [SMAC]
and its harder successor SMACv2, Google Research Football, and the particle worlds; the
most famous demonstrations are OpenAI's hide-and-seek of 2019, where teams discovered
tool use through six phases of counter-strategy, and AlphaStar's grandmaster-level
StarCraft II in the same year.

**Emergent communication** puts the channel inside the learning problem. Foerster's
RIAL and DIAL of 2016 let agents learn what to send by backpropagating through the
message; CommNet of the same year averages continuous messages across the team;
TarMAC of 2019 adds attention so agents learn *whom* to address. The results are real
and the caveats are real. Learned protocols are brittle to a teammate that did not
train with you, hard to interpret, and prone to solving the task without using the
channel at all when the task allows it. A 2025 line of work on *engineered* versus
*emergent* communication argues for fixing the message vocabulary and learning only
when to use it — which is the sandbox's position, with `BeliefMsg` fixed and the
policy free.

The place of all this in the sandbox is the socket bridge. A learned policy is a
process on the other end of it, seeing the same `Observation` a scripted one sees, and
the fixed message union is what lets a learned tank and a scripted tank be on one team.
The sandbox is, in the terms of this chapter, a Dec-POMDP environment with a
configurable communication graph — which is the thing Chapter 10's most relevant
paper was missing and had to build.

**Reading.**
Rashid et al., *QMIX*, 2018. Lowe et al., *MADDPG*, 2017. Yu et al., *The surprising
effectiveness of PPO in cooperative multi-agent games* (MAPPO), 2022.
[Samvelyan et al., *SMAC*, 2019; Ellis et al., *SMACv2*](https://arxiv.org/pdf/2212.07489), 2023.
Foerster et al., *Learning to communicate with deep multi-agent reinforcement
learning* (RIAL, DIAL), 2016. Sukhbaatar, Szlam, Fergus, *CommNet*, 2016.
[Das et al., *TarMAC: targeted multi-agent communication*](https://arxiv.org/abs/1810.11187), 2019.
[*Engineered over emergent communication in MARL*](https://www.arxiv.org/pdf/2508.02912), 2025.
[Amato, *A first introduction to cooperative multi-agent reinforcement
learning*](https://www.ccs.neu.edu/home/camato/publications/IntroMARL.pdf).

## Chapter 10. Evolution, Diversity And Games

Gradient-free search predates deep MARL and has kept two things it never had: a map of
the space searched, and an opponent.

**Evolutionary robotics** — Nolfi and Floreano's book of 2000 is the summary — encodes
a controller as a genome, runs it in simulation, scores it, and breeds. It handles
non-differentiable controllers, discrete structures like trees, and reward that arrives
once per episode, all of which describe a doctrine exactly. Its weakness is sample
cost, which a fast headless simulator largely removes.

**Quality diversity** changed what evolution is for. Mouret and Clune's MAP-Elites of
2015 does not look for the best controller; it looks for the best controller *in each
cell* of a grid of behavioural descriptors — say, distance travelled by fire volume —
and returns the whole grid. The result is an atlas: here is the best cautious
explorer, here the best aggressive camper, here the region nobody can fill. Applied to
swarms it has produced repertoires of behaviour trees that can be composed into
heterogeneous teams or switched in when one fails, and Swarm-Map-based Bayesian
optimisation uses the atlas as a prior for adapting a swarm in the field in a handful
of trials.

**Empirical game theory** brings the opponent. Rather than solve a game analytically,
play every strategy in a library against every other, record the payoff matrix, and
solve *that*. McMahan's double oracle of 2003 grows the library by adding, at each
round, the best response to the current equilibrium; Lanctot and colleagues'
policy-space response oracles [PSRO] of 2017 generalise it with learned best responses
and arbitrary meta-solvers. Its outputs are exactly what a researcher wants from a
sandbox: an exploitability number for each strategy, and a mixed strategy nobody can
exploit much.

The most relevant single paper to this project applies all of it. **UC-PSRO** of 2026
generates courses of action for a blue drone swarm against an adaptive red side in a
communication-degraded environment. Commander's intent is a five-part reward
decomposition — mission progress, survivability, neutralisation, time, risk — and the
policy is *conditioned* on the weight vector over those parts, resampled from a
Dirichlet each episode, so that one trained policy can be re-steered at execution time
by changing the weights. A **communication-dropout curriculum** anneals random edge
drops on the comms graph from zero to a maximum during training. PSRO self-play runs
over both sides. The result worth memorising is the one the authors call
counterintuitive: the dropout curriculum alone was the most effective component, and
success rate *rose* with test-time dropout, from 35 to 62 per cent as the drop
probability went from 0 to 0.75 — a team trained to expect silence coordinated better
in silence than a team trained on a perfect link coordinated with one. Intent
conditioning and PSRO slowed convergence within the budget without a clear gain. The
adversary in that paper is a stand-in for contested, uncertain conditions; what
transfers to a rescue is the dropout finding, not the red team.

That paper needed a synthetic stand-in environment with a configurable communication
graph and a notion of commander's intent. The sandbox is that environment, built to be
handed to other people.

**Reading.**
Nolfi and Floreano, *Evolutionary Robotics*, MIT Press 2000.
Mouret and Clune, *Illuminating search spaces by mapping elites*, 2015.
[*A quality-diversity approach to evolving a repertoire of diverse behaviour-trees in
robot swarms*](https://link.springer.com/chapter/10.1007/978-3-031-30229-9_10), 2023.
[*Rapidly adapting robot swarms with swarm map-based Bayesian
optimisation*](https://arxiv.org/pdf/2012.11444).
Lanctot et al., *A unified game-theoretic approach to multiagent reinforcement
learning* (PSRO), 2017. [*Policy space response oracles: a
survey*](https://www.ijcai.org/proceedings/2024/0880.pdf), 2024.
[*UC-PSRO: utility-conditioned policy-space response oracles with a
communication-dropout curriculum*](https://arxiv.org/html/2608.15372), 2026.

---

# Part VI — Humans And Commanders

## Chapter 11. Commanders, Intent And Playbooks

Organisations that operate under bad communication solved decentralised coordination
before robots existed, and their vocabulary is worth borrowing precisely.

**Mission command**, from the Prussian *Auftragstaktik*, is the doctrine that a
commander states the *intent* — the purpose and the desired end state — and the
constraints, and leaves the method to subordinates. It exists because orders do not
survive contact and radios do not survive terrain; a unit that knows *why* can act
when it can no longer be told *what*. **Commander's intent** is therefore a compact
object designed to be transmitted once and to remain valid when the link is gone. Rules
of engagement are its complement: what is forbidden regardless of intent. And Boyd's
**OODA loop** — observe, orient, decide, act — is the claim that tempo wins: the side
that cycles faster forces the other to act on stale orientation.

Civilian emergency response has the same structure under the Incident Command System:
a span of control of three to seven, unified command across agencies, objectives
stated per operational period, and units expected to act on the objective when the
radio is busy. The control centre is an incident commander as much as a military one,
and the boat in the scenario is exactly that.

Human-factors research turned these into interfaces. Miller and Parasuraman's
**Playbook** of 2007 is the key result for this book. Operators supervised up to eight
simulated unmanned vehicles in a capture-the-flag game against an equal opposing team,
using either waypoint control, a library of named "plays" they could call and lightly
parameterise, or both. Delegation by play raised mission success and cut completion
time, and the effect was strongest when the opponent's posture — offensive or
defensive — was unpredictable. The finding generalises: a human commanding a team does
better choosing among named, bounded behaviours than specifying behaviour directly,
and does best when they can do either. Later work on *levels of automation* for swarms
places every interface on a spectrum from teleoperation to full delegation and finds
the useful region in the middle, with the operator's span of control set by how much
each play absorbs.

OFFSET (Chapter 4) is the same finding at scale: a single operator commanded hundreds
of platforms through a tactic library and a sketch interface, and the research
question was how to author and evaluate tactics faster, not how to fly.

The sandbox's control centre is a Playbook. A doctrine library is a set of plays;
`SetObjective` is intent; `AssignRole` and `SetRally` are the light parameterisation;
and the design's insistence that a human at the centre and a policy at the centre emit
the same `Command` is what lets a human's play-calling be compared with an algorithm's
on the same footing Miller and Parasuraman used.

**Reading.**
[Miller and Parasuraman, *Designing for flexible interaction between humans and
automation: delegation interfaces for supervisory
control*](https://journals.sagepub.com/doi/10.1518/001872007779598037), 2007.
[Walker et al., *Levels of automation for human influence of robot
swarms*](https://www.ri.cmu.edu/pub_files/2014/9/walkerHFES2013loa-final-CR.pdf), 2013.
[*Delegation in human-machine teaming: progress, challenges and
prospects*](https://link.springer.com/chapter/10.1007/978-3-030-68017-6_2), 2021.
Boyd, *Patterns of Conflict*, 1986 (briefing). Shamir, *Transforming Command: The
Pursuit of Mission Command*, 2011.

## Chapter 12. Language Models As Commanders

The newest layer arrived in 2022 and slotted into the oldest architecture. A language
model is a deliberative planner with an unusual interface: it reads and writes text,
knows a great deal about tasks in general and nothing about this world in particular,
and takes seconds to answer. Every successful system of the last four years has put it
in the top layer and kept it out of the loop.

The early results set the pattern. SayCan of 2022 had the model score candidate skills
against a task and let a learned affordance function veto the infeasible ones.
Code-as-Policies the same year had it write short programs against a fixed API of
perception and control primitives. Both are the model choosing among and composing
*given* primitives, not inventing control.

Multi-robot systems followed. SMART-LLM of 2023 decomposes an instruction, forms
coalitions, and allocates sub-tasks across a heterogeneous team; RoCo has the robots'
models negotiate in dialogue and folds collision feedback back into the plan. The
2025–2026 crop converges on **structured outputs**: MRBTP prompts with a JSON
description of condition and action predicates and gets back a JSON plan per robot,
"allowing for the definition of a precise JSON schema that ensures strict adherence";
LLM-HBT builds behaviour trees dynamically for heterogeneous teams; H-AIM has the model
write a PDDL problem, hands it to a classical planner, and compiles the result to a
tree for reactive execution. Drone-specific work — Swarm-GPT for choreography,
SwarmChat for multimodal swarm interaction, a Web-of-Drones abstraction that exposes
platforms as typed things the model calls — follows the same shape. The verification
step is the recurring lesson: OnFly separates goal generation from progress monitoring
and checks proposals before acting on them, and Chapter 13 will say why that matters
beyond the lab.

A second use is upstream of the policy entirely. Eureka of 2023 had a model write
reward functions from the environment's source code and iterate on them against
training curves; LaRes and its relatives evolve populations of rewards the same way;
StarEvolve of 2025 closes the loop in StarCraft II with a planner, an executor and a
verifier, and fine-tunes itself on its own good games. The model here is not the
commander but the *coach* — it reads what happened and rewrites the objective.

What the literature agrees a language model is good at: turning intent into
structure, choosing among named options, explaining choices, and reading logs. What it
is bad at: geometry, timing, anything at five hertz, and staying inside a schema
without a validator. The design implication is exact, and the sandbox has it: the
model emits `Command`s against a fixed vocabulary, the runtime validates and clamps,
and the replay records the command rather than the model, so a match commanded by an
unrepeatable process is still reproduced bit for bit.

**Reading.**
Ahn et al., *Do as I can, not as I say* (SayCan), 2022. Liang et al., *Code as
policies*, 2022. Kannan et al., *SMART-LLM*, 2023. Mandi et al., *RoCo*, 2023.
[*MRBTP: efficient multi-robot behavior tree planning and
collaboration*](https://arxiv.org/pdf/2502.18072), 2025.
[*LLM-HBT: dynamic behavior tree construction for heterogeneous
robots*](https://arxiv.org/pdf/2510.09963), 2025.
[*H-AIM: orchestrating LLMs, PDDL and behavior trees for hierarchical multi-robot
planning*](https://arxiv.org/html/2601.11063v1), 2026.
[*Say the mission, execute the swarm*](https://arxiv.org/html/2605.03788v1), 2026.
[*SwarmChat*](https://arxiv.org/html/2509.16920), 2025.
[Eureka](https://eureka-research.github.io/), 2023.
[*SC2Arena and StarEvolve*](https://arxiv.org/pdf/2508.10428), 2025.
[*Multi-agent systems powered by large language models: applications in swarm
intelligence*](https://arxiv.org/html/2503.03800v1), 2025.

## Chapter 13. Search, Rescue, And The Drones That Do Not Come Back

Rescue robotics has been measuring the price of partial information for thirty
years, and its findings are the ones this sandbox is built to reproduce.

**Search is over belief, not terrain.** Koopman's work of the 1940s, systematised in
Stone's *Theory of Optimal Search*, treats the target's location as a probability
distribution and a sensor as a function from effort spent in a region to probability
of detection. The optimal plan spends effort where the posterior is highest per unit
cost, and with an exponential detection function the plan that maximises detection is
also the one that most reduces the entropy of the posterior — searching well and
learning fast are the same act. Survivor search is the *multistate* case: the person is
alive and stationary, then drifting, then not, and the plan must weigh where they
probably are against how long they probably have. A frontier that is merely unvisited
is worth less than a frontier where someone probably is, and the difference is the
whole point of carrying a belief.

**Flying over does not find people.** Robin Murphy and Thomas Manzini of Texas A&M,
writing on why drones and vision models do not yet find flood victims quickly, name
three obstacles: victims are obscured, camouflaged, entangled in debris or submerged;
no training data exists for people in those postures; and oblique imagery mislocates
what it does find, so ground teams chase false leads. Their conclusion is not more
altitude but a division of labour: algorithms prioritise, humans verify, then someone
goes in. Going in is the part that costs. Caged drones built for confined-space
inspection — flying inside drains, tanks and culverts, bumping walls and continuing —
show that the last metre of search is a navigation problem in a space where
line-of-sight radio does not reach, which is what the sandbox's planar LIDAR is for.

**Robots that leave the network are the normal case.** DARPA's Subterranean
Challenge, run 2018 to 2021 in tunnels, urban undergrounds and caves, was rescue
search under exactly these conditions: no positioning signal, degraded sensing, denied
communications, one human supervisor. The winning team, CERBERUS, had legged carriers
ferry WiFi "breadcrumb" nodes into the course and drop them to extend a mesh behind
the explorers. The more instructive story is CSIRO Data61's: they began with a chain of
drop nodes forming a communication backbone, found that in-band relaying halved
bandwidth at every hop and that the mesh was unreliable in the event, and switched
strategy — instead of a pervasive network, more autonomy per robot, so that each could
operate beyond range and *return* to report. The robots that did not return were
expected; the system was designed so that their absence was informative and their
last reports had already been delivered.

**Losses are a routing constraint, not an accident.** The *Team Surviving Orienteers*
problem formalises it: a graph whose edges carry a probability of surviving the
traversal, a team of K robots, and the objective of maximising the expected number of
nodes collectively visited subject to each robot's probability of reaching its
destination staying above a threshold. A greedy algorithm comes within a provable
factor of optimal in time linear in the team size. In the scenario, that is the
question of whether to send the fourth drone into the culvert: expected survivors
found per expected drone lost, with a floor on how much loss the boat will accept.

**The pickup is the scarce thing.** Finding a survivor is not rescuing one. The boat
has capacity, speed and a clock, and the survivors have priorities and windows; the
literature calls this the capacitated team orienteering problem with time windows,
and it is the model behind boat-supported flood evacuation planning in which drones
scout ahead to locate victims, check routes and carry supplies. Marsupial teams — a
surface vessel that launches and recovers a small aircraft — exist in the field, and
the EMILY robotic lifebuoy is teamed with a drone precisely so the boat can be steered
to what the drone has seen.

**The simulation lineage.** RoboCup Rescue's simulation league has run this exact
class of experiment since 2001: ambulance, fire and police agents in a collapsed city,
civilians trapped in rubble who die if not reached in time and need several agents
acting together, and a server that caps both the number and the length of messages so
that agents cannot share what they perceive. Its stated design goal — effective
cooperation despite sensing and communication limitations — is this sandbox's
sentence in other words. The arena differs in being continuous, adversarial and
reproducible from a seed; the research questions are the same.

Three lessons carry over.

*Denial is the design condition.* A team should assume it is out of range and treat
contact as a windfall. CSIRO's reversal is the field's verdict, and UC-PSRO's
dropout-curriculum result in Chapter 10 is the same verdict from a simulator.

*Silence is evidence.* A drone that has not reported has either seen nothing or is
gone, and a teammate's policy should distinguish those. The last message before an
expected loss is the highest-value message that drone will ever send, and a doctrine
should say when to send it.

*Find, report, deliver are three problems.* Search theory owns the first, the
communication chapter owns the second, and orienteering under capacity owns the
third. A sandbox that scores only finding has left out the constraint that makes the
scenario hard, and a later objective should score delivery.

**Reading.**
Koopman, *Search and Screening*, 1946; Stone, *Theory of Optimal Search*, 1975.
[*Review of search theory: advances and applications to search and rescue decision
support*](https://www.researchgate.net/publication/235193662_Review_of_Search_Theory_Advances_and_Applications_to_Search_and_Rescue_Decision_Support).
[Murphy and Manzini, *Why drones and AI can't quickly find missing flood victims,
yet*](https://theconversation.com/why-drones-and-ai-cant-quickly-find-missing-flood-victims-yet-261035), 2025.
Murphy, *Disaster Robotics*, MIT Press 2014.
[Tranzatto et al., *Team CERBERUS wins the DARPA Subterranean Challenge: technical
overview and lessons learned*](https://arxiv.org/abs/2207.04914), 2022.
[Hudson et al., *Heterogeneous ground and air platforms, homogeneous sensing: Team
CSIRO Data61's approach*](https://arxiv.org/pdf/2104.09053), 2021.
[Saboia et al., *ACHORD: communication-aware multi-robot
coordination*](https://ai.jpl.nasa.gov/public/documents/papers/saboia-et-al-rss2022.pdf), 2022.
[Jorgensen et al., *The Team Surviving Orienteers
problem*](https://arxiv.org/abs/1612.03232), 2017.
[*UAVs as a tool for optimizing boat-supported flood evacuation
operations*](https://doi.org/10.3390/drones8110621), 2024.
[Kitano et al., *RoboCup Rescue: search and rescue in large-scale disasters as a domain
for autonomous agents
research*](https://www.academia.edu/41510858/RoboCup_Rescue_search_and_rescue_in_large_scale_disasters_as_a_domain_for_autonomous_agents_research), 1999.
[*Information-driven team collaboration in RoboCup
Rescue*](https://doi.org/10.3390/info17010008), 2026.

---

# Part VII — The Sandbox

## Chapter 14. Where The Sandbox Sits

Every part of the design has a shelf in the preceding chapters. Naming the shelf is
useful for two reasons: it tells a researcher which literature their experiment is
speaking to, and it tells a builder which known failure modes to test for.

| Sandbox component | Field | Shelf |
|---|---|---|
| Drives summed into `thrust` | Behaviour-based control, potential fields | Chapter 2, 3 |
| Leash, spacing | Flocking cohesion and separation | Chapter 2 |
| Stances, first match wins | Sequencing layer; fallback node of a behaviour tree | Chapter 3 |
| Doctrine library | Swarm tactics (OFFSET); plays (Playbook) | Chapter 4, 11 |
| `Role`, seven fixed values | Role-based allocation; ALLIANCE | Chapter 5 |
| `DesignateTarget`, target scorer | Assignment, target tracking | Chapter 5, 6 |
| `frontiers` in `WorldModel` | Frontier-based exploration | Chapter 6 |
| `Track` with growing uncertainty | Prediction step of a filter | Chapter 7 |
| `ingest_scan` vs `ingest_belief` | Rumour propagation, covariance intersection | Chapter 7 |
| `BeliefMsg` union, `encode(budget, interest)` | Bandwidth-limited fusion, goal-oriented comms | Chapter 8 |
| Comms radii, relay role | Algebraic connectivity, relay chains | Chapter 8 |
| Socket bridge, fixed `Observation` | Dec-POMDP environment for CTDE | Chapter 9 |
| Replay as inputs, event database | Payoff matrix for empirical game theory | Chapter 10 |
| Control centre, `Command`, `Origin::Human` | Mission command, delegation interface | Chapter 11 |
| Small language model at the centre | Deliberative layer, structured outputs | Chapter 12 |
| Range-limited comms, tanks out of contact | Autonomy beyond range and return | Chapter 13 |

**The scenario's names for things.** The arena's vocabulary and the rescue's, side by
side, so that a result read in one can be said in the other.

| Arena | Scenario |
|---|---|
| Shape found and farmed | Survivor located |
| Nest | Debris field where survivors concentrate |
| Control centre | The boat |
| Tank death | Drone unrecoverable |
| Sense radius | Low-altitude field of view |
| Comms radius | Line-of-sight radio over water |
| Opposing team | A second crew competing for the same scarce pickup |

Three things are absent by design, and the absence is the experiment.

**No global view for anyone.** The control centre cannot see. This is the difference
between the sandbox and most MARL benchmarks, where the centralised critic sees all
during training and nothing is stopping a deployed team from routing observations
through a server. The sandbox forbids it at every layer so that whatever coordination
appears was purchased with messages.

**No learned communication vocabulary.** `BeliefMsg` is fixed. Chapter 9's caveats
about emergent protocols are the reason: a fixed vocabulary is interpretable, lets
mixed teams talk, and makes "how many bytes of what kind" a countable quantity. The
`Raw` variant is the escape hatch, and its use is itself a measurement.

**No omniscient scoring of the team.** Score is the only public fact. Everything a
researcher wants to know about *how* the score was earned has to be reconstructed from
the event log, which is why the log records decisions and named stances rather than
positions.

## Chapter 15. Active Policy Creation

This chapter is speculation, offered because the user asked for it. Each idea names
what it is, why it is not already in the literature in this form, what it needs from
the sandbox, and what it would let someone measure. They are grouped by how far they
reach from what exists. None is a commitment.

### Near: the architecture already allows these

**1. The doctrine atlas.** Run MAP-Elites over the doctrine parameter space with
behavioural descriptors chosen from the event log — exploration coverage, bytes sent,
fire volume, mean distance from the control centre. The output is not a best doctrine
but a filled grid: the best doctrine at every combination of caution and talkativeness,
and the empty cells where no doctrine survives. Novel because quality diversity has
been run over neural and tree controllers but not over an *interpretable, named*
parameter space whose cells a human can read as tactics. Needs: the headless runner and
a batch driver. Measures: the shape of the viable-tactics region as comms radius
shrinks.

**2. Best-response ladders.** Seed a library with the shipped doctrines and run double
oracle: at each round, search doctrine space for the best response to the current
mixed strategy and add it. Because doctrines are readable, the ladder is a
*narrative* — aggressive beats passive, then screen-and-scout beats aggressive, then a
feint beats the screen — and the exploitability number of "aggressive" is a fact about
the game, not about a network. Needs: the batch driver from idea 1 and a payoff matrix
from the database. Measures: how many rounds until exploitability plateaus, and whether
the plateau moves with the comms model.

**3. Distillation from replays.** A replay is a stream of inputs. Given one, search for
the doctrine whose action stream best matches it, tick for tick. Two uses: explain a
learned or human-commanded team as the nearest readable doctrine, and compress a
Python policy into something the control centre can select by name. Novel as an
interpretability tool because most behaviour cloning targets a network; here the
target is a document. Needs: a distance between action streams and the search from
idea 1. Measures: how much of a learned policy's score a doctrine recovers — the
"readability tax".

**4. Counterfactual branching.** Because the replay is inputs, a match can be forked at
tick T with one team's doctrine swapped and re-simulated from that state. Credit
assignment for tactics becomes an experiment rather than an inference: *this* stance
change at *this* tick cost the match, and here is the branch where it did not. Needs:
world snapshot and restore at a tick, which a deterministic simulation gets nearly for
free. Measures: the sensitivity of the outcome to each decision — a map of the match's
turning points.

**5. Automatic after-action reports.** Named stances and typed events were chosen so
that a match is a sequence of legible facts. A language model given the event table
and a fixed template writes the after-action report armies have written by hand for a
century: what was ordered, what each unit did, where the orders never arrived. Cheap,
and useful immediately for reading a thousand-match sweep. Needs: nothing new. Measures:
whether the report's stated causes survive idea 4's counterfactuals.

### Middle: one new mechanism each

**6. Doctrine drift under partition.** Give every doctrine a version, and have the
control centre's doctrine changes propagate only over the comms model, so that a tank
out of range keeps the old orders. The map now has *two* doctrines on it, and the
boundary between them is the network partition made visible. Measure the team's
*coherence* — the fraction of tanks on the current version, weighted by time — and
correlate it with score. Novel because doctrine is normally global in simulators;
making it a message turns mission command's central problem into a scalar. Needs: a
version field, delivery through the comms model when it lands, an overlay in the
viewer.

**7. Bottom-up proposals.** Mission command runs in both directions: subordinates
report and *propose*. Let a tank emit an intent — `BeliefMsg::Intent` already exists —
that the control centre may ratify into a command for the team or override. A doctrine
then has a policy for proposing as well as for acting, and the centre has a policy for
listening. Measures: how often the centre's decision was a tank's idea, and whether
teams with proposal channels beat teams without them under the same bandwidth.

**8. Role auctions over lossy links.** Replace static role composition with CBBA run
over `BeliefMsg`: tanks bid for roles by local cost, consensus resolves the winners,
and the doctrine sets the *bidding rules* rather than the roles. Under a perfect link
this reproduces the static assignment; under partition it produces two tanks that both
think they are the scout, which is the correct and measurable outcome. Needs: a bid
payload and a consensus round in the decision loop. Measures: allocation quality
against bandwidth — the curve CBBA papers draw, on a game where allocation has a score.

**9. Stigmergic fields.** Add two decaying scalar fields to the world model — danger,
where teammates took damage, and richness, where shapes were seen — that propagate only
by belief messages and that drives can read. This is Buzz's virtual stigmergy over a
lossy link. A tank routing around a danger field it has never itself seen is acting on
hearsay, which is what the belief chapter says to be careful about, and a field version
of rumour propagation is a thing worth watching form. Needs: two grids in the model and
a `MapPatch` for them. Measures: how far a mark spreads before it decays, as a function
of comms radius.

**10. One-word orders.** Impose the byte budget on commands, not only on beliefs. A
control centre that can send eight bytes a tick must choose *which* knob to turn; a
doctrine patch becomes a codebook entry. This is radio discipline as a design
constraint, and it makes the question "what does a commander choose to say when they
can say one thing" answerable from the log. Needs: budgeted `encode` for commands.
Measures: the distribution of commands sent under each budget.

### Far: the interesting speculations

**11. The model reads the database.** Close the loop between matches. A language model
is given read access to the match database — the event table, the score table, the
stance histogram — and asked to edit the doctrine; the edit is validated, the rematch
runs, the model reads again. This is Eureka with a doctrine instead of a reward and a
SQL table instead of a training curve, and the database schema was designed to be
queried, which makes it a sensor. The interesting variable is *what the model is
allowed to see*: only score, or the whole log. Needs: nothing the design does not
already provide; the loop is a script. Measures: doctrine improvement per rematch
against the same for MAP-Elites — a coach against a breeder.

**12. Doctrine inference of the enemy.** A scout's product is not only a map. Given the
enemy's observed behaviour — where they went, when they fired, how they moved together
— classify which doctrine from the library they are most likely running, then have the
control centre select the best response from idea 2's ladder. The match now contains a
meta-game played *inside* it, and the scout's value is the information gain about the
enemy's doctrine per unit of risk. Needs: the classifier from idea 3 turned outward.
Measures: time-to-identify against scouting doctrine, and whether identifying the
enemy is worth the tank it costs.

**13. Feints and deception.** Once firing is priced — that is, once sensing is a sweep
and comms carry position — a doctrine can include behaviour whose purpose is to be
misread: fire to draw a screen off the nest, broadcast a false intent, move in a
pattern idea 12 will misclassify. Its place is tactical oversight: a team that can be
misled must reconcile conflicting evidence, which is Chapter 7's problem made
adversarial, and the pressure that puts on the other team's coordination is the reason
to keep it. Needs: v0.5 sensing and the comms model. Measures: whether a team running deception beats a team running the
classifier, which is idea 12 against its own countermeasure.

**14. Threshold bandits.** Keep everything about a doctrine fixed except the numbers in
its stance guards — the health fraction at which retreat fires, the enemy distance at
which engage fires — and learn only those, as a contextual bandit per stance with the
event log as reward. Interpretability is preserved because the structure never moves;
only the thresholds tune. Novel as a deliberately tiny learning problem embedded in a
readable policy. Needs: a bandit and a reward signal from scores. Measures: how much of
the gap between the doctrine and a learned policy the thresholds alone close.

**15. Mixed-model teams and rumour.** The design already wants this and no one has run
it: one team with tanks running two world models, one that fuses naively and one that
uses covariance intersection, both talking through the same `BeliefMsg`. Chapter 7
predicts the naive tanks will grow confident about things the careful tanks doubt, and
that the confident ones will die first. Needs: the second world model. Measures:
confidence-versus-accuracy per tank over the match, which is the calibration curve of a
belief.

**16. Human play as training data.** `Origin::Human` is recorded on every command. A
tournament of humans at the control centre produces a corpus of play-calling against
known doctrines; idea 3 distils each human into a doctrine, and idea 11's model is
fine-tuned on the corpus. The question this makes measurable is old and unanswered:
what do human commanders do that the ladder in idea 2 did not find?

### From the scenario

**17. The last message.** A doctrine stance guarded on imminent loss — health below a
floor with an enemy adjacent, or a region flagged as no-return — whose only drive is
to emit: position, what was seen, where it was going. Teammates treat silence after an
expected report as evidence of loss and re-plan. Needs: the comms model. Measures:
information recovered per drone lost.

**18. Deliver, not destroy.** An `objective` variant where a located survivor scores
only when a capacity-limited carrier reaches it within a window. Points move from the
finder to the team, and the scarce resource is the carrier's route — capacitated team
orienteering inside the arena. Needs: the objective crate and a carrier entity.
Measures: survivors delivered against survivors found; the gap is the boat.

**19. Confined regions.** Regions where sense and comms radii shrink to a fraction:
culverts. Entering one buys information at the price of contact. Whether a doctrine
sends a drone in, and which one, is a survival-orienteering decision the doctrine
should be able to state. Needs: terrain in the arena spec. Measures: survivors found in
confined regions per drone lost there.

**20. Belief-weighted frontiers.** `frontiers` ordered by a prior over where
survivors probably are, not by staleness alone — Koopman's allocation as a drive.
Needs: a prior field in the world model, seeded from the spawn configuration the way a
rescuer's prior is seeded from last known addresses. Measures: time-to-find against
uniform frontier search.

**21. Survivability as a field.** A risk field over the map, learned from where
teammates were lost, that every drive multiplies against — the Team Surviving
Orienteers constraint as a potential. Needs: idea 9's field machinery. Measures: loss
rate against exploration rate as the field's weight moves.

### A note on what makes these possible

Every idea above leans on four properties the design chose early: the replay is a
stream of decisions, the event log records named facts, the policy vocabulary is fixed
and readable, and nobody sees the whole map. None of them was chosen for these ideas.
They were chosen so that results would be reproducible and comparable, and the ideas
are what reproducible, comparable results permit.

---

## Afterword

The sentence this book opened with has been true of every team that ever operated —
ants, armies, robots — and each field that met it built a way to live with it rather
than a way around it. Flocks live with it by needing only neighbours. Auctions live
with it by making bids carry the private information. Fusion lives with it by refusing
to be more certain than the evidence. Mission command lives with it by sending intent
once and trusting. Learning lives with it by training in silence until silence is
normal.

The sandbox is a place to set those answers against each other on the same map with
the same clock, and the doctrine layer is the dial that selects among them. What a
researcher does with the dial is the research. Most of those answers were found by
people looking for other people, and the sandbox is a place to test them before the
water rises.

---

## Consolidated Reading List

Ordered by chapter. Links are to the pages consulted while writing; classic references
without links are easily found by title.

- Bernstein et al., 2002 · Oliehoek and Amato, 2016 · [Gerkey and Matarić, 2004](https://journals.sagepub.com/doi/10.1177/0278364904045564)
- Reynolds, 1987 · Vicsek, 1995 · Couzin, 2002 · Khatib, 1986 · [Ren and Beard, 2005](http://www.et.byu.edu/~beard/papers/reprints/RenBeard05a.pdf) · Olfati-Saber, 2006 · [Oh, Park, Ahn, 2015](https://www.sciencedirect.com/science/article/abs/pii/S0005109814004038)
- Brooks, 1986 · Arkin, 1998 · Gat, 1998 · [Colledanchise and Ögren, 2018](https://arxiv.org/abs/1709.00084) · [Ghzouli et al., 2020](https://www.cse.chalmers.se/~bergert/paper/2020-sle-behaviortrees.pdf) · [Formalisms survey, 2026](https://arxiv.org/html/2603.15427v2) · [Pinciroli and Beltrame, 2016](https://arxiv.org/abs/1507.05946)
- Rubenstein et al., 2014 · [DARPA OFFSET](https://www.darpa.mil/research/programs/offensive-swarm-enabled-tactics) · [OFFSET FX-6](https://www.darpa.mil/news/2021/offset-swarms-take-flight) · [OFFSET congestion](https://arxiv.org/pdf/2307.16788)
- Smith, 1980 · [Choi, Brunet, How, 2009](https://dl.acm.org/doi/10.1109/tro.2009.2022423) · Parker, 1998 · [Market MRTA survey, 2022](https://link.springer.com/article/10.1007/s10846-022-01803-0) · [MRTA systematic review, 2024](https://dl.acm.org/doi/10.1145/3700591)
- Sharon et al., 2015 · van den Berg et al., 2008, 2011 · Yamauchi, 1997, 1998 · Cortés et al., 2004 · [Robin and Lacroix, 2016](https://link.springer.com/article/10.1007/s10514-015-9491-7)
- Julier and Uhlmann, 1997 · [Inverse CI](https://www.researchgate.net/publication/314194756_Decentralized_data_fusion_with_inverse_covariance_intersection) · [Dagan and Ahmed, 2023](https://arxiv.org/pdf/2307.10594)
- [Roth, Simmons, Veloso, 2005](https://link.springer.com/chapter/10.1007/1-4020-3389-3_8) · Zavlanos and Pappas, 2008 · [Connectivity under uncertainty](https://arxiv.org/abs/2012.09808) · [Swarm relays](https://arxiv.org/pdf/1909.10496) · [Intermittent rendezvous, 2023](https://arxiv.org/abs/2309.13494) · [Goal-oriented comms, 2025](https://arxiv.org/pdf/2508.07720)
- Rashid et al., 2018 · Lowe et al., 2017 · Yu et al., 2022 · [SMACv2](https://arxiv.org/pdf/2212.07489) · Foerster et al., 2016 · Sukhbaatar et al., 2016 · [Das et al., 2019](https://arxiv.org/abs/1810.11187) · [Engineered vs emergent, 2025](https://www.arxiv.org/pdf/2508.02912)
- Nolfi and Floreano, 2000 · Mouret and Clune, 2015 · [QD behaviour trees for swarms, 2023](https://link.springer.com/chapter/10.1007/978-3-031-30229-9_10) · Lanctot et al., 2017 · [PSRO survey, 2024](https://www.ijcai.org/proceedings/2024/0880.pdf) · [UC-PSRO, 2026](https://arxiv.org/html/2608.15372)
- [Miller and Parasuraman, 2007](https://journals.sagepub.com/doi/10.1518/001872007779598037) · [Walker et al., 2013](https://www.ri.cmu.edu/pub_files/2014/9/walkerHFES2013loa-final-CR.pdf) · [Delegation in human-machine teaming, 2021](https://link.springer.com/chapter/10.1007/978-3-030-68017-6_2)
- Ahn et al., 2022 · Liang et al., 2022 · Kannan et al., 2023 · Mandi et al., 2023 · [MRBTP, 2025](https://arxiv.org/pdf/2502.18072) · [LLM-HBT, 2025](https://arxiv.org/pdf/2510.09963) · [H-AIM, 2026](https://arxiv.org/html/2601.11063v1) · [Web-of-Drones, 2026](https://arxiv.org/html/2605.03788v1) · [SwarmChat, 2025](https://arxiv.org/html/2509.16920) · [Eureka, 2023](https://eureka-research.github.io/) · [StarEvolve, 2025](https://arxiv.org/pdf/2508.10428) · [LLM swarm survey, 2025](https://arxiv.org/html/2503.03800v1)
- Koopman, 1946 · Stone, 1975 · [Search theory for SAR review](https://www.researchgate.net/publication/235193662_Review_of_Search_Theory_Advances_and_Applications_to_Search_and_Rescue_Decision_Support) · [Murphy and Manzini, 2025](https://theconversation.com/why-drones-and-ai-cant-quickly-find-missing-flood-victims-yet-261035) · Murphy, 2014 · [CERBERUS lessons, 2022](https://arxiv.org/abs/2207.04914) · [CSIRO Data61, 2021](https://arxiv.org/pdf/2104.09053) · [ACHORD, 2022](https://ai.jpl.nasa.gov/public/documents/papers/saboia-et-al-rss2022.pdf) · [Team Surviving Orienteers, 2017](https://arxiv.org/abs/1612.03232) · [Boat-supported flood evacuation, 2024](https://doi.org/10.3390/drones8110621) · [RoboCup Rescue, 1999](https://www.academia.edu/41510858/RoboCup_Rescue_search_and_rescue_in_large_scale_disasters_as_a_domain_for_autonomous_agents_research) · [RoboCup Rescue information-driven, 2026](https://doi.org/10.3390/info17010008)
