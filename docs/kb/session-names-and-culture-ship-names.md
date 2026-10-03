# Session names and Culture ship names

Reference for naming Claude Code sessions with the names of the Minds and ships in Iain M.
Banks's Culture novels. It records how Culture ships are named, how Claude Code names,
lists and addresses sessions, and what is not established.

Sources: the English Wikipedia articles on The Culture and on each novel, read as raw
wikitext, and the Claude Code documentation pages for commands, command line options,
hooks and cross-session messaging, and the Claude Code changelog. Quoted phrases were
matched against those pages.

## Culture ship names

- A Culture ship and its Mind are one entity. Wikipedia: "a spaceship without a Mind would
  be considered damaged or incomplete to the Culture".
- The Minds choose their own names: "The Minds themselves choose their own names, and thus
  they usually express something about a particular Mind's attitude, character or aims".
  Names "range from funny to just plain cryptic".
- Warships keep the habit: "Even the names of warships retain this humorous approach,
  though the implications are much darker."
- Characters address a ship's Mind as "Ship". Wikipedia: "It seems normal practice to
  address the ship's Mind as 'Ship'".
- Ships are classed by a prefix: GSV (General Systems Vehicle), GCU (General Contact
  Unit), ROU (Rapid Offensive Unit), and others.

### Names stated in the Wikipedia articles

| Name | What the article states | Source |
|---|---|---|
| Sanctioned Parts List | habitation or factory ship | The Culture article |
| So Much For Subtlety | habitation or factory ship | The Culture article |
| All Through With This Niceness And Negotiation Stuff | warship | The Culture article |
| Attitude Adjuster | warship | The Culture article |
| Of Course I Still Love You | ambassador ship | The Player of Games |
| Funny, It Worked Last Time... | ambassador ship | The Culture article |
| Just Read the Instructions | sentient star ship | The Player of Games |
| Limiting Factor | sapient warship | The Player of Games |
| Arbitrary | ship with "a sense of humour, of its own" | The State of the Art |
| Size Isn't Everything | GSV, over 80 km long | Use of Weapons |
| Sleeper Service | Eccentric GSV | Excession |
| Lasting Damage | GSV, later a Mind that ends its own higher functions | Look to Windward |
| Experiencing a Significant Gravitas Shortfall | a running gag in the series | Look to Windward |
| Liveware Problem | ship with a humanoid avatar | Matter |
| Falling Outside The Normal Moral Constraints | "very slightly psychotic" warship | Surface Detail |
| Mistake Not… | ship of non-standard class | The Hydrogen Sonata |

This is a sample. A complete list was not collected.

### Existing uses of the names

- SpaceX named two autonomous spaceport drone ships Just Read the Instructions and Of
  Course I Still Love You, and a third A Shortfall of Gravitas.
- The Five Deeps Expedition named its craft after Culture ships and drones. The deep
  submergence vehicle Limiting Factor takes its name from the ship in The Player of Games.

## Claude Code session names

| Item | Behaviour |
|---|---|
| `--name`, `-n` | Sets a display name, shown in `/resume` and the terminal title. `claude --resume <name>` resumes by name. |
| `/rename [name]` | Renames the current session and shows the name on the prompt bar. Without a name it generates one. Works with `-p`. Requires v2.1.205. |
| Name sanitising | Wherever a name is set, control and invisible characters become spaces, and the name is capped at 200 characters. Requires v2.1.221. An empty name is rejected. |
| SessionStart hook | `hookSpecificOutput.sessionTitle` "sets the session title, with the same effect as /rename". It applies when `source` is `startup`, `resume` or `fork`. It is ignored on `clear` and `compact`. |
| UserPromptSubmit hook | Accepts `hookSpecificOutput.sessionTitle`. |
| Hook input | `session_title` holds a custom title when one is set. A hook can check it "to avoid overwriting an existing custom title". A generated title does not appear in it. |
| Duplicates | If another live session on the machine already has the name, Claude Code gives the new session a variant. Sessions can still share a name. |
| Default | Claude Code generates a title when none is set. |

### Addressing

- Claude finds a target with `ListAgents` and sends with `SendMessage`. `/list-agents`,
  also `/peers`, shows the session's own name on the first line and the sessions Claude can
  message.
- A session "answers to the name you set with the /rename command or the --name flag".
- In the prompt, `@name` mentions a session. A name with characters outside letters,
  digits, hyphens and underscores needs double quotes, for example `@"release notes"`.
- A received message shows the sender's name, for example "Message from @api-worker".
- A session whose title starts with `/` was unaddressable until a later fix.
- While a session is connected to Remote Control, `/list-agents` leaves out "any session
  name it can't attribute to a person". The row then reads "(unnamed session)". The
  documentation names `--name` and `/rename` typed at the terminal as attributable.
- Messages to a session on the same machine travel over a local socket. Messages to a cloud
  session travel through Anthropic servers. Finding a cloud session requires a claude.ai
  sign-in with Remote Control. An API key does not work.
- All but one name in the table above include spaces, commas or ellipses and need quotes in
  an `@` mention. The longest, at 52 characters, is under the 200 character cap.

### Cloud sessions

- The Claude Code Remote MCP server provides `set_session_title`, which renames a Claude
  Code Remote session by session ID. A session that holds this tool can rename another
  cloud session.
- A plugin installs in a cloud session through a session start script. See
  [claude-code-plugin-packaging.md](claude-code-plugin-packaging.md).

## Not established

- Whether a plugin's SessionStart hook sets the title of a cloud session. The hooks page
  does not say. It was not tested.
- Whether a name set by a hook counts as attributable while a session is connected to
  Remote Control.
- Whether `SendMessage` accepts a name with spaces, commas and ellipses without quotes.
- A complete list of Culture ship names. The Culture wiki on Fandom returned 403 to
  automated requests. Banks's own essay on the Culture could not be fetched because the
  host's TLS certificate did not match its name. Neither was bypassed.

## Related

- [claude-code-mods.md](claude-code-mods.md): hooks and plugin tiers.
- [claude-code-plugin-packaging.md](claude-code-plugin-packaging.md): packaging a plugin and
  loading it in cloud sessions.
- [session-creation-and-environments.md](session-creation-and-environments.md): how
  sessions are created.
