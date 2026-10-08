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
common-remove = Remove
common-save = Save
common-retry = Retry
common-open = Open
common-name = Name
common-expand = Expand
common-browse = Browse…
common-restore-defaults = Restore defaults
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

## Accounts

# The Accounts page header, empty state and account rows.
account-add-offline = Add offline account
account-sign-in-microsoft = Sign in with Microsoft
account-menu-third-party = Third-party sign-in…
account-menu-servers = Auth servers…
account-empty-title = No accounts yet
account-empty-help = Sign in with Microsoft to join official servers; sign in to third-party auth servers such as LittleSkin from the ⋯ menu in the top right; an offline account needs no sign-in, and its name is the name you use in game.
account-subtitle-current = Current: { $name } ({ $kind })
account-subtitle-empty = No accounts yet; add one to play
account-needs-sign-in = Sign in again
account-chip-current = Current
account-kind-offline = Offline account
account-kind-third-party = Third-party account
account-skin-local = Local skin
account-skin-littleskin = LittleSkin skin
account-skin-site = Skin site skin
account-detail-custom-uuid = Custom UUID
account-copied-uuid = UUID copied
account-menu-copy-uuid = Copy UUID
account-menu-skin = Skin…
account-menu-refresh = Refresh sign-in
account-menu-remove = Remove…
account-remove-title = Remove account "{ $name }"?
account-remove-body-signed-in-current = This identity is forgotten and its sign-in is deleted from the system credential store (a third-party account also tells the server to invalidate its token); no games or saves are deleted. It's the current account, so the first remaining one takes over.
account-remove-body-signed-in = This identity is forgotten and its sign-in is deleted from the system credential store; no games or saves are deleted.
account-remove-body-offline-current = This identity is only forgotten; no games or saves are deleted. It's the current account, so the first remaining one takes over.
account-remove-body-offline = This identity is only forgotten; no games or saves are deleted.

# The offline account add dialog.
account-name-label = Name
account-name-required = Enter a name
account-name-too-long = Name can be at most { $count ->
    [one] { $count } character
   *[other] { $count } characters
}
account-name-invalid-char = Name can only use letters, numbers and underscores; "{ $character }" isn't allowed
account-name-help = At most { $count ->
    [one] { $count } character
   *[other] { $count } characters
}, using letters, numbers or underscores
account-name-help-first = At most { $count ->
    [one] { $count } character
   *[other] { $count } characters
}, using letters, numbers or underscores. This is the first account and becomes the current one automatically
account-name-placeholder = e.g. Steve
account-uuid-placeholder = Leave empty: decided by the name
account-uuid-help = The game uses it to recognize you, and player data in saves is stored under it. Leave it empty and the name decides it (the same name always gets the same one); it can only be set when adding and can't be changed later.
account-uuid-problem = UUID needs 32 hexadecimal digits (hyphens allowed)
account-advanced-expand = Advanced options
account-advanced-collapse = Hide advanced options

# The offline account skin dialog.
account-skin-title = { $name }'s skin
account-skin-kind-default = Default
account-skin-kind-local = Local file
account-skin-kind-littleskin = LittleSkin
account-skin-kind-site = Skin site (CustomSkinLoader)
account-skin-model-classic = Classic (wide arms)
account-skin-model-slim = Slim (narrow arms)
account-skin-model-label = Model
account-skin-skin-label = Skin image
account-skin-skin-placeholder = Skin image (PNG)
account-skin-cape-label = Cape image
account-skin-cape-placeholder = Cape image (PNG, optional)
account-skin-api-label = Skin site address
account-skin-api-placeholder = Skin site address (CustomSkinLoader API)
account-skin-browse = Browse…
account-skin-pick-picture = Choose image
account-skin-choose-picture = Choose a skin image, or a cape image
account-skin-enter-address = Enter the skin site address
account-skin-save = Save
account-skin-api-help = The address must serve <player name>.json and a textures/ folder; LittleSkin and Blessing Skin skin sites both work this way.
account-skin-default-help = The game picks a default skin from the player's UUID on its own; nothing extra is needed at launch.
account-littleskin-hint = You need to create a character on LittleSkin with the same name as this offline account. The account's skin then comes from whatever that character has set on the skin site.
account-skin-agent-note = Once a skin is chosen, the launcher starts a small local skin server when launching the game and loads authlib-injector (downloaded automatically the first time).
account-open-littleskin = Open LittleSkin

# Sign-in failure sentences (Microsoft, third-party auth and skin sites).
account-auth-declined = You declined this sign-in in the browser
account-auth-expired = The code has expired; start the sign-in again
account-auth-cancelled = Sign-in cancelled
account-auth-sign-in-required = Your sign-in has expired; sign in again
account-auth-no-xbox = This Microsoft account has no Xbox profile yet; create one at xbox.com first
account-auth-child = This is a child account; a parent must allow online play in the Microsoft family group
account-auth-xbox-unavailable = Xbox services aren't available in your region
account-auth-adult-verification = This account must complete adult verification on the Xbox website first
account-auth-no-game = This account doesn't own Minecraft: Java Edition
account-auth-services-refused = Minecraft services refused this launcher's sign-in; the app registration may not be approved by Mojang yet
account-auth-credential-store = The system credential store isn't available, so the sign-in can't be saved securely and didn't happen
account-auth-network = Can't reach the sign-in service; check your network and try again
account-auth-protocol = The sign-in service gave an unexpected response
account-yggdrasil-network = Can't connect to the auth server. It may be a network problem; check that the device is online, or use a proxy.
account-yggdrasil-malformed = Can't parse the auth server's response; the server may be down.
account-yggdrasil-credentials = Wrong username or password, or too many attempts have temporarily blocked sign-in; try again later.
account-yggdrasil-session-expired = Your sign-in has expired; sign in again.
account-yggdrasil-no-character = This account has no character on this server
account-yggdrasil-character-deleted = This character has been deleted
account-yggdrasil-invalid-token = Your sign-in has expired; sign in again
account-yggdrasil-migrate = Your account needs to be migrated to a Microsoft account. If it already has been, sign in with the migrated Microsoft account
account-skin-error-io = Can't read this skin file
account-skin-error-picture = Unrecognized skin file; it needs to be a PNG image
account-skin-error-network = Can't reach the skin site; check your network and the address
account-skin-error-malformed = The skin site gave an unexpected response
account-skin-error-invalid-api = The skin site address isn't valid
account-failure-sign-in-required = Account { $name } needs to sign in again
account-failure-injector = Can't download authlib-injector. It may be a network problem; check your network, try another download source, or use a proxy
account-failure-no-pending = This sign-in has expired; sign in again
account-failure-duplicate = An account named "{ $name }" already exists (names aren't case-sensitive)
account-failure-duplicate-uuid = This UUID is already used by another account
account-failure-profile = The name or UUID doesn't meet the requirements
account-failure-save = Couldn't save the account

# The auth server dialog.
account-server-label = Auth server
account-server-add-title = Add auth server
account-server-add-menu = Add auth server…
account-server-address-placeholder = e.g. littleskin.cn or the auth server's API address
account-server-find = Find
account-server-builtin = { $url } · Built-in
account-server-accounts = { $url } · { $count ->
    [one] { $count } account
   *[other] { $count } accounts
}
account-server-already-listed = This server is already in the list
account-server-remove-title = Remove auth server "{ $name }"?
account-server-remove-body = Only this server is forgotten; you can add it again later.
account-server-remove-body-accounts = This server is forgotten, and it removes { $count ->
    [one] { $count } account
   *[other] { $count } accounts
} signed in on it (along with their sign-ins in the system credential store). Games and saves aren't affected.

# The third-party account sign-in dialog.
account-third-party-title = Sign in to a third-party account
account-login-username = Username
account-login-email = Email
account-password-label = Password
account-password-hint = The password is only sent to the auth server you chose; the launcher doesn't store it and only puts the token the server returns into the system credential store.
account-http-warning = Warning: this server uses unencrypted HTTP, so your password is sent in the clear when you sign in.
account-sign-in-action = Sign in
account-choose-character-action = Choose
account-choose-character = This account has multiple characters; choose one to play

# The Microsoft sign-in dialog.
account-microsoft-title = Sign in with Microsoft
account-microsoft-sign-in = Sign in in the browser
account-microsoft-retry = Try again
account-microsoft-cancel-sign-in = Cancel sign-in
account-microsoft-intro = Choose "Sign in in the browser", enter a code on the page that opens, then come back here.
account-microsoft-storage-note = Your sign-in is kept only in the system credential store and is never written to the launcher's files.
account-microsoft-requesting = Asking Microsoft for a sign-in code…
account-microsoft-code-lead = Enter this code at { $address }, then sign in and confirm on the page:
account-microsoft-copy-code = Copy code
account-microsoft-reopen = Reopen the page
account-microsoft-waiting = Waiting for you to finish signing in in the browser…

# Toast messages for account actions.
account-signed-in = Signed in as { $name }
account-added = Added account { $name }
account-server-added = Added auth server { $name }
account-server-removed = Removed auth server
account-skin-saved = Skin saved; it takes effect on the next launch

## Activity

# Task speed, as in "3.2 MB/s".
activity-rate-bytes = { $rate }/s
activity-rate-files = { $rate } files/s
activity-eta-under-minute = Under a minute
activity-eta-minutes = { $count ->
    [one] About { $count } minute
   *[other] About { $count } minutes
}
activity-eta-hours-minutes = About { $hours } { $hours ->
    [one] hour
   *[other] hours
} { $minutes } { $minutes ->
    [one] minute
   *[other] minutes
}
# Time remaining at the right of a task row; left is a phrase such as "About 3 minutes".
activity-remaining = { $left } left
activity-status-running = In progress
activity-status-done = Done
activity-status-failed = Failed
activity-status-cancelled = Cancelled
activity-empty-title = No activity yet
activity-empty-help = Downloads and installs show up here
activity-tab-empty = Nothing in this category
activity-clear-finished = Clear finished
activity-none-running = Nothing is running right now
activity-running-count = { $count ->
    [one] { $count } thing in progress
   *[other] { $count } things in progress
}
# Launch recovery notes, the most important one first.
activity-recovery-library = The Library file couldn't be read; the original was kept and it started over
activity-recovery-library-candidates = The Library file couldn't be read; the original was kept and it started over; found { $count ->
    [one] { $count } possible game folder
   *[other] { $count } possible game folders
} on disk
activity-recovery-settings = The settings file couldn't be read; the original was kept and defaults are in use
activity-recovery-stuck = The interrupted operation couldn't be finished; the files were kept and it will try again next launch
activity-recovery-conflict = An interrupted record was found that can't be handled automatically; the files involved weren't changed
activity-recovery-restore-rolled-back = The last snapshot restore was interrupted; everything is back to how it was before
activity-recovery-session-interrupted = The launcher quit while a game was running last time; the result of that session is unknown
activity-recovery-profile-missing = A game's folder is missing; check it in Diagnostics
activity-recovery-delete-rolled-back = The last delete was interrupted; the game was kept as it was
activity-recovery-delete-completed = The interrupted delete finished
activity-recovery-publish-completed = The interrupted import or copy finished
activity-recovery-publish-discarded = The interrupted import or copy didn't finish and was cleaned up; you can start again
activity-recovery-log-skipped = { $count ->
    [one] { $count } line in the activity log couldn't be read and was skipped
   *[other] { $count } lines in the activity log couldn't be read and were skipped
}
activity-recovery-more = { $headline } ({ $more } more { $more ->
    [one] recovery record
   *[other] recovery records
})
# Retrying a failed or cancelled task.
activity-retry-started = Started again; progress is in Activity
activity-retry-ok = It worked this time
activity-retry-failed = It still didn't work
# Activity's category tabs.
activity-tab-all = All
activity-tab-download = Downloads
activity-tab-install = Installs
activity-tab-update = Updates
activity-tab-repair = Repairs

## New game

new-game-loader-stable = Stable
new-game-loader-latest = Latest
new-game-loader-other = Other
new-game-install-help = Download them now to make the first launch faster; you can play without them, since starting the game fills them in.
new-game-channel-snapshot = Snapshot
new-game-channel-pre-release = Pre-release
new-game-channel-candidate = Release candidate
new-game-channel-old = Old
new-game-loader-tag-stable = Stable
new-game-loader-tag-testing = Testing
new-game-default-name = New { $loader } game
new-game-change-warning = Installed mods may not be compatible with the new game version or loader. Worlds and settings aren't changed; creating a snapshot first is recommended so you can restore if something goes wrong.
new-game-pick-game-version = Choose a game version
new-game-pick-loader-version = Choose a loader version
new-game-resolved-note = Will install { $loader } { $version }
new-game-change = Change
new-game-create = Create
new-game-change-title = Change game version and loader
new-game-loader-unsupported = { $loader } doesn't support this game version yet; try another version
new-game-loader-version-label = Loader version
new-game-install-now = Download game files right after creating
new-game-snapshot-first = Create a snapshot first
new-game-read-game-versions = Can't read the version list
new-game-read-loader-versions = Can't read { $loader } versions
new-game-loader-unsupported-install = { $loader } can't be installed yet; support is on the way
new-game-create-failed = Couldn't create it; you can try again
new-game-created-downloading = Created { $name }; downloading game files
new-game-created = Created { $name }
new-game-download-failed = The game files didn't finish downloading; starting the game will try again
new-game-import-pack-prompt = Choose a modpack (.mrpack, or a MultiMC / Prism .zip)
new-game-import-pack-done = Imported modpack { $name }

## Project details

project-tab-about = About
project-tab-versions = Versions
project-tab-gallery = Gallery
project-channel-release = Release
project-channel-beta = Beta
project-channel-alpha = Alpha
project-offline = Can't reach Modrinth
project-offline-help = Check your network and open it again
project-gallery-empty = This project has no gallery
project-open-in-browser = Open in browser
project-switch-version = Switch version
project-install-as-new-game = Install as a new game
project-install-into = Install into { $name }
project-switch = Switch
project-add-game-version = Add game version
project-versions-empty = No versions available yet
project-version-unfit = Doesn't fit the current game
project-column-channel = Channel
project-column-published = Published
project-save-as = Save as
project-download-version-file = Download this version's file · { $size }
project-versions-no-match = No versions match the filters
project-versions-no-match-help = Try loosening the filters

## Dependencies

dependency-install-alone = Install just it
dependency-install-together = Install together ({ $count ->
    [one] { $count } item
   *[other] { $count } items
})
dependency-needs-more = { $mod_title } also needs these
dependency-about = About { $mod_title }
dependency-no-version = { $title } (no version that fits this game)
dependency-needed = Required: without them, this mod probably won't launch. Unchecked ones aren't installed.
dependency-optional = Optional: they add features, and it works without them.
dependency-conflicts = It declares that it's incompatible with the installed { $conflicts }; using them together may not launch.

## Export modpack

export-name-required = Enter a modpack name
export-version-required = Enter a version number
export-include-required = Choose at least one thing to include
export-version-placeholder = e.g. 1.0.0
export-summary-placeholder = One-line description (optional)
export-ok = Choose a location and export
export-title = Export modpack
export-version-label = Version
export-summary-label = Summary
export-format-label = Format
export-include-label = What goes into the modpack
export-note = Files that are on Modrinth are listed by address; the rest are packed in directly. Saves and logs aren't included by default.

## Version picker

version-picker-no-match = No matching versions
version-picker-loading = Reading versions…

## Model preview

model-title = 3D schematic preview
model-open = Open 3D preview
model-error-no-gpu = This computer has no usable graphics device, so the 3D preview can't be shown
model-error-parse = Can't read this schematic file
model-error-pack = Can't read the game's texture pack
model-error-mesh = Couldn't build the 3D preview
model-error-render = Couldn't draw the 3D preview
model-undrawable = { $count ->
    [one] { $count } block type can't be drawn for this game version and is missing from the preview
   *[other] { $count } block types can't be drawn for this game version and are missing from the preview
}
model-read-failed = Couldn't read the 3D preview
model-needs-game = Once this game is installed, you can preview schematics with its textures
model-capture-unsupported = The current window system doesn't support mouse capture yet; use Orbital mode
model-capture-failed = Couldn't capture the mouse; click the view to try again
model-capture-ended = Mouse capture ended; click the view to re-enter
model-loading = Building the 3D preview…
model-aria = 3D schematic preview; in Orbital mode, arrow keys rotate and plus/minus zoom; in Explore mode, click or press Enter to capture the mouse, WASD to move, Space to rise, Shift to descend, Esc to release; R resets
model-reset = Reset view
model-hint-orbital = Drag to rotate · Scroll to zoom · Arrow keys rotate · + / − to zoom
model-hint-explore-captured = WASD to move · Space to rise · Shift to descend · Esc to release the mouse
model-hint-explore = Click the view or press Enter to enter · WASD to move · Space to rise · Shift to descend

## Placeholder pages

placeholder-library-body = Your games gather here.
placeholder-discover-title = Nothing discovered yet
placeholder-discover-body = Mods, resource packs and shaders show up here.
placeholder-activity-title = Nothing in progress
placeholder-activity-body = Downloads, installs and repairs leave a trace here when they finish.
placeholder-accounts-body = Add an offline account to play.
placeholder-settings-title = Loading settings
placeholder-settings-body = The launcher and game defaults live here.

## Photosensitivity

photosensitivity-title = About the photosensitivity filter
photosensitivity-body = This filter only excludes projects whose authors declare flashing or strobing photosensitivity content. Projects without a declaration aren't excluded, so it can't guarantee the content is safe for people with photosensitive epilepsy.
photosensitivity-dismiss = Got it, don't show again
photosensitivity-ok = Got it

## Plugin settings

plugin-setting-title = Edit { $label }
plugin-setting-integer = Enter a whole number
plugin-setting-integer-range = Enter a whole number between { $min } and { $max }
plugin-setting-choice-only = Choose this setting in its row instead
plugin-setting-save-failed = Couldn't save this setting; try again.

## Launch

launch-already-running = A game is already running

## Logs

logs-saved = Saved to { $path } (names and paths redacted)
logs-export-failed = Couldn't export the logs

## App

app-quit = Quit LumilioCL

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

# Validation messages in settings dialogs.
settings-number-whole = { $what } needs a whole number
settings-number-range = { $what } must be between 1 and { $max } MB
settings-window-size = Width and height must both be filled in, from 1 to { $max }
settings-env-name-invalid = The environment variable name "{ $name }" can't be empty or contain equals signs, spaces or control characters
settings-control-character = Arguments and commands can't contain control characters
settings-wrapper-unclosed = The wrapper command has an unclosed quote or no program name
settings-quick-play = The quick-play target isn't available
settings-memory-min-above-max = Minimum memory can't be above maximum memory
settings-env-line = Line { $line } must be written as NAME=value
settings-concurrency-range = Downloads at once must be between { $min } and { $max }
settings-mirror-line = Line { $line } must be written as official prefix => mirror prefix
settings-mirror-prefixes = Both prefixes on line { $line } must be filled in
# Stand-ins for field names in validation messages, as in "Minimum memory needs a whole number".
settings-field-min-memory = Minimum memory
settings-field-max-memory = Maximum memory
settings-field-window-width = Window width
settings-field-window-height = Window height

# Action messages triggered by the Settings page.
settings-java-pick = Choose Java
settings-java-added = Added Java { $version }
settings-java-not-found = No Java was found here; choose a Java program or JDK folder
settings-java-add-failed = Couldn't add Java
settings-reclaim-failed = Couldn't check for game files
settings-reclaim-none = No unused game files
settings-cleaned = Cleaned up { $size }
settings-clean-files-failed = Couldn't clean up game files
settings-clear-cache-failed = Couldn't clear the cache
settings-diagnostics-saved = Diagnostics saved to { $path }
settings-diagnostics-failed = Couldn't export diagnostics
settings-java-install-started = Downloading Java; progress is in Activity
settings-java-installed = Installed Java { $version }
settings-java-install-failed = Didn't install Java
