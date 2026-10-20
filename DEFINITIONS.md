# Worldwright — MUD and MU* Definitions

**Understanding the traditions without confusing names, architectures, or compatibility.**

| Field | Value |
|---|---|
| Document version | 0.1 |
| Updated | October 20, 2026 |
| Purpose | Explain MUD, MUSH, MUCK, MUX, MOO, MU*, and related development vocabulary |
| Worldwright status | Proposed Rust toolkit; no legacy-engine compatibility has been established |
| Related documents | [README](README.md), [Design](DESIGN.md), [Architecture](ARCHITECTURE.md), [Contributing](CONTRIBUTING.md) |

> **The central distinction:** These names mix historical lineages, programming models, community traditions, and gameplay styles. They are not mutually exclusive categories, formal interoperability standards, or a ranking from primitive to advanced.
>
> Historical descriptions below are explanatory, not recommendations to reproduce old authentication, shared-administrator-account, plaintext transport, or retention practices. Check the particular engine and release before relying on behavior or license terms.

## Contents

- [Quick Comparison](#quick-comparison)
- [Four Different Questions](#four-different-questions)
- [MUD](#mud)
- [MU Star](#mu-star)
- [MUSH](#mush)
- [MUCK](#muck)
- [MUX](#mux)
- [MOO](#moo)
- [Other Important Families](#other-important-families)
- [One Scenario Across the Traditions](#one-scenario-across-the-traditions)
- [World and Programming Vocabulary](#world-and-programming-vocabulary)
- [Player and Community Vocabulary](#player-and-community-vocabulary)
- [Client and Protocol Vocabulary](#client-and-protocol-vocabulary)
- [Worldwright Terminology and Scope](#worldwright-terminology-and-scope)
- [Compatibility and Migration](#compatibility-and-migration)
- [Choosing a Direction](#choosing-a-direction)
- [Common Misconceptions](#common-misconceptions)
- [Reference Notes](#reference-notes)

## Quick Comparison

The table describes common associations, not requirements imposed on every world.

| Term | Common expansion or meaning | Typical emphasis | Distinguishing technical association |
|---|---|---|---|
| **MUD** | Historically Multi-User Dungeon; also Multi-User Domain or Dimension | Broadly, shared text worlds; in narrower usage, automated adventure/RPG play | No single required language, engine, database, or protocol |
| **MU*** | Wildcard umbrella for the family of multi-user text worlds | Inclusive reference across traditions | A category label, not an engine or acronym with one full expansion |
| **MUSH** | Commonly Multi-User Shared Hallucination; expansions vary | Collaborative roleplay, social interaction, building | TinyMUD-derived MUSH-family engines and attribute-based softcode; broader cultural use also exists |
| **MUCK** | Name derived playfully from MUD; expansions such as Multi-User Created Kingdom are backronyms | User-built environments and programmable social/roleplay worlds | TinyMUCK family; Fuzzball's MUF and MPI are important examples |
| **MUX** | Often glossed as Multi-User eXperience; not a technical standard | MUSH-like roleplay and world building | Usually refers to TinyMUX-based worlds in MU* discussions |
| **MOO** | Historically MUD, Object Oriented | Programmable object worlds, social collaboration, education, or games | Objects with properties, verbs, inheritance, and the MOO language |

A fantasy MUSH and a science-fiction MUSH can have more technical infrastructure in common than two fantasy MUDs. Conversely, a modern engine can support a MUSH-like play style without implementing a historical MUSH language or database.

## Four Different Questions

When someone describes a project as a “MUD” or “MUSH,” ask which layer they mean:

| Layer | Question | Example |
|---|---|---|
| Experience | What do participants do? | Explore, fight, converse, build, solve puzzles, roleplay |
| Community model | How do participants organize play? | Freeform scenes, staff-run plots, persistent campaigns, casual social space |
| Implementation | What software and language execute the world? | PennMUSH, TinyMUX, Fuzzball, LambdaMOO, an LPC driver/mudlib, or a custom Rust server |
| Compatibility | What can another system actually understand? | Network frames, commands, scripts, database records, or imported content |

A shared answer at one layer does not establish agreement at the others. Two games can accept the command `look` but have incompatible object identities, permissions, scripting, persistence, and networking.

## MUD

### Meaning and history

**Multi-User Dungeon** is the historical expansion. **Multi-User Domain** and **Multi-User Dimension** are also used, especially when describing spaces that are not focused on dungeon exploration.

Worldwright uses **multi-user domain** as its preferred broad expansion because the toolkit is intended for many settings and interaction styles. That choice does not revise the historical name of the original MUD.

The original MUD was created by Roy Trubshaw and Richard Bartle at the University of Essex, with early work beginning in 1978. Bartle's historical account distinguishes the first experiments, subsequent rewrites, and the development of the gameplay; treating all of that as one finished 1978 product obscures the history. See [Bartle's account](https://mud.co.uk/richard/mudhist.htm).

### Broad usage

In its broad sense, a MUD is a multi-user interactive world, traditionally accessed mainly through text. Common elements include:

- Multiple participants sharing a world model.
- Persistent identities or state, though persistence policies vary.
- Commands and descriptive output.
- Places, entities, and relationships among them.
- Communication and interaction with other participants.

Not every world has all the same object types, persistence guarantees, or simulation rules. Some emphasize social presence rather than a conventional game objective.

### Narrower gameplay usage

Players sometimes use “MUD” more narrowly for a game with automated systems such as:

- Combat rounds, health, skills, classes, or levels.
- Equipment, loot, currency, and progression.
- NPC encounters, quests, travel, and resource gathering.
- PvE and, where allowed, PvP.

This narrower usage helps distinguish an automated RPG from a primarily scene-driven roleplay community. It is a community convention, not a universal definition. A game can combine automated combat with extensive collaborative writing.

### What the name does not tell you

“MUD” does not establish fantasy genre, a Telnet requirement, a C implementation, free access, open-source licensing, real-time combat, or compatibility with another game's client extensions.

For Worldwright, the initial reference slice fits comfortably within the broad term: a browser-accessible shared world with authoritative commands, rooms, communication, and durable gameplay.

## MU Star

**MU***, often read aloud as “M-U-star,” is an umbrella notation. The asterisk is a wildcard standing in for the changing endings of names such as MUD, MUSH, MUCK, MUX, and MOO. Related historical shorthand includes M* or M***.

Use it when the point applies across traditions:

- “MU* accessibility and client support.”
- “Permissions in programmable MU* worlds.”
- “A survey of MU* content-authoring practices.”

It is not:

- A specific server product.
- A protocol all listed systems implement.
- A claim that every branch has the same source-code ancestry.
- A promise that a single importer or scripting runtime can handle them all.

Worldwright can describe its design influences as **MU*-informed**. It should not claim to be a universal MU* compatibility layer without evidence for named targets and versions.

## MUSH

### Meaning

MUSH is most commonly expanded as **Multi-User Shared Hallucination**, with other historical expansions encountered. Avoid presenting one expansion as a standards-body definition.

The traditional MUSH family developed from TinyMUD. PennMUSH's own introduction describes the social/roleplaying orientation, building capabilities, and internal MUSHcode language. See the [PennMUSH introduction](https://github.com/pennmush/pennmush/wiki).

### Typical player experience

A MUSH often emphasizes:

- Writing scenes with other participants.
- Character concepts, relationships, and a shared setting.
- Local poses/emotes, out-of-character coordination, and channels.
- Staff-facilitated plots or player-organized events.
- Character approval, sheets, dice, or other rule support when the game chooses them.

Some MUSHes implement substantial combat and economy systems. Others rely more heavily on negotiated outcomes or staff adjudication. “Roleplay-oriented” does not mean “has no rules” or “contains only chat.”

### Technical model

Classic MUSH-family engines commonly expose player, room, thing, and exit objects, with attributes and permissions. Behavior can be implemented through expressions, functions, queued actions, and user-defined commands attached to objects or shared command contexts.

This in-world programming is often called **softcode** or **MUSHcode**, contrasted with changes to the server's compiled implementation, or **hardcode**.

Examples associated with the family include PennMUSH, TinyMUSH, and RhostMUSH. TinyMUX is closely related in ecosystem and play style, discussed below. Exact syntax, evaluation behavior, permissions, and database formats vary by engine and version.

### Implications for Worldwright

A MUSH-like experience suggests features such as scene tracking, character applications, configurable command sets, local emotes, private coordination, and moderator tooling.

Those features do not require Worldwright to execute MUSHcode. Its proposed initial extension model is trusted Rust game modules and validated content. Advertising MUSHcode execution would require a separately designed interpreter/compatibility layer, evaluation limits, permission model, and tests.

## MUCK

### Meaning

MUCK is a playful name related to MUD. Expansions such as **Multi-User Created Kingdom**, **Multi-User Chat Kingdom**, and **Multi-User Construction Kit** are encountered, but should be labeled backronyms or community glosses—not a single settled original expansion. See the [TinyMUCK historical overview](https://en.wikipedia.org/wiki/TinyMUCK) for the naming discussion and its references.

### Typical player experience

MUCKs are commonly associated with:

- Social interaction and roleplay.
- Player-created rooms, objects, descriptions, and utilities.
- Persistent personal environments and customizable avatars.
- A world that participants can extend rather than merely explore.

Specific communities have influenced MUCK culture, but the software category is not defined by a particular fandom, genre, content rating, or adult theme. Never infer a site's audience or conduct rules from “MUCK” alone.

### Technical model

TinyMUCK-derived systems add programmable behavior to a shared object world. In the Fuzzball tradition, two important languages are:

| Term | Meaning and typical role |
|---|---|
| **MUF** | MUCK Forth; a stack-based programming language used for programs and more substantial utilities |
| **MPI** | A message-oriented expression facility used for descriptions and other dynamic text or smaller behaviors |

The Fuzzball manual describes MUF programming as requiring the relevant privilege, while MPI is available more broadly with its own restrictions. This is a description of that documented model, not proof that every deployed MUCK has identical permissions. See [Fuzzball's programming guide](https://fuzzball-muck.github.io/muckman/programming.html).

MUCK object categories commonly include players, rooms, things, exits/actions, and programs. Exits/actions can have behavior beyond a simple door between two locations. Properties, object control, flags, and program privilege levels are central to understanding a particular implementation.

### Implications for Worldwright

A MUCK-like authoring experience would need safe user-owned objects, scoped building permissions, quotas, programmable interactions, and privacy-aware control of puppets or delegated entities.

Allowing a builder to author content is not the same as allowing arbitrary executable code. MUF or MPI compatibility is not part of Worldwright's baseline and would require an explicit project with migration, isolation, and permission semantics.

## MUX

### Meaning

Within MU* discussions, MUX usually refers to **TinyMUX** or a world running it. **Multi-User eXperience** is a common gloss, but the engine name and ecosystem relationship are more useful than treating the expansion as a separate architectural definition.

TinyMUX's own FAQ describes it as a server for creating and hosting MUSHes. That is the key distinction: **MUX is not necessarily a separate gameplay category from MUSH**. See the [TinyMUX FAQ](https://www.tinymux.org/faq/).

### Typical player experience

Many MUX worlds share MUSH conventions: collaborative roleplay, character sheets, scenes, channels, social spaces, and setting-specific systems. A player may notice more variation between two communities running the same engine than between a MUX and a MUSH using different engines.

### Technical model

TinyMUX has its own implementation, releases, expression behavior, object/attribute conventions, configuration, and extension features. Similar-looking softcode is not proof of semantic equivalence with PennMUSH or TinyMUSH.

Do not generalize a feature listed for one TinyMUX release to all MUX worlds. This guide does not claim a particular release is production-ready, compatible, or currently deployed at any site.

### Implications for Worldwright

A future TinyMUX importer would need a specific target version and migration contract. Names, descriptions, and exit links are only the beginning; attributes, locks, code, object ownership, queued actions, and visibility semantics require separate treatment.

“MUX” in this context also should not be confused with an operating-system terminal multiplexer, a network multiplexer, or an audio/video muxer.

## MOO

### Meaning

The historical expansion is **MUD, Object Oriented**. “Multi-User Object Oriented” is also encountered as a gloss, but the [LambdaMOO programmer's manual](https://lambda.moo.mud.org/pub/MOO/ProgrammersManual.txt) explicitly uses the former.

MOO can refer to the programming-language tradition, a server implementation, or a world/community using it. Distinguish **LambdaMOO the server software**, **LambdaCore or another starting database**, and **a particular running world**.

### Typical experience

MOOs have been used for social worlds, collaborative projects, education, experimentation, and games. Their significance is not just a preference for conversation: they make programmable objects a central organizing concept.

Participants with appropriate permissions may create and modify objects and their behavior inside the shared world. The extent of those privileges is a community/server policy, not an automatic right for every guest.

### Technical model

In the LambdaMOO model:

- **Objects** represent entities in the database.
- **Properties** hold object data.
- **Verbs** are programs associated with objects.
- **Inheritance** allows objects to reuse properties and behavior.
- **Ownership and permissions** determine who can inspect, change, and execute relevant behavior.

A verb can implement a player command or serve as an internal callable method. The manual explains that verb ownership influences its execution permissions, making privileged verbs especially sensitive. See [Verbs on Objects](https://lambda.moo.mud.org/pub/MOO/html/ProgrammersManual_7.html).

The server supplies language execution, connections, task handling, and database access. The world database contains not just descriptions but behavior. The manual deliberately distinguishes server behavior from conventions of a particular core database; that separation matters when discussing compatibility. See the [manual introduction](https://lambda.moo.mud.org/pub/MOO/html/ProgrammersManual_1.html).

### Implications for Worldwright

A Rust server with structs or traits is **not automatically a MOO**. MOO-like programming involves a persistent object model, object-attached behavior, inheritance/lookup semantics, ownership, and constrained in-world execution—not simply use of an object-oriented design technique.

Worldwright's stable entities and game SDK can support some comparable experiences, but the baseline does not provide a MOO interpreter, compatible verb dispatch, or import of a MOO database/core.

## Other Important Families

### TinyMUD

TinyMUD is an important historical server and social/building tradition. Its influence and derivatives help explain the MUSH and MUCK families, but not every later text-world engine is a direct source-code fork of it.

A conceptual influence diagram is not a source genealogy. Avoid drawing inheritance arrows unless the specific historical relationship has been checked.

### LPMud, LPC, driver, and mudlib

The LPMud tradition is associated with the **driver/mudlib split**:

- The **driver** implements low-level execution and runtime mechanisms.
- **LPC** is the programming language used for game objects and behavior.
- The **mudlib** supplies game-side foundations and rules on top of the driver.

FluffOS documentation illustrates this distinction: a driver alone is not the finished game. See [FluffOS driver concepts](https://www.fluffos.info/concepts/general/MudOSdriver.html).

LPMud does not prescribe one combat system, community style, or license for all mudlibs. Worldwright's engine/game separation is analogous at a high level, but Worldwright does not currently execute LPC or load an LPC mudlib.

### Diku-family terminology

DikuMUD and its derivatives are strongly associated with room-based RPG systems, mobiles/NPCs, equipment, zones, and resets. Derivative engines differ substantially; “Diku-like” is not an exact schema.

Names such as CircleMUD, Merc, ROM, and SMAUG are engine/family identifiers, not interchangeable file formats. Read a specific engine's manuals, source, and license before describing an importer or reusing its content. This guide does not assert a universal license or complete genealogy for that family.

### Modern frameworks and hybrid worlds

A modern framework may use conventional application languages, relational storage, and browser clients while supporting MUD-like or MUSH-like experiences. Its label may describe gameplay intent rather than historical source ancestry.

Worldwright belongs in this design space: a proposed Rust toolkit influenced by MU* practices, not a port of an identified legacy engine.

## One Scenario Across the Traditions

Consider a sealed hatch in an abandoned orbital station. These are illustrative design approaches, not commands promised by any named engine.

| Style/model | How the hatch might work | Main implementation concern |
|---|---|---|
| Automated adventure MUD | Check a character's access card, skill, cooldown, and inventory; open the passage when rules allow | Authoritative state transitions and durable item/quest effects |
| MUSH-like roleplay | Participants pose an investigation, make supported rolls, and agree or receive adjudication on the outcome | Scene coordination, visibility, permissions, and rules support |
| MUCK/Fuzzball-style extension | A room/action and MUF or MPI behavior implement access and descriptive effects | Object control, program privileges, quotas, and safe customization |
| TinyMUX-based world | Object attributes and softcode implement commands and narrative or automated checks | Exact engine expression/lock/evaluation semantics |
| MOO-style object programming | A hatch object exposes a verb and properties, possibly inheriting behavior from a generic hatch object | Method lookup, ownership, permissions, and object persistence |
| Worldwright baseline | Validated hatch/room definitions feed trusted Rust game rules; a typed command creates reviewed effects | Engine/game separation, authorization, commit/retry semantics, and testability |

Any of these can be science fiction or fantasy. Replace the hatch with an enchanted gate and the architecture questions remain largely the same.

## World and Programming Vocabulary

| Term | Meaning and cautions |
|---|---|
| **World** | A shared environment or deployment; not necessarily an entire server process in every engine |
| **Room** | A navigable location; can represent a forest, spaceship, conceptual space, or tutorial page rather than an indoor room |
| **Exit** | A connection/action associated with navigation; its exact object/command behavior varies by engine |
| **Area / zone** | A content, administrative, scheduling, or reset grouping; these meanings are not universally equivalent |
| **Object** | An entity in the world's model; may mean anything from a stored record to a programmable persistent object |
| **Thing / item** | Often a manipulable object, but engine object-type rules differ |
| **Mobile / mob** | Traditionally a mobile entity, commonly an NPC; not a reference to a mobile phone |
| **NPC** | Non-player character; may be automated, staff-controlled, or controlled by a system-specific mechanism |
| **PC** | Player character; separate from the human and account in Worldwright's terminology |
| **Definition / prototype** | Reusable description of an entity type or content object |
| **Instance** | A particular live entity or environment created from definitions |
| **Instantiation / spawn** | Creation of a live entity; repeated creation must respect game rules and persistence identity |
| **Reset / respawn** | Replenishment or restoration behavior; not always a full database rollback or server restart |
| **dbref** | Database reference, commonly displayed as `#123` in several traditions; scope, reuse, and lifetime vary |
| **vnum** | Virtual number used by some engines for prototype/content identity; not automatically a live-instance ID |
| **Attribute / property** | Stored object data; the two words have engine-specific meanings and are not always substitutes |
| **Verb** | A callable object-associated program in MOO; in general parser discussion, the action word in a command |
| **Softcode** | Behavior expressed in an engine's in-world language or data-driven programming facilities |
| **Hardcode** | The server implementation or changes requiring its normal build/release process |
| **MUF** | MUCK Forth, a stack-based language associated with TinyMUCK/Fuzzball |
| **MPI** | MUCK message-oriented programming/expression facility; not the HPC Message Passing Interface |
| **MUSHcode** | Collective term for MUSH-family softcode; dialect and semantics vary |
| **LPC** | Language associated with LPMud drivers and mudlibs; C-like syntax does not make it ordinary C |
| **Mudlib / core database** | Game-side foundations in particular ecosystems; not universally interchangeable terms |
| **Lock** | Often an engine permission predicate for using, entering, or controlling an object; not necessarily a thread mutex or database lock |
| **Flag / bit** | An engine-defined state or privilege marker on an object |
| **Queue / task** | Deferred execution mechanism; permissions, quotas, cancellation, and ordering vary |
| **Tick / round / pulse** | A scheduling/game-time unit, not necessarily equal to network latency or a database commit |
| **Dump / checkpoint** | Persisted state capture; a file existing does not prove crash consistency, off-site backup, or restore success |
| **OLC** | Online creation/editing of content while an engine runs; permission and publication behavior vary |
| **Quota** | A bound on resources such as objects, storage, execution, or queued work |
| **Sandbox** | An enforced isolation/permission boundary, not merely an API convention or a memory-safe implementation language |

Names in this table are conceptual aids. Migration requires checking actual representation, scope, permission semantics, and lifecycle.

## Player and Community Vocabulary

### Identity and roles

| Term | Meaning |
|---|---|
| **Player / participant** | The human using the service; older documentation sometimes also calls the character object a player |
| **Account** | Authentication identity; may own several characters depending on the game |
| **Character** | A represented persona or controllable entity |
| **Alt** | An alternate character, often belonging to the same person; games set their own disclosure/use rules |
| **Session** | An authenticated context; distinct from an individual network connection |
| **Puppet** | An entity controlled through another character or interface, often relaying observations to its controller |
| **Bot** | Automated participant/client or programmed entity; permission to automate depends on the service |
| **Builder** | Someone authorized to create content within a scope |
| **Wizard / immortal / god / admin** | Historical privilege or staff labels; meanings vary and may differ from fictional character titles |
| **Moderator** | Someone responsible for conduct processes; does not inherently need database or secret access |

Worldwright should model human responsibilities as explicit capabilities rather than copy a universal all-powerful “wizard” account. Never use historical shared-password practices as a modern operational recommendation.

### Play and social conventions

- **IC — in character:** Acting or communicating as the fictional character.
- **OOC — out of character:** Communicating as the participant or coordinating the activity.
- **RP — roleplay:** Portraying a character in the setting; the amount of automation varies.
- **Pose / emote:** A textual action or narrative contribution, often including the character's name.
- **Scene:** A bounded interaction among characters; can be synchronous or supported asynchronously by tools.
- **Plot:** A larger narrative sequence, potentially linking several scenes.
- **GM / storyteller:** A person facilitating or adjudicating play, under the game's rules.
- **PvE:** Player interaction against environment or system-controlled challenges.
- **PvP / PK:** Player-versus-player conflict / player killing, with scope and consent defined by each game.
- **RPI / RP-enforced:** Community labels for stronger roleplay expectations; not technical standards.
- **Metagaming:** Using information or incentives outside the character's legitimate knowledge in ways the game disallows.
- **Powergaming / godmodding:** Context-dependent labels for imposed outcomes or excessive control over others' characters; define the prohibited behavior rather than relying on the label alone.
- **Consent:** Agreement to participate in a scene or interaction within stated boundaries; distinct from a database permission or software license.
- **Channel / page / tell / whisper:** Communication mechanisms with different scopes; confirm audience and retention before assuming privacy.
- **AFK / idle:** Away from keyboard or connected without active input; not proof that a person is available or consenting.

IC conflict is not permission for OOC harassment. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for official Worldwright activities; independent games need their own policies.

## Client and Protocol Vocabulary

| Term | Meaning | Worldwright implication |
|---|---|---|
| **Client** | Software used to interact with a server | A browser client is the initial target |
| **Telnet** | A protocol involving option negotiation, not just newline-delimited text | Legacy support would require an actual negotiated adapter; plaintext credentials are not acceptable for public service |
| **TLS** | Transport encryption with peer authentication according to its configuration | Secure transport is required for public credentials/session data |
| **WebSocket / WSS** | Bidirectional framed communication; WSS uses TLS | Transport alone does not define message schemas, identity, or authorization |
| **ANSI formatting** | Terminal control/formatting sequences used by many clients | Rendering and escaping need a deliberate safe subset; raw controls can be harmful |
| **GMCP** | Generic MUD Communication Protocol, commonly used for structured side-channel data with capable clients | Optional future compatibility work, not the same as Worldwright's proposed JSON-over-WebSocket contract |
| **MSSP** | MUD Server Status Protocol | A server-status convention, not authentication or game-state synchronization |
| **MCCP** | MUD Client Compression Protocol | Compression support adds negotiation and resource-limit considerations |
| **MXP** | MUD eXtension Protocol for richer client presentation | Not HTML equivalence and not permission to render untrusted markup |
| **MCP** | MUD Client Protocol in this context | Distinct from the AI-tooling term Model Context Protocol |
| **Trigger / alias / macro** | Client automation responding to output or shortening input | Not inherently cheating; game rules must define permitted automation |
| **Mapper** | Client-maintained or server-assisted navigation map | Server must not expose hidden locations merely to support a map |

These are short orientation definitions, not protocol specifications. Before implementing any extension, read its actual specification and the target client's behavior, then document negotiation, limits, and tests.

A client name is not a server category: for example, software called MUSHclient can be used with compatible servers beyond MUSHes. A client being able to connect says little about script, database, or content portability.

## Worldwright Terminology and Scope

### Preferred project description

> Worldwright is a proposed Rust development suite for multi-user domains: persistent, multiplayer, primarily text-driven worlds. It draws on MU* traditions without claiming drop-in compatibility with a legacy engine.

### Concept mapping

| Worldwright term | Related historical concepts | Important difference |
|---|---|---|
| Engine | Server, driver, hardcode | Shared runtime mechanisms, not a particular legacy VM |
| SDK | Mudlib APIs, programming facilities | Proposed Rust interfaces for trusted game modules |
| Content definition | Prototype, area record, room description | Versioned immutable release input, not automatically live mutable softcode |
| Entity instance | Object, mob, character instance | Explicit typed identity separate from a reusable definition |
| Typed command | Player command, action, verb invocation | Validated intent, not authority supplied by a client |
| Effect plan | Script result or state mutation | Reviewed application of allowed effects, not unrestricted callback writes |
| Receipt | Transaction/operation result | Durable retry reconciliation, not just a log line |
| World writer epoch | Ownership/fencing mechanism | Infrastructure authority, unrelated to a character's wizard flag |
| Studio | Builder tools / OLC | Starts with validated drafts and publication workflow, not unrestricted live editing |
| Console | Staff/admin tooling | Scoped operational and moderation controls, not universal secret access |
| Testkit | Fixtures, bots, replay utilities | Verification support; deterministic replay is promised only where demonstrated |

### Baseline versus possible directions

| Capability | Status in the current proposal |
|---|---|
| Browser-accessible room-based text worlds | Baseline target |
| Commands, communication, basic interactions, durable items/rewards | Baseline target, not implemented evidence |
| Setting-independent Rust game rules | Baseline SDK direction |
| Rich scene-oriented roleplay tools | Potential game/tool extension; not a verified feature |
| Player-authored persistent executable behavior | Deferred pending an isolation and capability design |
| MUSHcode, MUF, MPI, LPC, or MOO execution | Not included in the baseline |
| Legacy world/database import | No importer or compatibility guarantee established |
| Traditional-client protocol gateway | Possible later work with a named specification and tested client matrix |

Supporting a style of play is different from emulating the original engine that popularized it.

## Compatibility and Migration

### Separate the compatibility dimensions

1. **Transport compatibility:** Can the client negotiate and exchange frames/bytes correctly?
2. **Command compatibility:** Are grammar, target matching, aliases, and error behavior equivalent?
3. **Data compatibility:** Can records, identifiers, ownership, and relationships be preserved?
4. **Language compatibility:** Do scripts execute with the same values, lookup, evaluation, and scheduling semantics?
5. **Permission compatibility:** Do visibility, locks, effective owners, and privilege checks preserve security?
6. **Gameplay compatibility:** Are rules, progression, random outcomes, and resets equivalent enough for the migration's stated goal?
7. **Operational compatibility:** Are backup, restart, moderation, and maintenance procedures supported?

Each dimension needs a named target, version, acceptance tests, and limitations. Passing one does not imply the others.

### Practical migration rules

- Read real source, manuals, schemas, and representative licensed data.
- Preserve a mapping between source identifiers and destination identities.
- Distinguish reusable definitions from live instances and historical records.
- Inventory embedded code; descriptions may contain executable expressions or markup.
- Treat imported locks and privileged code as security-sensitive.
- Do not weaken privacy or grant broader access to make an import succeed.
- Treat old account credentials as a security migration; resets may be safer than reuse.
- Do not silently turn active behaviors into inert strings while claiming parity.
- Produce a match/differs/missing report and migration/recovery tests.
- Confirm rights to engine code, world content, contributions, and private player data separately.

An importer should parse and validate unknown material in isolation, not execute it in a production world. Worldwright's proposed permissive license cannot override a source engine's or content author's license.

## Choosing a Direction

Choose desired capabilities before choosing a label:

| Primary goal | Questions to resolve |
|---|---|
| Automated adventure/RPG | Combat/progression model, economy integrity, NPC behavior, downtime/recovery policy |
| Collaborative roleplay | Scene workflow, consent, character approval, logs/privacy, rules/adjudication support |
| User-built social world | Object ownership, building quotas, moderation, editable content scope |
| Programmable shared environment | Execution isolation, permissions, budgets, persistence, code review, language semantics |
| Migration of an existing community | Preservation requirements, source rights, client expectations, data and script compatibility |

For Worldwright, a sensible first proof is a small original room-based world. A second game with different rules or a stronger social focus can test whether engine mechanisms truly remain separate from the first game's assumptions.

That proof does not require implementing every historical scripting language.

## Common Misconceptions

| Misconception | Correction |
|---|---|
| “MUD means fantasy combat only.” | Broad usage includes many themes and interaction styles. |
| “MUSH and MUX are unrelated genres.” | TinyMUX identifies itself as a server for MUSHes; the distinction often concerns implementation. |
| “MUCK has one official acronym expansion.” | Multiple backronyms exist; explain the naming uncertainty. |
| “MOO means any object-oriented multiplayer engine.” | It identifies a specific programmable-world/language tradition. |
| “Social worlds have no complex code or game rules.” | Many contain substantial programming and automation. |
| “All players can program every MOO or MUCK.” | Privileges and community policies control access. |
| “All MU* worlds are direct TinyMUD forks.” | Influence and source ancestry are different. |
| “Telnet text and WebSocket text are interchangeable.” | Framing, negotiation, security, and application contracts differ. |
| “Softcode is automatically safe.” | Permissions, execution bounds, and isolation determine safety. |
| “A dbref is a globally permanent entity ID.” | Scope and reuse depend on the engine. |
| “A database dump is a tested backup.” | Recovery needs consistent data, required artifacts, and a successful restore rehearsal. |
| “A public MUD database is freely reusable.” | Code, content, and player information have separate rights and privacy obligations. |
| “A MUSH-style Rust game is PennMUSH-compatible.” | Style similarity establishes no command, language, or storage compatibility. |

## Reference Notes

References were consulted on October 20, 2026. They support terminology and selected architectural descriptions; they are not endorsements of current maintenance, security, capacity, or license compatibility. Historical pages can describe practices that should not be copied into a modern deployment.

- **Early MUD history:** [Richard Bartle's first-person account](https://mud.co.uk/richard/mudhist.htm). Useful for the original MUD's authorship and sequence of development.
- **MUSH definition and orientation:** [PennMUSH project wiki](https://github.com/pennmush/pennmush/wiki).
- **MUSH lineage context:** [PennMUSH repository](https://github.com/pennmush/pennmush).
- **MUX as MUSH server:** [TinyMUX FAQ](https://www.tinymux.org/faq/); avoid extrapolating release-specific features to every deployment.
- **MUCK naming:** [TinyMUCK overview and cited historical sources](https://en.wikipedia.org/wiki/TinyMUCK), a secondary reference for the backronym discussion.
- **MUCK language model:** [Fuzzball programming manual](https://fuzzball-muck.github.io/muckman/programming.html).
- **MUCK object/community vocabulary:** [Fuzzball terms and abbreviations](https://fuzzball-muck.github.io/muckman/terms.html); historical client and administrative advice is not current operational guidance.
- **MOO expansion and language:** [LambdaMOO programmer's manual](https://lambda.moo.mud.org/pub/MOO/ProgrammersManual.txt).
- **MOO server/database separation:** [LambdaMOO introduction](https://lambda.moo.mud.org/pub/MOO/html/ProgrammersManual_1.html).
- **MOO behavior/permissions:** [Verbs on Objects](https://lambda.moo.mud.org/pub/MOO/html/ProgrammersManual_7.html).
- **LPMud driver concept:** [FluffOS/MudOS driver explanation](https://www.fluffos.info/concepts/general/MudOSdriver.html).

Acronym expansions and community usage vary. Where a named project's own documentation differs from this general guide, use its version-specific terms for implementation work and record the distinction.
