## English: the launcher's own words. Every id in the Simplified Chinese
## catalog has an entry here, with the same arguments.

## Common

common-none = None
common-not-set = Not set
common-edit = Edit
common-edit-more = Edit…
common-add = Add
common-auto = Automatic
common-show-more = Show more
common-show-less = Show less
common-more = More
common-copy = Copy
common-close = Close
common-technical-details = Technical details
common-cancel = Cancel
common-delete = Delete
common-done = Done
# Between the parts of one sentence, as in "cache; natives".
common-clause-separator = {"; "}
# Opens the folder with the item selected, named as each system names it.
common-reveal-macos = Show in Finder
common-reveal-windows = Show in File Explorer
common-reveal-linux = Show in file manager
# Between the items of a list, as in "Wrapper, After exit".
common-list-separator = {", "}
# The first item of a list and how many there are, as in "3 items: -Da, …".
common-list-first-of = { $count } items: { $first }, …

## Time

time-just-now = Just now
time-minutes-ago = { $count ->
    [one] { $count } minute ago
   *[other] { $count } minutes ago
}
time-hours-ago = { $count ->
    [one] { $count } hour ago
   *[other] { $count } hours ago
}
time-yesterday = Yesterday
time-days-ago = { $count ->
    [one] { $count } day ago
   *[other] { $count } days ago
}
time-months-ago = { $count ->
    [one] { $count } month ago
   *[other] { $count } months ago
}
time-years-ago = { $count ->
    [one] { $count } year ago
   *[other] { $count } years ago
}

## Loaders

loader-vanilla = Vanilla

## Language names are written in their own language, whatever the interface language.

language-simplified-chinese = 简体中文
language-english = English

## Navigation

route-home = Home
route-library = Library
route-discover = Discover
route-activity = Activity
route-accounts = Accounts
route-settings = Settings
nav-back = Back
nav-forward = Forward
nav-add-account = Add account
nav-add-account-help = Add an offline account to play
nav-current-account = Current account: { $name }
nav-switch-account = Switch account
nav-manage-accounts = Manage accounts…
# A line in the account chip's dropdown, as in “Microsoft · Sign in again”.
nav-account-needs-sign-in = { $kind } · Sign in again
nav-no-game = No game yet
nav-no-game-help = Create one in the Library
nav-current-game = Current game: launches and installs use it
nav-switch-game = Switch current game
detail-title-modpack = Modpack details
detail-title-mod = Mod details
detail-title-resource-pack = Resource pack details
detail-title-shader = Shader details

## Home

home-loading = Getting ready
home-continue = Continue
home-continue-eyebrow = Pick up where you left off
home-import = Bring your old game over
home-create = New
home-first-use-eyebrow = First time here
home-first-use-title = Keep a familiar world close
home-first-use-body = Import your old game; the rest of the settings can unfold later.
home-phase-verifying = Checking
home-phase-libraries = Libraries
home-phase-assets = Assets
home-phase-starting = Starting
home-phase-verifying-headline = Checking game files
home-phase-libraries-headline = Filling in libraries
home-phase-assets-headline = Preparing assets
home-phase-starting-headline = Starting the game
home-entering = Entering · { $title }
home-playing-eyebrow = In game
# A line under the hero while a game is running.
home-playing-caption = { $minutes ->
    [0] Just started
    [one] Played for { $minutes } minute
   *[other] Played for { $minutes } minutes
} · The launcher stays quiet
home-stop-game = Quit game
home-recovery-eyebrow = Needs a look
home-recovery-crashed-title = The game quit unexpectedly
home-recovery-failed-title = The last launch didn't succeed
home-recovery-interrupted = The last session didn't end cleanly.
home-recovery-exited-early = The game exited while starting.
home-recovery-exited-early-code = The game exited while starting (exit code { $code }).
# phase is the name of a launch step, such as "Assets".
home-recovery-stopped-at = It stopped at the “{ $phase }” step.
home-recovery-crashed = The game quit unexpectedly.
home-recovery-crashed-code = The game quit unexpectedly (exit code { $code }).
home-recover = Recover and continue
home-attention = Needs attention
# One notice, when more problems follow.
home-attention-more = { $detail } ({ $more } more { $more ->
    [one] issue
   *[other] issues
})
home-recent = Recent
home-continued = Continue playing
home-record = Play history
home-record-play-time = Play time
home-record-worlds = Worlds
home-record-servers = Servers
home-places-empty = No worlds yet
home-places-empty-help = Press "Continue" to enter the game; worlds you create will show up here
home-place-enter = Enter
home-place-world = World
home-place-last-played = Last played { $when }
home-place-hardcore = Hardcore
home-place-server = Server · { $address }
home-meta-last-played = { $meta } · Last played { $when }
home-meta-never-played = { $meta } · Never played

## Home hero: the world's way of saying it, with a little in-game play (design language §8).

hero-dawn-eyebrow = Overworld
hero-dawn-title = A new day starts with the first block
hero-dawn-caption = The sun is square, the clouds are flat, and everything is just right.
hero-dawn-hud = Time { $time }
hero-caves-eyebrow = Caves
hero-caves-title = Torchlight falls one level with every step
hero-caves-caption = Break the last block of stone and the lava's glow pours in on its own.
hero-caves-hud = Torches ×{ $torches }
hero-redstone-eyebrow = Redstone
hero-redstone-title = After fifteen blocks the signal goes out
hero-redstone-caption = Two wires of the same length, and only the one through a repeater lit the lamp.
hero-redstone-hud = Signal { $signal }
hero-portal-eyebrow = Nether
hero-portal-title = Four by five obsidian, and another world catches fire
hero-portal-caption = One strike of flint and steel, and purple light spills across the netherrack.
hero-portal-hud-active = Portal active
hero-portal-hud-igniting = Portal igniting
hero-portal-hud-inactive = Portal inactive
hero-hearth-eyebrow = Camp
hero-hearth-title = The campfire is still burning
hero-hearth-caption = The world paused the moment you left.
hero-hearth-hud = Campfire light { $light }
hero-loading-hud = Chunks { $loaded }/{ $total }

## Library

library-tab-all = All games
library-tab-favorites = Favorites
library-tab-collections = Collections
library-sort = Sort by
library-sort-recent = Recently played
library-sort-name = Name
library-sort-created = Date created
library-loader = Loader
library-game-count = { $count ->
    [one] { $count } game
   *[other] { $count } games
}
library-loading = Reading…
library-empty = No games yet
library-empty-help = Create one, or install a modpack from Discover
library-favorites-empty = No favorites yet
library-favorites-empty-help = Star a card and the games you play most will show up here
library-no-match = No matching games
library-no-match-help = Try another keyword
library-never-played = Never played
library-play = Play
library-favorite = Add to favorites
library-unfavorite = Remove from favorites
library-menu-play = Play
library-menu-open = Open
library-menu-make-current = Make current game
library-menu-collections = Add to collection…
library-menu-copy = Copy…
library-menu-export = Export modpack…
library-delete = Delete…
library-new-game = New game
library-import-pack = Import modpack
library-import-game = Import from another launcher…
library-restore = Restore from backup…
library-open-folder = Open Library folder
library-collections-none = No collections yet
library-collections-none-help = Group games your own way, such as "Survival", "Servers", "Modpacks"
library-collection-new = New collection
library-collection-rename = Rename…
library-collection-delete = Delete collection
library-collection-empty = This collection is empty; pick "Add to collection…" from a game card's ⋯ menu
collection-name-required = Enter a name
collection-name-taken = A collection named "{ $name }" already exists
collection-name-placeholder = e.g. Survival, Modpacks
collection-new-placeholder = Or create a new collection
collection-add-title = Add to collection
collection-add-none = No collections yet; name one below to create it
collection-delete-title = Delete collection "{ $name }"?
collection-delete-body = Only this collection is deleted; its games stay in the Library.
game-delete-title = Delete "{ $name }"?
game-delete-body = The game folder, saves and history are deleted with it, and can't be recovered.
reclaim-title = Clean up { $size } of unused game files?
reclaim-body = No game uses them. Deleting them means they'll be downloaded again when needed.
reclaim-body-kept =
    No game uses them. Deleting them means they'll be downloaded again when needed.
    Not cleaned up: { $kept }
reclaim-confirm = Clean up
collection-rename-title = Rename collection
collection-rename-confirm = Rename
collection-create-confirm = Create
collection-renamed = Collection renamed to "{ $name }"
collection-created = Collection "{ $name }" created
collection-save-failed = Couldn't save the collection
collection-updated = Collection updated
collection-update-failed = Couldn't update the collection
library-import-game-prompt = Choose another launcher's game folder
library-import-game-none = No importable game was found in this folder
library-import-game-started = Importing { $name }; progress is in Activity
library-import-game-done = Imported "{ $name }"
library-import-game-failed = Didn't import { $name }
library-restore-prompt = Choose a backup file (.zip)
library-restore-started = Starting the backup restore; progress is in Activity
library-restore-done = Restored as a new game "{ $name }"
library-restore-failed = Couldn't restore this backup
game-picker-title = Which game to import
game-picker-body = This folder has more than one game. Importing copies the player files (mods, saves, settings); the originals are left untouched.
game-picker-import = Import

## Discover

discover-subtitle = Find what to play next
discover-kind-modpack = Modpack
discover-kind-resource-pack = Resource pack
discover-kind-shader = Shader
discover-searching = Searching…
discover-no-source = No content source is available
discover-no-source-help = Turn on a content source (such as Modrinth) in Settings › Plugins, then come back
discover-offline = You're offline, or Modrinth can't be reached
discover-offline-help = Connect to the network and try again
discover-no-results = No results
discover-no-results-help = Try another keyword or loosen the filters
discover-browse-modrinth = Browse on Modrinth
discover-search-again = Search again
discover-page-size = Results per page
discover-sort-relevance = Relevance
discover-sort-downloads = Downloads
discover-sort-follows = Followers
discover-sort-newest = Newest
discover-sort-updated = Recently updated
# what is the filter's name (game version, loader); value is the game's value, such as "1.21.1".
discover-locked = { $what } comes from the game: { $value }
discover-locked-help = Unlocking shows content that doesn't fit this game; installing still picks only the versions that fit.
discover-unlock = Unlock filters
discover-sync = Sync with the game
discover-all-versions = Show all versions
discover-open-source = Open source
discover-advanced-remembered = Your choices here are remembered and still apply next time you visit Discover.
discover-hide-installed = Hide installed
discover-filters-not-loaded = Filters didn't load
discover-clear-filters = Clear filters
discover-install = Install
discover-installing = Installing…
discover-update = Update
discover-installed = Installed
discover-open-modrinth = Open on Modrinth
discover-copy-link = Copy link
discover-link-copied = Project link copied
discover-install-started = Installing { $title }; progress is in Activity
discover-modpack-installed = Installed modpack { $name }
discover-install-failed = Didn't install { $title }
discover-needs-game = Create a game in the Library first
discover-installed-into = Installed { $file } into { $game }
discover-dependencies-failed = { $dependencies } didn't install; { $title } may not launch
environment-client-or-server = Client or server
environment-client-and-server = Client and server
environment-client = Client
environment-server = Server
environment-singleplayer = Singleplayer
environment-dedicated-server = Dedicated server

## Discover filter groups and advanced excludes

discover-exclude-ai-content = AI-generated content
discover-exclude-ai-content-code = AI code
discover-exclude-ai-content-assets = AI assets
discover-exclude-ai-content-text = AI text
discover-exclude-ai-functionality = Generative AI functionality
discover-exclude-advertisements = Advertisements
discover-exclude-system-interactions = External system interactions
discover-exclude-telemetry = Telemetry
discover-exclude-telemetry-opt-in = Opt-in telemetry
discover-exclude-telemetry-opt-out = Opt-out telemetry
discover-exclude-telemetry-always-active = Always-active telemetry
discover-exclude-paid-features = Paid features
discover-exclude-archived = Archived
discover-exclude-plugin = Plugin
discover-exclude-datapack = Data pack
discover-exclude-epilepsy = Photosensitivity triggers
discover-section-version = Game version
discover-section-loader = Loader
discover-section-environment = Environment
discover-section-license = License
discover-section-advanced = Advanced exclusions
discover-section-categories = Categories
discover-section-features = Features
discover-section-resolutions = Resolution
discover-section-performance = Performance impact

## Modrinth's category, feature and loader tags, looked up at runtime as tag-<Modrinth name>; proper nouns aren't here.

tag-adventure = Adventure
tag-cursed = Cursed
tag-decoration = Decoration
tag-economy = Economy
tag-equipment = Equipment
tag-food = Food
tag-game-mechanics = Game mechanics
tag-library = Library
tag-magic = Magic
tag-management = Management
tag-minigame = Minigame
tag-mobs = Mobs
tag-optimization = Optimization
tag-social = Social
tag-storage = Storage
tag-technology = Technology
tag-transportation = Transportation
tag-utility = Utility
tag-worldgen = World generation
tag-challenging = Challenging
tag-combat = Combat
tag-kitchen-sink = Kitchen sink
tag-lightweight = Lightweight
tag-multiplayer = Multiplayer
tag-quests = Quests
tag-audio = Audio
tag-blocks = Blocks
tag-core-shaders = Core shaders
tag-entities = Entities
tag-environment = Environment
tag-fonts = Fonts
tag-gui = GUI
tag-items = Items
tag-locale = Locale
tag-modded = Modded
tag-models = Models
tag-realistic = Realistic
tag-simplistic = Simplistic
tag-themed = Themed
tag-tweaks = Tweaks
tag-vanilla-like = Vanilla-like
tag-atmosphere = Atmosphere
tag-bloom = Bloom
tag-cartoon = Cartoon
tag-colored-lighting = Colored lighting
tag-fantasy = Fantasy
tag-foliage = Foliage
tag-path-tracing = Path tracing
tag-reflections = Reflections
tag-semi-realistic = Semi-realistic
tag-low = Low
tag-medium = Medium
tag-high = High
tag-potato = Potato
tag-screenshot = Screenshot
tag-vanilla = Vanilla
tag-plugin = Plugin
tag-datapack = Data pack
tag-features = Features

## Library and Discover search and filters

library-search = Search games
library-all-loaders = All loaders
discover-search = Search Modrinth, press Enter
discover-version-search = Search versions

## Settings

settings-title = Settings
settings-subtitle = The launcher itself, and the values every game starts from
settings-loading = Reading settings…
settings-tab-general = General
settings-tab-game-defaults = Game defaults
settings-tab-java = Java
settings-tab-downloads = Downloads & storage
settings-tab-about = About
settings-tab-plugins = Plugins

settings-follow-system = System
settings-appearance = Appearance
settings-appearance-light = Light
settings-appearance-dark = Dark
settings-after-launch = When the game starts
settings-after-launch-keep = Keep
settings-after-launch-hide = Hide launcher
settings-foreground = Come back when the game exits
settings-foreground-help = Brings the launcher to the front when a game ends, so you can pick the next one.
settings-motion = Reduce motion
settings-motion-reduce = Reduce
settings-motion-full = Full
settings-language = Language
settings-language-help = The language of the launcher. Game logs and project descriptions stay as written.

settings-memory = Memory
settings-memory-value = Min { $min } · Max { $max }
settings-memory-help = This computer has { $total }; the recommended maximum is { $recommended } MB. Leave it empty to let Java decide; each game can also set its own.
settings-memory-help-unknown = Leave it empty to let Java decide; each game can also set its own.
settings-memory-dialog = Default memory
settings-memory-intro = The memory every game uses unless it sets its own. Leave empty for no limit.
settings-memory-min = Minimum memory (MB)
settings-memory-min-help = Memory the game takes as soon as it starts (-Xms).
settings-memory-max = Maximum memory (MB)
settings-memory-max-help = The most memory the game can use (-Xmx).
settings-memory-placeholder = Not set
settings-window = Window size & fullscreen
settings-window-intro = Fill in width and height together; leave both empty for the game's own default (854 × 480).
settings-window-width = Width
settings-window-height = Height
settings-window-fullscreen = Start fullscreen
settings-window-fullscreen-value = Fullscreen
settings-fullscreen-off = Off
settings-fullscreen-on = On
settings-fullscreen-unset = Not set
settings-one-per-line = One argument per line.
settings-jvm = Java arguments
settings-jvm-help = Extra arguments for Java, such as -XX:+UseG1GC. A game's own arguments take precedence.
settings-game-args = Game arguments
settings-game-args-help = Extra arguments for the game itself, such as --demo.
settings-env = Environment variables
settings-env-help = Extra environment variables for the game process.
settings-env-intro = One per line, written as NAME=value.
settings-commands = Before, wrapper and after-exit commands
settings-commands-help = You write these commands yourself; they run with your user's permissions.
settings-commands-dialog = Commands
settings-commands-intro = Commands run with your user's permissions, and only once you fill them in. Variables: $INST_ID, $INST_NAME, $INST_DIR (the game folder), $INST_JAVA, $INST_MC_VERSION, $INST_LOADER.
settings-command-pre = Before launch
settings-command-pre-short = Before launch
settings-command-pre-help = Runs before the game starts; if it fails (exit code not 0), this launch is cancelled.
settings-command-pre-placeholder = e.g. ./prepare.sh
settings-command-wrapper = Wrapper
settings-command-wrapper-short = Wrapper
settings-command-wrapper-help = Goes in front of the Java command, such as gamemoderun or mangohud.
settings-command-wrapper-placeholder = e.g. gamemoderun
settings-command-post = After exit
settings-command-post-short = After exit
settings-command-post-help = Runs after the game exits; a failure is only recorded. It may run for up to 60 seconds.
settings-command-post-placeholder = e.g. ./cleanup.sh
settings-section-resources = Resources
settings-section-arguments = Arguments & environment
settings-section-commands = Commands

settings-java-none = No Java found
settings-java-none-help = Choose one with “Add Java”, or add the folder it lives in to the extra search folders.
settings-java-enable = Use this Java
settings-java-disabled = { $title } (turned off)
settings-java-roots = Extra search folders
settings-java-roots-help = Java is also looked for in these folders, besides the usual places.
settings-java-roots-intro = One folder per line. Each game picks a suitable Java on its own.
settings-java-roots-field = Folders
settings-java-roots-count = { $count ->
    [one] { $count } folder
   *[other] { $count } folders
}
settings-java-found = Java found
settings-java-rescan = Detect again
settings-java-install = Download recommended Java
settings-java-add = Add Java…

settings-storage-games = Games
settings-storage-shared = Shared files
settings-storage-java = Java
settings-storage-cache = Cache
settings-mirror-added = Added
settings-mirror-not-added = Not added
settings-bmclapi-help = Game files, Forge, NeoForge, Fabric and authlib-injector. Once added, you can choose “Mirror first”.
settings-mcim-help = Modrinth and CurseForge metadata and files. The official source is always tried first and MCIM only after it fails; never with “Official only”.
settings-tencent-maven = Tencent Maven
settings-tencent-maven-help = Java libraries from Maven Central.
settings-download-source = Download source
settings-download-source-help = Official only never uses a mirror. The other modes try the next source when the preferred one fails; MCIM is always the fallback after the official source. Official first by default.
settings-source-official-only = Official only
settings-source-official-first = Official first
settings-source-mirror-first = Mirror first
settings-mirrors = Mirror rules
settings-mirrors-help = Replaces the start of an official address with the start of a mirror address.
settings-mirrors-intro = One rule per line, written as official prefix => mirror prefix.
settings-mirrors-field = Rules
settings-mirrors-count = { $count ->
    [one] { $count } rule
   *[other] { $count } rules
}
settings-concurrency = Downloads at once
settings-concurrency-help = How many files download at the same time, 1 to 32. Lower it on an unsteady network.
settings-concurrency-intro = Leave empty to let the launcher decide.
settings-concurrency-field = Count (1–32)
settings-data-dir = Data folder
settings-data-dir-help = Games, shared files and settings all live here. It can't be changed.
settings-reclaim = Check for unused game files
settings-clear-cache = Clear cache ({ $size })
settings-usage-measuring = Measuring…
settings-section-downloads = Downloads
settings-section-storage = Storage
settings-section-usage = Usage

settings-version = Version
settings-updates = Check for updates
settings-updates-help = The launcher can't update itself yet.
settings-updates-value = Not yet available
settings-logs = Launcher logs
settings-logs-help = A record of downloads and installs.
settings-diagnostics = Diagnostics
settings-diagnostics-help = Packs the version, a settings summary, the Java list and each game's recent logs. Player names, UUIDs and your folder paths are replaced; for the before, wrapper and after-exit commands it only records whether they are set.
settings-diagnostics-export = Export…
settings-license = License

settings-plugins-none = No plugins
settings-plugins-none-help = Core plugins appear here, and you can turn each on or off.
settings-plugin-read-files = Read the game's { $folder } folder
settings-plugin-network = Connect to { $hosts }
settings-plugin-launch-events = Know when games start and exit
settings-plugin-discord = Show game status in Discord on this computer
settings-plugin-enabled = On
settings-plugin-disabled = Off
settings-plugin-failed = Paused for this run; tried again after a restart
settings-plugin-enable = Turn on plugin
settings-plugin-state = On
settings-plugin-permission = Permission
settings-plugin-failure = Why it failed
settings-plugin-failure-value = This run paused the plugin
settings-plugin-enable-setting = Turn on this setting
settings-plugin-defaults = Defaults
settings-plugin-reset = Restore defaults
