//! Live Library, Discover and Activity (plan 0007). They render
//! [`LiveModel`] and report [`LiveIntent`]s; the application does the work.

use crate::key::Key;
use gpui::{
    App, ClickEvent, Entity, IntoElement, ObjectFit, StyledImage as _, Window, div, img,
    prelude::*, px,
};
use gpui_component::input::InputState;
use gpui_component::select::{SearchableVec, Select, SelectState};
use gpui_component::{
    Icon, IndexPath, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use lumilio_core::ProjectKind;

use super::ViewState;
use crate::assets::UiIcon;
use crate::cover::Loader;
use crate::home::WorldHint;
use crate::kit::{self, Emit, ViewIntent};
use crate::live::{
    AccountRow, ActivityRow, ActivityState, DiscoverChange, LOADER_CHOICES, LibraryCard,
    LiveHandler, LiveIntent, LiveModel, PAGE_SIZES, PageItem, SORTS, SearchRow, SearchStatus,
    cover_loader, environment_label, page_items, sort_label, tag_label,
};
use crate::theme::{self, ShellColors};

pub const LIBRARY_TABS: [&str; 3] = ["全部游戏", "收藏", "合集"];
/// The tab that shows the person's collections.
pub const COLLECTIONS_TAB: usize = 2;

/// View-state groups of the Library's ordering and loader filter.
pub const LIBRARY_SORT: u8 = 210;
pub const LIBRARY_LOADER: u8 = 211;
pub const SORT_LABELS: [&str; 3] = ["最近游玩", "名称", "创建时间"];

/// The number a loader is remembered by: 1 vanilla, 2 Fabric, 3 Forge,
/// 4 NeoForge, 5 Quilt (0 is every loader).
#[must_use]
pub const fn loader_code(loader: lumilio_core::Loader) -> usize {
    match loader {
        lumilio_core::Loader::Vanilla => 1,
        lumilio_core::Loader::Fabric => 2,
        lumilio_core::Loader::Forge => 3,
        lumilio_core::Loader::NeoForge => 4,
        lumilio_core::Loader::Quilt => 5,
    }
}

/// The loaders the library has, in a steady order, for the filter.
#[must_use]
pub fn present_loaders(cards: &[LibraryCard]) -> Vec<lumilio_core::Loader> {
    const ORDER: [lumilio_core::Loader; 5] = [
        lumilio_core::Loader::Vanilla,
        lumilio_core::Loader::Fabric,
        lumilio_core::Loader::Forge,
        lumilio_core::Loader::NeoForge,
        lumilio_core::Loader::Quilt,
    ];
    ORDER
        .into_iter()
        .filter(|loader| cards.iter().any(|card| card.loader == *loader))
        .collect()
}

/// The cards in the chosen order (0 as given — most recently played, favourites
/// first; 1 by name; 2 newest first) and only those with the loader, if any.
#[must_use]
pub fn arranged(
    mut cards: Vec<&LibraryCard>,
    sort: usize,
    loader: Option<lumilio_core::Loader>,
) -> Vec<&LibraryCard> {
    cards.retain(|card| loader.is_none_or(|loader| card.loader == loader));
    match sort {
        1 => cards.sort_by_key(|card| card.name.to_lowercase()),
        2 => cards.sort_by_key(|card| std::cmp::Reverse(card.created)),
        _ => {}
    }
    cards
}
pub const ACTIVITY_TABS: [&str; 5] = ["全部", "下载", "安装", "更新", "修复"];
pub const DISCOVER_TABS: [&str; 4] = ["整合包", "Mod", "资源包", "光影"];
pub const DISCOVER_KINDS: [ProjectKind; 4] = [
    ProjectKind::Modpack,
    ProjectKind::Mod,
    ProjectKind::ResourcePack,
    ProjectKind::Shader,
];

/// Inputs the live pages own. Created lazily, because they need a window.
pub struct LiveControls {
    pub library_filter: Entity<InputState>,
    pub discover_search: Entity<InputState>,
    pub sort: Entity<SelectState<SearchableVec<String>>>,
    pub page_size: Entity<SelectState<SearchableVec<String>>>,
    pub version: Entity<SelectState<SearchableVec<String>>>,
    /// How many game versions the version list was last filled with.
    pub versions_shown: usize,
    /// The Library's ordering and loader filter.
    pub library_sort: Entity<SelectState<SearchableVec<String>>>,
    pub library_loader: Entity<SelectState<SearchableVec<String>>>,
    /// The loader codes behind the loader list's entries after "全部".
    pub library_loaders: Vec<usize>,
}

/// The text of the "no loader filter" entry.
pub const ALL_LOADERS: &str = "全部加载器";

/// The text of the "no version filter" entry.
pub const ALL_VERSIONS: &str = "全部版本";

impl LiveControls {
    pub fn new(window: &mut Window, cx: &mut gpui::Context<Self>) -> Self {
        Self {
            library_filter: cx.new(|cx| InputState::new(window, cx).placeholder("搜索游戏")),
            discover_search: cx
                .new(|cx| InputState::new(window, cx).placeholder("搜索 Modrinth，回车确认")),
            sort: cx.new(|cx| {
                let labels: Vec<String> = SORTS.iter().map(|s| sort_label(*s).to_owned()).collect();
                SelectState::new(
                    SearchableVec::new(labels),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            page_size: cx.new(|cx| {
                let labels: Vec<String> = PAGE_SIZES.iter().map(u32::to_string).collect();
                let at = PAGE_SIZES.iter().position(|size| *size == 20).unwrap_or(0);
                SelectState::new(
                    SearchableVec::new(labels),
                    Some(IndexPath::new(at)),
                    window,
                    cx,
                )
            }),
            version: cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(vec![ALL_VERSIONS.to_owned()]),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
                .searchable(true)
            }),
            versions_shown: 0,
            library_sort: cx.new(|cx| {
                let labels: Vec<String> = SORT_LABELS.iter().map(|s| (*s).to_owned()).collect();
                SelectState::new(
                    SearchableVec::new(labels),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            library_loader: cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(vec![ALL_LOADERS.to_owned()]),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            library_loaders: Vec::new(),
        }
    }
}

pub type DiscoverChangeHandler = std::rc::Rc<dyn Fn(DiscoverChange, &mut Window, &mut App)>;

pub struct LiveCtx<'a> {
    pub colors: ShellColors,
    pub model: &'a LiveModel,
    pub state: &'a ViewState,
    pub emit: Emit,
    pub handler: LiveHandler,
    /// Applies a change to the Discover query and searches again.
    pub change: DiscoverChangeHandler,
    pub controls: &'a LiveControls,
    pub filter: String,
}

fn send(handler: &LiveHandler, intent: LiveIntent) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| handler(intent.clone(), window, cx)
}

// ── Library ─────────────────────────────────────────────────────────────

/// The card's ⋯ menu (IA `library.md`): everything about a game that is not
/// worth a button of its own.
fn card_menu(card: &LibraryCard, ctx: &LiveCtx) -> Vec<kit::MenuEntry> {
    let go = |label: &'static str, intent: LiveIntent| {
        kit::MenuEntry::new(label, send(&ctx.handler, intent))
    };
    let id = || card.id.clone();
    // ia[library]: 删除游戏 | 卡片 ⋯ 菜单 → 警告弹窗 | 先在库里确认，再打开游戏页执行删除 | L-LIB-06
    let delete = {
        let handler = ctx.handler.clone();
        let (id, name) = (card.id.clone(), card.name.clone());
        kit::MenuEntry::new("删除…", move |window, cx| {
            // Asked here, before the game's page is opened for it.
            let (handler, id) = (handler.clone(), id.clone());
            crate::collections::confirm_game_delete(
                &name,
                move |window, cx| handler(LiveIntent::DeleteGameOf(id.clone()), window, cx),
                window,
                cx,
            );
        })
        .danger()
    };
    // ia[library]: 菜单：开始游戏 | 卡片 ⋯ 菜单 | 与卡片上的「启动」相同 | H-PLAY-01
    // ia[library]: 菜单：打开 | 卡片 ⋯ 菜单 | 游戏页（进历史） | H-INSTANCE-01
    // ia[library]: 设为当前游戏 | 卡片 ⋯ 菜单 | 首页、发现页的安装目标和右下角芯片跟着换 | H-NAV-03
    // ia[library]: 合集：加入 / 移出 | 卡片 ⋯ 菜单「加入合集…」→ 多选列表 | 勾选即加入或移出，可顺手新建；合集是标签，不移动文件 | ARCH User Collections
    // ia[library]: 复制游戏 | 卡片 ⋯ 菜单 → 复制弹窗（名称、是否复制存档） | 先打开游戏页再弹窗，之后同游戏页 | L-LIB-05
    // ia[library]: 在访达中显示 | 卡片 ⋯ 菜单 | 打开游戏目录 | H-INSTANCE-10
    // ia[library]: 导出整合包 | 卡片 ⋯ 菜单 → 导出弹窗 | 先打开游戏页再弹出导出，之后同游戏页 | L-LIB-08、H-INSTANCE-09
    vec![
        go("开始游戏", LiveIntent::Play(id())),
        go("打开", LiveIntent::OpenInstance(id())),
        go("设为当前游戏", LiveIntent::InstallTarget(id())),
        go("加入合集…", LiveIntent::EditCollections(id())),
        go("复制…", LiveIntent::CopyGameOf(id())),
        go("在访达中显示", LiveIntent::RevealGame(id())),
        go("导出整合包…", LiveIntent::ExportPackOf(id())),
        delete,
    ]
}

fn card(index: usize, card: &LibraryCard, ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let open = send(&ctx.handler, LiveIntent::OpenInstance(card.id.clone()));
    let play = send(&ctx.handler, LiveIntent::Play(card.id.clone()));
    let favorite_label = if card.favorite {
        "取消收藏"
    } else {
        "收藏游戏"
    };
    let favorite = {
        let handler = ctx.handler.clone();
        let id = card.id.clone();
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            handler(LiveIntent::ToggleFavorite(id.clone()), window, cx);
        }
    };
    // ia[library]: 打开游戏 | 点卡片 | 游戏页（进历史） | H-INSTANCE-01
    kit::faceplate(("live-card", index), colors)
        .debug_selector(|| format!("live-card-{index}"))
        .on_click(move |_, window, cx| open(window, cx))
        .child(kit::faceplate_head(
            card.name.clone(),
            cover_loader(card.loader).label(),
            colors,
        ))
        .child(kit::display_window(
            kit::faceplate_cover(card.seed, cover_loader(card.loader), card.world, colors),
            colors,
        ))
        .child(
            h_flex().justify_between().items_center().gap(px(8.)).child(
                div()
                    .min_w_0()
                    .font_family(theme::MONO_FONT)
                    .text_size(px(10.))
                    .text_color(colors.muted)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(card.meta.clone()),
            ),
        )
        .child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child(card.played.clone()),
        )
        .child(
            h_flex()
                .gap_2()
                // ia[library]: 直接启动 | 卡片按键行里的「启动」 | 首页启动时刻接管；不打开游戏页 | H-PLAY-01
                .child(
                    kit::action(
                        ("live-play", index),
                        "启动",
                        Some(UiIcon::Play),
                        false,
                        move |window, cx| {
                            cx.stop_propagation();
                            play(window, cx);
                        },
                    )
                    .small()
                    .debug_selector(move || format!("live-play-{index}")),
                )
                // ia[library]: 收藏 / 取消 | 卡片按键行里的星（图标颜色表示状态） | 立即切换；收藏页同步；不打开游戏页 | L-LIB-01
                .child(
                    Key::new(("live-favorite", index))
                        .icon(Icon::new(if card.favorite {
                            UiIcon::StarFilled
                        } else {
                            UiIcon::Star
                        }))
                        .icon_color(if card.favorite {
                            colors.primary
                        } else {
                            colors.muted
                        })
                        .white()
                        .small()
                        .tooltip(favorite_label)
                        .on_click(favorite)
                        .debug_selector(move || format!("live-favorite-{index}")),
                )
                .child(
                    div()
                        .debug_selector(move || format!("live-card-more-{index}"))
                        .child(kit::small_more_menu(
                            ("live-card-more", index),
                            card_menu(card, ctx),
                            colors,
                        )),
                ),
        )
}

/// The Collections tab: one section per collection, each a row of cards.
fn collections_body(ctx: &LiveCtx, needle: &str) -> gpui::AnyElement {
    let colors = ctx.colors;
    if ctx.model.collections.is_empty() {
        return v_flex()
            .w_full()
            .gap_3()
            .child(kit::empty(
                "还没有合集",
                "把游戏按你的方式归类，比如“生存”“服务器”“整合包”",
                colors,
            ))
            .child(h_flex().justify_center().child(kit::action(
                "live-collection-empty-new",
                "新建合集",
                Some(UiIcon::Plus),
                true,
                send(&ctx.handler, LiveIntent::NewCollection),
            )))
            .into_any_element();
    }
    let mut next = 0;
    let mut sections = Vec::new();
    for (section, collection) in ctx.model.collections.iter().enumerate() {
        let cards: Vec<&LibraryCard> = ctx
            .model
            .collection_cards(collection)
            .into_iter()
            .filter(|card| needle.is_empty() || card.name.to_lowercase().contains(needle))
            .collect();
        // ia[library]: 合集：改名 / 删除 | 合集节标题 ⋯ 菜单 | 删除先确认，游戏不受影响 | ARCH User Collections
        let menu = kit::more_menu(
            ("live-collection-more", section),
            vec![
                kit::MenuEntry::new(
                    "改名…",
                    send(
                        &ctx.handler,
                        LiveIntent::RenameCollection(collection.name.clone()),
                    ),
                ),
                kit::MenuEntry::new("删除合集", {
                    let handler = ctx.handler.clone();
                    let name = collection.name.clone();
                    move |window, cx| {
                        let (handler, doomed) = (handler.clone(), name.clone());
                        crate::collections::confirm_delete(
                            &name,
                            move |window, cx| {
                                handler(LiveIntent::DeleteCollection(doomed.clone()), window, cx)
                            },
                            window,
                            cx,
                        );
                    }
                })
                .danger(),
            ],
            colors,
        );
        let body = if cards.is_empty() {
            div()
                .text_sm()
                .text_color(colors.muted)
                .child(if collection.members.is_empty() {
                    "这个合集还是空的，在游戏卡片的 ⋯ 菜单里选“加入合集…”"
                } else {
                    "没有匹配的游戏"
                })
                .into_any_element()
        } else {
            let row = h_flex()
                .w_full()
                .flex_wrap()
                .gap_4()
                .children(cards.iter().map(|item| {
                    let element = card(next, item, ctx);
                    next += 1;
                    element
                }));
            row.into_any_element()
        };
        sections.push(
            v_flex()
                .w_full()
                .gap_3()
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_baseline()
                                .child(
                                    div()
                                        .text_color(colors.foreground)
                                        .child(collection.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.muted)
                                        .child(format!("{} 个游戏", cards.len())),
                                ),
                        )
                        .child(menu),
                )
                .child(body)
                .into_any_element(),
        );
    }
    v_flex()
        .w_full()
        .gap_6()
        .children(sections)
        .into_any_element()
}

pub fn library(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let tab = ctx.state.library_tab.min(LIBRARY_TABS.len() - 1);
    let needle = ctx.filter.to_lowercase();
    let loaders = present_loaders(&ctx.model.library);
    let sort = ctx.state.choice(LIBRARY_SORT, 0).min(SORT_LABELS.len() - 1);
    // The remembered loader, if the library still has one: its place among
    // the chips (0 is every loader).
    let code = ctx.state.choice(LIBRARY_LOADER, 0);
    let loader_choice = loaders
        .iter()
        .position(|loader| loader_code(*loader) == code)
        .map_or(0, |at| at + 1);
    let loader = loader_choice.checked_sub(1).map(|at| loaders[at]);
    let shown: Vec<&LibraryCard> = arranged(
        ctx.model
            .library
            .iter()
            .filter(|card| tab == 0 || card.favorite)
            .filter(|card| needle.is_empty() || card.name.to_lowercase().contains(&needle))
            .collect(),
        sort,
        loader,
    );

    let body = if tab == COLLECTIONS_TAB && !ctx.model.library.is_empty() {
        collections_body(ctx, &needle)
    } else if shown.is_empty() {
        let (title, text) = if !ctx.model.library_loaded {
            ("正在读取…", "")
        } else if ctx.model.library.is_empty() {
            ("还没有游戏", "新建一个，或去发现里装一个整合包")
        } else if tab == 1 && needle.is_empty() {
            ("还没有收藏", "点卡片上的星标，常玩的游戏会出现在这里")
        } else {
            ("没有匹配的游戏", "换个关键词试试")
        };
        kit::empty(title, text, colors).into_any_element()
    } else {
        h_flex()
            .w_full()
            .flex_wrap()
            .gap_4()
            .children(
                shown
                    .iter()
                    .enumerate()
                    .map(|(index, item)| card(index, item, ctx)),
            )
            .into_any_element()
    };

    let mut actions = kit::PageActions::new("live-library-actions");
    if tab == COLLECTIONS_TAB {
        // ia[library]: 合集：新建 | 合集标签 L2「新建合集」 | 弹窗取名；名称不重复 | ARCH User Collections
        actions = actions.secondary(
            kit::action(
                "live-new-collection",
                "新建合集",
                Some(UiIcon::Plus),
                false,
                send(&ctx.handler, LiveIntent::NewCollection),
            )
            .debug_selector(|| "live-new-collection".into()),
        );
    } else {
        // ia[library]: 导入整合包 | L2 次要 → 系统选文件（.mrpack，或 MultiMC/Prism/本启动器备份的 .zip） | 后台导入，进度在动态；完成 toast 并可点“打开” | L-LIB-03、H-INSTALL-04
        actions = actions.secondary(kit::action(
            "live-import",
            "导入整合包",
            Some(UiIcon::Download),
            false,
            send(&ctx.handler, LiveIntent::ImportPack),
        ));
    }
    // ia[library]: 导入其他启动器的游戏 | L2 ⋯ 菜单 → 选文件夹（MultiMC/Prism 实例、.minecraft）→ 有多个时勾选 | 后台导入，复制玩家文件，原文件不动；CurseForge 格式不支持 | L-LIB-03 | ADR 0016
    // ia[library]: 从备份恢复 | L2 ⋯ 菜单 → 选备份 .zip | 后台恢复成新游戏，不覆盖已有的 | — | ADR 0015
    // ia[library]: 打开游戏库文件夹 | L2 ⋯ 菜单 | 在访达中打开所有游戏所在的文件夹 | —
    let actions = actions
        .more(kit::MenuEntry::new(
            "导入其他启动器的游戏…",
            send(&ctx.handler, LiveIntent::ImportGame),
        ))
        .more(kit::MenuEntry::new(
            "从备份恢复…",
            send(&ctx.handler, LiveIntent::RestoreBackup),
        ))
        .more(kit::MenuEntry::new(
            "打开游戏库文件夹",
            send(&ctx.handler, LiveIntent::OpenGamesFolder),
        ))
        // ia[library]: 新建游戏 | L2 主要 → 新建游戏弹窗 | 创建并（可选）立即安装；成功后打开新游戏页 | L-LIB-02、H-INSTALL-01/02
        .primary(kit::action(
            "live-new",
            "新建游戏",
            Some(UiIcon::Plus),
            true,
            send(&ctx.handler, LiveIntent::NewInstance),
        ));

    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            "游戏库",
            format!("{} 个游戏", ctx.model.library.len()),
            actions.render(colors),
            colors,
        ))
        .child(kit::toolbar(
            Some(
                kit::tabs("live-library-tabs", &LIBRARY_TABS, tab, {
                    let emit = ctx.emit.clone();
                    move |index, window, app| emit(ViewIntent::LibraryTab(index), window, app)
                })
                .into_any_element(),
            ),
            // ia[library]: 搜索 | L3 搜索框 | 按名称过滤；空结果“没有匹配的游戏” | H-NAV-04
            Some(kit::search_field(&ctx.controls.library_filter, colors)),
        ))
        .children(
            (tab != COLLECTIONS_TAB && ctx.model.library.len() > 1).then(|| {
                // ia[library]: 排序 / 按加载器筛选 | L4 两个下拉（排序、加载器），与发现页同一种控件 | 记在偏好设置里，下次打开还是这样；库里只有一种加载器时不显示加载器下拉 | H-NAV-04
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_center()
                    .flex_wrap()
                    .child(toolbar_select(
                        "排序方式",
                        &ctx.controls.library_sort,
                        130.,
                        colors,
                    ))
                    .children((loaders.len() > 1).then(|| {
                        toolbar_select("加载器", &ctx.controls.library_loader, 150., colors)
                    }))
            }),
        )
        .child(kit::entrance(body, ("live-library-body", tab)))
}

// ── Discover ────────────────────────────────────────────────────────────

/// The placeholder cover: shown until the real icon arrives, and instead of
/// it when a project has none or the picture cannot be loaded.
fn placeholder(seed: u32, colors: ShellColors) -> gpui::AnyElement {
    const LOADERS: [Loader; 5] = [
        Loader::Fabric,
        Loader::Forge,
        Loader::NeoForge,
        Loader::Quilt,
        Loader::Vanilla,
    ];
    const WORLDS: [WorldHint; 4] = [
        WorldHint::Overworld,
        WorldHint::Underground,
        WorldHint::Redstone,
        WorldHint::Nether,
    ];
    div()
        .size_full()
        .child(crate::cover::element(
            seed,
            LOADERS[seed as usize % LOADERS.len()],
            WORLDS[seed as usize / 5 % WORLDS.len()],
            colors.surface,
            px(0.),
            px(0.),
        ))
        .into_any_element()
}

/// A project's icon at `side` pixels, fetched by address.
pub fn project_icon(
    url: Option<&str>,
    seed: u32,
    side: f32,
    colors: ShellColors,
) -> gpui::AnyElement {
    let frame = div()
        .flex_none()
        .size(px(side))
        .rounded(px(side / 6.))
        .overflow_hidden()
        .bg(colors.surface_subtle);
    match url {
        Some(url) => frame
            .child(
                img(url.to_owned())
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .with_loading(move || placeholder(seed, colors))
                    .with_fallback(move || placeholder(seed, colors)),
            )
            .into_any_element(),
        None => frame.child(placeholder(seed, colors)).into_any_element(),
    }
}

/// One line of a row's spec column: a muted mono legend and its value.
fn spec(legend: &'static str, value: String, colors: ShellColors) -> impl IntoElement {
    h_flex()
        .gap(px(8.))
        .items_baseline()
        .justify_end()
        .font_family(theme::MONO_FONT)
        .child(
            div()
                .text_size(px(10.))
                .text_color(colors.muted)
                .child(legend),
        )
        .child(
            div()
                .min_w(px(52.))
                .text_xs()
                .text_color(colors.foreground)
                .text_right()
                .child(value),
        )
}

/// How many category tags a row shows before folding the rest into `+N`.
const SHOWN_TAGS: usize = 3;

/// The result row's action: install, or — when the target game already has
/// it — a chip saying so, or an update button when a newer version fits.
fn install_control(
    index: usize,
    row: &SearchRow,
    ctx: &LiveCtx,
    install: impl Fn(&mut Window, &mut App) + 'static,
) -> gpui::AnyElement {
    let colors = ctx.colors;
    let Some(have) = ctx.model.installed.get(&row.project_id) else {
        // ia[discover]: 安装（最新兼容版本） | 结果行「安装」 | 后台任务，toast“开始安装”，完成 toast；Mod 先过依赖提示 | H-DISC-04
        return kit::action(
            ("live-install", index),
            "安装",
            Some(UiIcon::Download),
            false,
            install,
        )
        .small()
        .into_any_element();
    };
    // ia[discover]: 已安装状态 | 结果行「已安装」/「更新」 | 目标游戏已有的（按 Modrinth 识别）显示「已安装」；有更新显示「更新」，点了用新版本替换旧文件；详情页主按钮同样变化 | — | 只认 Modrinth 认得的文件
    match &have.update {
        Some(version) => {
            let intent = LiveIntent::UpdateInstalled {
                kind: row.kind,
                project: row.slug.clone(),
                title: row.title.clone(),
                file_name: have.file_name.clone(),
                version_id: version.clone(),
            };
            let handler = ctx.handler.clone();
            kit::action(
                ("live-update", index),
                "更新",
                Some(UiIcon::Refresh),
                false,
                move |window, cx| {
                    cx.stop_propagation();
                    handler(intent.clone(), window, cx);
                },
            )
            .small()
            .debug_selector(move || format!("live-update-{index}"))
            .into_any_element()
        }
        None => div()
            .debug_selector(move || format!("live-installed-{index}"))
            .child(kit::chip("已安装", Some(kit::tone_ok()), colors))
            .into_any_element(),
    }
}

// ia[discover]: 打开项目详情 | 点结果行 | 详情页（进历史） | H-DISC-02
fn result_row(index: usize, row: &SearchRow, ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let open = send(
        &ctx.handler,
        LiveIntent::OpenProject {
            kind: row.kind,
            slug: row.slug.clone(),
        },
    );
    let install = {
        let handler = ctx.handler.clone();
        let intent = LiveIntent::Install {
            kind: row.kind,
            slug: row.slug.clone(),
            title: row.title.clone(),
            version: None,
        };
        move |window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            handler(intent.clone(), window, cx);
        }
    };
    let hidden = row.categories.len().saturating_sub(SHOWN_TAGS);
    let plain = |text: String| kit::tag(text, kit::TagKind::Plain, colors);
    let tags = h_flex()
        .flex_wrap()
        .gap(px(6.))
        .children(
            row.loaders
                .iter()
                .map(|name| kit::tag(tag_label(name), kit::TagKind::Ink, colors)),
        )
        .children(
            row.environment
                .map(|environment| plain(environment_label(environment).to_string())),
        )
        .children(
            row.categories
                .iter()
                .take(SHOWN_TAGS)
                .map(|name| plain(tag_label(name).to_string())),
        )
        .children((hidden > 0).then(|| plain(format!("+{hidden}"))));
    let body = colors.body;

    // A catalogue line: index, framed icon, name and summary, spec column.
    h_flex()
        .id(("live-result", index))
        .w_full()
        .gap_4()
        .py(px(16.))
        .px(px(4.))
        .items_start()
        .border_b_1()
        .border_color(colors.border)
        .cursor_pointer()
        .hover(move |row| row.bg(body.panel))
        .debug_selector(|| format!("live-result-{index}"))
        .on_click(move |_, window, cx| open(window, cx))
        .child(
            div()
                .flex_none()
                .w(px(24.))
                .pt(px(4.))
                .font_family(theme::MONO_FONT)
                .text_xs()
                .text_color(body.orange_text)
                .child(format!("{:02}", index + 1)),
        )
        .child(
            div()
                .flex_none()
                .p(px(3.))
                .rounded(px(5.))
                .border_1()
                .border_color(colors.border)
                .bg(body.display)
                .child(project_icon(row.icon_url.as_deref(), row.seed, 80., colors)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(px(20.))
                        .line_height(px(26.))
                        .font_weight(gpui::FontWeight::LIGHT)
                        .text_color(colors.foreground)
                        .child(row.title.clone()),
                )
                .child(
                    div()
                        .font_family(theme::MONO_FONT)
                        .text_size(px(11.))
                        .text_color(colors.muted)
                        .child(row.author.to_uppercase()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .line_clamp(2)
                        .child(row.summary.clone()),
                )
                .child(div().pt(px(2.)).child(tags)),
        )
        .child(
            v_flex()
                .flex_none()
                .w(px(190.))
                .self_stretch()
                .items_end()
                .justify_between()
                .gap_2()
                .child(install_control(index, row, ctx, install))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(3.))
                        .border_t_1()
                        .border_color(colors.border)
                        .pt(px(6.))
                        .child(spec("DL", row.downloads.clone(), colors))
                        .child(spec("FAV", row.follows.clone(), colors))
                        .children(
                            (!row.updated.is_empty())
                                .then(|| spec("UPD", row.updated.clone(), colors)),
                        ),
                ),
        )
}

// ia[discover]: 分页 | 列表上方的页码键 | 翻页并回到列表顶部的第一行 | H-DISC-01
fn pager(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let pages = ctx.model.pages();
    let current = ctx.model.query.page;
    let number = |page: u32, active: bool| {
        let change = ctx.change.clone();
        let mut button = Key::new(("live-page", page as usize))
            .label((page + 1).to_string())
            .small();
        button = if active {
            button.primary()
        } else {
            button.ghost()
        };
        theme::clickable(button, !active).on_click(move |_: &ClickEvent, window, cx| {
            change(DiscoverChange::Page(page), window, cx)
        })
    };
    h_flex()
        .gap_1()
        .items_center()
        .children(page_items(current, pages).into_iter().map(|item| {
            match item {
                PageItem::Page(page) => number(page, page == current).into_any_element(),
                PageItem::Gap => div()
                    .px_1()
                    .text_color(colors.muted)
                    .child("…")
                    .into_any_element(),
            }
        }))
        .children((pages > 1 && current + 1 < pages).then(|| {
            let change = ctx.change.clone();
            let next = current + 1;
            theme::clickable(
                Key::new("live-page-next")
                    .icon(Icon::new(UiIcon::Next))
                    .ghost()
                    .small(),
                true,
            )
            .on_click(move |_: &ClickEvent, window, cx| {
                change(DiscoverChange::Page(next), window, cx)
            })
        }))
}

// ia[discover]: 筛选 | 右侧丝印编号筛选栏：游戏版本、加载器、类别；清除筛选 | 重新搜索；加载器与类别是带 LED 的选项列表 | H-DISC-01
fn sidebar(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let query = &ctx.model.query;
    let change = |value: DiscoverChange| {
        let change = ctx.change.clone();
        move |window: &mut Window, cx: &mut App| change(value.clone(), window, cx)
    };
    // The filters are a numbered silkscreen column, not a card.
    let mut number = 0;
    let mut section = |title: &'static str, body: gpui::AnyElement| {
        number += 1;
        kit::section_at(number, title, colors, body).into_any_element()
    };

    let version = section(
        "游戏版本",
        Select::new(&ctx.controls.version)
            .small()
            .placeholder("全部版本")
            .into_any_element(),
    );

    let loaders = query.has_loaders().then(|| {
        section(
            "加载器",
            v_flex()
                .border_t_1()
                .border_color(colors.foreground)
                .children(LOADER_CHOICES.iter().map(|name| {
                    kit::led_option(
                        ("live-loader", name.len() + name.as_bytes()[0] as usize),
                        tag_label(name),
                        query.loaders.iter().any(|chosen| chosen == name),
                        colors,
                        change(DiscoverChange::ToggleLoader((*name).to_owned())),
                    )
                }))
                .into_any_element(),
        )
    });

    let categories = ctx.model.filters.categories(query.kind);
    let category_body = if categories.is_empty() {
        div()
            .text_xs()
            .text_color(colors.muted)
            .child(if ctx.model.filters.loaded {
                "这个分类下没有类别"
            } else {
                "正在读取…"
            })
            .into_any_element()
    } else {
        v_flex()
            .border_t_1()
            .border_color(colors.foreground)
            .children(categories.into_iter().enumerate().map(|(index, name)| {
                kit::led_option(
                    ("live-category", index),
                    tag_label(name),
                    query.categories.iter().any(|chosen| chosen == name),
                    colors,
                    change(DiscoverChange::ToggleCategory(name.to_owned())),
                )
            }))
            .into_any_element()
    };
    let categories = section("类别", category_body);

    v_flex()
        .w(px(240.))
        .flex_none()
        .gap_5()
        .child(version)
        .children(loaders)
        .child(categories)
        .children(ctx.model.filters.error.as_ref().map(|message| {
            h_flex()
                .gap_1()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.danger)
                        .child("筛选项没有加载"),
                )
                .child(kit::technical("live-filters-technical", message.clone()).xsmall())
        }))
        .children(query.filtered().then(|| {
            kit::ghost(
                "live-clear-filters",
                "清除筛选",
                change(DiscoverChange::ClearFilters),
            )
        }))
}

pub fn discover(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let query = &ctx.model.query;
    let tab = DISCOVER_KINDS
        .iter()
        .position(|kind| *kind == query.kind)
        .unwrap_or(0);

    let body = match &ctx.model.search {
        SearchStatus::Idle | SearchStatus::Searching if ctx.model.results.is_empty() => {
            kit::empty("正在搜索…", "", colors).into_any_element()
        }
        SearchStatus::Failed(message) => v_flex()
            .items_center()
            .child(kit::empty("连不上 Modrinth", "检查网络后再试一次", colors))
            .child(kit::technical("live-search-technical", message.clone()))
            .into_any_element(),
        SearchStatus::Done { .. } if ctx.model.results.is_empty() => {
            kit::empty("没有结果", "换个关键词或放宽筛选试试", colors).into_any_element()
        }
        _ => v_flex()
            .w_full()
            .border_t_1()
            .border_color(colors.foreground)
            .children(
                ctx.model
                    .results
                    .iter()
                    .enumerate()
                    .map(|(index, row)| result_row(index, row, ctx)),
            )
            .into_any_element(),
    };

    let actions = kit::PageActions::new("live-discover-actions")
        .more(kit::MenuEntry::new("在 Modrinth 中浏览", {
            let url = lumilio_core::browse_page_url(query.kind);
            move |_, cx| cx.open_url(&url)
        }))
        .more(kit::MenuEntry::new(
            "重新搜索",
            send(&ctx.handler, LiveIntent::Search(query.clone())),
        ));

    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            "发现",
            "找到下一次想玩的东西",
            actions.render(colors),
            colors,
        ))
        .child(kit::toolbar(
            Some(
                // ia[discover]: 分类 | L3 标签：整合包 / Mod / 资源包 / 光影 | 切换搜索的项目类型并重新搜索 | H-DISC-01
                kit::tabs("live-discover-tabs", &DISCOVER_TABS, tab, {
                    let change = ctx.change.clone();
                    move |index, window, app| {
                        let kind = DISCOVER_KINDS[index.min(DISCOVER_KINDS.len() - 1)];
                        change(DiscoverChange::Kind(kind), window, app);
                    }
                })
                .into_any_element(),
            ),
            Some(
                // ia[discover]: 搜索 | L3 搜索框（回车确认） | 按关键词搜索 Modrinth | H-DISC-01
                div()
                    .debug_selector(|| "live-discover-search".into())
                    .child(kit::search_field(&ctx.controls.discover_search, colors))
                    .into_any_element(),
            ),
        ))
        .child(
            h_flex()
                .w_full()
                .items_start()
                .gap_4()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_4()
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child(
                                    h_flex()
                                        .gap_3()
                                        .items_center()
                                        // ia[discover]: 排序 / 显示数量 | L4 两个下拉 | 重新搜索 | H-DISC-01
                                        .child(toolbar_select(
                                            "排序方式",
                                            &ctx.controls.sort,
                                            150.,
                                            colors,
                                        ))
                                        .child(toolbar_select(
                                            "显示数量",
                                            &ctx.controls.page_size,
                                            90.,
                                            colors,
                                        )),
                                )
                                .child(pager(ctx)),
                        )
                        .child(kit::entrance(
                            body,
                            ("live-discover-body", query.page as usize),
                        )),
                )
                .child(sidebar(ctx)),
        )
}

fn toolbar_select(
    label: &'static str,
    state: &Entity<SelectState<SearchableVec<String>>>,
    width: f32,
    colors: ShellColors,
) -> impl IntoElement {
    h_flex()
        .gap_2()
        .items_center()
        .child(div().text_sm().text_color(colors.muted).child(label))
        .child(div().w(px(width)).child(Select::new(state).small()))
}

// ── Activity ────────────────────────────────────────────────────────────

/// 「重试」 for a finished task that remembers its input.
// ia[activity]: 失败重试 | 失败或已取消任务行的「重试」 | 用当时的输入重新发起，作为新任务出现，旧条目保留 | — | 升级前的旧记录没有输入，不显示
fn retry_button(
    index: usize,
    row: &ActivityRow,
    handler: &LiveHandler,
) -> Option<impl IntoElement> {
    let action = row.retry.clone()?;
    Some(
        kit::ghost(
            ("live-retry", index),
            "重试",
            send(handler, LiveIntent::RetryTask(action)),
        )
        .debug_selector(move || format!("live-retry-{index}")),
    )
}

/// 「打开」 for a task about a game that is still in the library.
// ia[activity]: 打开结果 | 完成或失败任务行的「打开」 | 任务关联到仍在游戏库里的游戏时跳到游戏页；整合包安装/导入完成后关联到新建的游戏 | —
fn open_button(
    index: usize,
    row: &ActivityRow,
    handler: &LiveHandler,
    opens: bool,
) -> Option<impl IntoElement> {
    let id = row.instance.clone().filter(|_| opens)?;
    Some(
        kit::ghost(
            ("live-open", index),
            "打开",
            send(handler, LiveIntent::OpenInstance(id)),
        )
        .debug_selector(move || format!("live-open-{index}")),
    )
}

/// How fast a running task goes and how long is left, when that is known.
fn speed_line(row: &ActivityRow, colors: ShellColors) -> Option<gpui::Div> {
    let rate = row.rate?;
    let mut parts = vec![crate::live::rate_text(row.unit, rate)];
    if let Some((done, total)) = row.amount
        && let Some(left) = crate::live::eta_text(total.saturating_sub(done), rate)
    {
        parts.push(format!("剩余{left}"));
    }
    Some(
        div()
            .text_xs()
            .text_color(colors.muted)
            .child(parts.join(" · ")),
    )
}

fn activity_row(
    index: usize,
    row: &ActivityRow,
    handler: &LiveHandler,
    colors: ShellColors,
    opens: bool,
) -> gpui::Div {
    let (tone, trail) = match &row.state {
        ActivityState::Running => (
            colors.foreground,
            h_flex()
                .gap_3()
                .items_center()
                // ia[activity]: 看进度 | 任务行右侧进度条和百分比 | 实时进度；速度和剩余时间在有两次读数后出现（下载按字节，安装/修复按文件个数） | —
                .child(match row.fraction {
                    Some(fraction) => h_flex()
                        .gap_3()
                        .items_center()
                        .child(kit::progress(("live-progress", index), fraction, colors))
                        .child(
                            div()
                                .w(px(36.))
                                .text_xs()
                                .text_color(colors.muted)
                                .child(format!("{}%", (fraction * 100.) as u32)),
                        )
                        .into_any_element(),
                    None => kit::chip("进行中", None, colors).into_any_element(),
                })
                .children(speed_line(row, colors))
                // ia[activity]: 取消 | 运行中任务行的「取消」 | 任务停止，状态“已取消” | —
                .children(row.cancel.map(|id| {
                    kit::ghost(
                        ("live-cancel", index),
                        "取消",
                        send(handler, LiveIntent::CancelTask(id)),
                    )
                }))
                .into_any_element(),
        ),
        ActivityState::Done => (
            kit::tone_ok(),
            h_flex()
                .gap_2()
                .items_center()
                .children(open_button(index, row, handler, opens))
                .child(kit::chip("已完成", Some(kit::tone_ok()), colors))
                .into_any_element(),
        ),
        ActivityState::Failed(message) => (
            colors.danger,
            h_flex()
                .gap_2()
                .items_center()
                // ia[activity]: 技术详情 | 失败任务行的技术详情 | 失败原因弹窗 | —
                .child(kit::technical(
                    ("live-task-technical", index),
                    message.clone(),
                ))
                .children(retry_button(index, row, handler))
                .children(open_button(index, row, handler, opens))
                .child(kit::chip("失败", Some(colors.danger), colors))
                .into_any_element(),
        ),
        ActivityState::Cancelled => (
            colors.muted,
            h_flex()
                .gap_2()
                .items_center()
                .children(retry_button(index, row, handler))
                .child(kit::chip("已取消", None, colors))
                .into_any_element(),
        ),
    };
    let detail = row.detail.clone();
    kit::row(
        row.title.clone(),
        detail,
        Some(
            div()
                .size(px(8.))
                .rounded_full()
                .bg(tone)
                .into_any_element(),
        ),
        Some(trail),
        colors,
    )
}

pub fn activity(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let running = ctx.model.active_tasks();
    let tab = ctx.state.activity_tab.min(ACTIVITY_TABS.len() - 1);
    let shown = crate::live::activity_in_tab(&ctx.model.activity, tab);
    let finished = ctx
        .model
        .activity
        .iter()
        .any(|row| row.state != ActivityState::Running);
    let body = if shown.is_empty() {
        if ctx.model.activity.is_empty() {
            kit::empty("还没有动态", "下载和安装会出现在这里", colors).into_any_element()
        } else {
            kit::empty("这一类里没有动态", "", colors).into_any_element()
        }
    } else {
        kit::panel_list(
            shown
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    let opens = row
                        .instance
                        .as_ref()
                        .is_some_and(|id| ctx.model.library.iter().any(|card| &card.id == id));
                    activity_row(index, row, &ctx.handler, colors, opens)
                })
                .collect(),
            colors,
        )
        .into_any_element()
    };
    let actions = kit::PageActions::new("live-activity-actions").secondary(
        // ia[activity]: 清除已完成 | L2 次要「清除已完成」 | 清空结束的条目；没有结束的条目时禁用 | —
        kit::action(
            "live-clear-finished",
            "清除已完成",
            None,
            false,
            send(&ctx.handler, LiveIntent::ClearFinished),
        )
        .disabled(!finished),
    );
    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            "动态",
            match running {
                0 => "现在没有进行中的事情".to_owned(),
                count => format!("{count} 件事正在进行"),
            },
            actions.render(colors),
            colors,
        ))
        .child(kit::toolbar(
            Some(
                // ia[activity]: 分类筛选 | L3 标签：全部 / 下载 / 安装 / 更新 / 修复 | 按任务类别过滤 | —
                kit::tabs("live-activity-tabs", &ACTIVITY_TABS, tab, {
                    let emit = ctx.emit.clone();
                    move |index, window, app| emit(ViewIntent::ActivityTab(index), window, app)
                })
                .into_any_element(),
            ),
            None,
        ))
        .child(kit::entrance(body, ("live-activity-body", tab)))
}

// ── Accounts ────────────────────────────────────────────────────────────

fn account_row(index: usize, row: &AccountRow, ctx: &LiveCtx) -> gpui::Div {
    let colors = ctx.colors;
    // ia[accounts]: 选为当前 | 账户行左侧的单选圆点 | 之后的启动使用该身份 | H-ACC-06
    let select = send(&ctx.handler, LiveIntent::SelectAccount(row.key.clone()));
    let dot = div()
        .flex_none()
        .size(px(16.))
        .rounded_full()
        .border_1()
        .border_color(if row.selected {
            colors.primary
        } else {
            colors.muted.opacity(0.6)
        })
        .flex()
        .items_center()
        .justify_center()
        .when(row.selected, |ring| {
            ring.child(div().size(px(8.)).rounded_full().bg(colors.primary))
        });
    let short = row.uuid.split('-').next().unwrap_or_default().to_owned();
    let detail = format!(
        "离线账户 · {short}{}",
        if row.custom_id {
            " · 自定义 UUID"
        } else {
            ""
        }
    );
    // ia[accounts]: 复制 UUID | 账户行 ⋯ 菜单 | 复制到剪贴板，toast“已复制 UUID” | H-ACC-09
    let copy = {
        let handler = ctx.handler.clone();
        let text = row.uuid.clone();
        move |window: &mut Window, cx: &mut App| {
            handler(
                LiveIntent::CopyText {
                    text: text.clone(),
                    notice: "已复制 UUID".to_owned(),
                },
                window,
                cx,
            )
        }
    };
    // ia[accounts]: 移除 | 账户行 ⋯ 菜单 → 警告弹窗 | 只删身份（Microsoft 账户同时删凭据库里的登录信息），不删游戏和存档；移除当前账户后改用剩下的第一个 | H-ACC-08
    let remove = {
        let handler = ctx.handler.clone();
        let (key, name) = (row.key.clone(), row.name.clone());
        let selected = row.selected;
        let microsoft = row.microsoft;
        move |window: &mut Window, cx: &mut App| {
            let handler = handler.clone();
            let target = key.clone();
            let title = format!("移除账户“{name}”？");
            let description = match (microsoft, selected) {
                (true, true) => {
                    "会忘记这个身份并从系统凭据库删除它的登录信息，不会删除任何游戏或存档。它是当前账户，移除后会改用剩下的第一个。"
                }
                (true, false) => {
                    "会忘记这个身份并从系统凭据库删除它的登录信息，不会删除任何游戏或存档。"
                }
                (false, true) => {
                    "只会忘记这个身份，不会删除任何游戏或存档。它是当前账户，移除后会改用剩下的第一个。"
                }
                (false, false) => "只会忘记这个身份，不会删除任何游戏或存档。",
            };
            window.open_alert_dialog(cx, move |alert, _, _| {
                let handler = handler.clone();
                let target = target.clone();
                alert
                    .title(title.clone())
                    .description(description)
                    .ok_text("移除")
                    .ok_variant(gpui_component::button::ButtonVariant::Danger)
                    .cancel_text("取消")
                    .show_cancel(true)
                    .on_ok(move |_, window, cx| {
                        handler(LiveIntent::RemoveAccount(target.clone()), window, cx);
                        true
                    })
            });
        }
    };
    let mut entries = vec![kit::MenuEntry::new("复制 UUID", copy)];
    if row.microsoft {
        // ia[accounts]: 刷新登录 | Microsoft 账户行 ⋯ 菜单「刷新登录」 | 失效的登录显示“需要重新登录”，刷新后恢复 | H-ACC-07
        entries.push(kit::MenuEntry::new(
            "刷新登录",
            send(&ctx.handler, LiveIntent::RefreshAccount(row.key.clone())),
        ));
    }
    entries.push(kit::MenuEntry::new("移除…", remove).danger());
    let menu = kit::more_menu(("account-more", index), entries, colors);
    h_flex()
        .w_full()
        .items_center()
        .gap_3()
        .py(px(10.))
        .child(
            h_flex()
                .id(("account-choose", index))
                .debug_selector(move || format!("account-choose-{index}"))
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_3()
                .cursor_pointer()
                .on_click(move |_, window, cx| select(window, cx))
                .child(dot)
                .child(kit::avatar(&row.name, 32.))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_sm()
                                .font_medium()
                                .text_color(colors.foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(row.name.clone()),
                        )
                        .child(div().text_xs().text_color(colors.muted).child(detail)),
                ),
        )
        .children(
            row.needs_sign_in
                .then(|| kit::chip("需要重新登录", Some(colors.danger), colors)),
        )
        .children(
            row.selected
                .then(|| kit::chip("当前", Some(colors.primary), colors)),
        )
        .child(menu)
}

pub fn accounts(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let model = ctx.model;
    let subtitle = match model.selected_account() {
        Some(row) => format!(
            "当前：{}（{}）",
            row.name,
            if row.microsoft { "Microsoft" } else { "离线" }
        ),
        None if model.accounts_loaded => "还没有账户，添加一个才能进游戏".to_owned(),
        None => String::new(),
    };
    // ia[accounts]: 添加离线账户 | L2 次要「添加离线账户」→ 弹窗（可自定义 UUID） | 列表新增；UUID 只在添加时可设 | H-ACC-02
    let add = kit::action(
        "account-new",
        "添加离线账户",
        Some(UiIcon::Plus),
        false,
        send(&ctx.handler, LiveIntent::NewAccount),
    );
    // ia[accounts]: Microsoft 登录 | L2 主要「登录 Microsoft」→ 设备码弹窗 | 登录后出现在列表和导航芯片里，启动用真实的玩家名和令牌 | H-ACC-01 | ADR 0013；真实登录要 Mojang 批准应用注册
    let microsoft = kit::action(
        "account-microsoft",
        "登录 Microsoft",
        None,
        true,
        send(&ctx.handler, LiveIntent::MicrosoftSignIn),
    );
    let body = if model.accounts.is_empty() {
        v_flex()
            .items_center()
            .child(kit::empty(
                "还没有账户",
                "用 Microsoft 登录可以进入正版服务器；离线账户不需要登录，名称就是你在游戏里的名字。",
                colors,
            ))
            .into_any_element()
    } else {
        kit::list(
            model
                .accounts
                .iter()
                .enumerate()
                .map(|(index, row)| account_row(index, row, ctx))
                .collect(),
            colors,
        )
        .into_any_element()
    };
    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            "账户",
            subtitle,
            kit::PageActions::new("accounts-actions")
                .secondary(add)
                .primary(microsoft)
                .render(colors),
            colors,
        ))
        .child(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::library_card;
    use lumilio_core::{InstanceRecord, InstanceSettings};

    fn card(id: &str, name: &str, loader: lumilio_core::Loader, created: u64) -> LibraryCard {
        library_card(
            &InstanceRecord {
                id: id.to_owned(),
                name: name.to_owned(),
                game_version: "1.21.1".to_owned(),
                loader,
                loader_version: None,
                favorite: false,
                created_at: created,
                last_played: None,
                play_seconds: 0,
                installed: false,
                settings: InstanceSettings::default(),
            },
            10,
        )
    }

    #[test]
    fn loaders_are_remembered_by_a_steady_number() {
        use lumilio_core::Loader::{Fabric, Forge, NeoForge, Quilt, Vanilla};
        let codes = [Vanilla, Fabric, Forge, NeoForge, Quilt].map(loader_code);
        assert_eq!(codes, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn the_library_is_ordered_and_filtered_by_the_choices() {
        use lumilio_core::Loader::{Fabric, Forge, Vanilla};
        let cards = vec![
            card("a", "Zebra", Fabric, 1),
            card("b", "apple", Forge, 3),
            card("c", "Mango", Fabric, 2),
        ];
        let names = |shown: Vec<&LibraryCard>| -> Vec<String> {
            shown.iter().map(|card| card.name.clone()).collect()
        };
        let all = || cards.iter().collect::<Vec<_>>();
        assert_eq!(names(arranged(all(), 0, None)), ["Zebra", "apple", "Mango"]);
        assert_eq!(names(arranged(all(), 1, None)), ["apple", "Mango", "Zebra"]);
        assert_eq!(names(arranged(all(), 2, None)), ["apple", "Mango", "Zebra"]);
        assert_eq!(names(arranged(all(), 1, Some(Fabric))), ["Mango", "Zebra"]);
        assert!(arranged(all(), 0, Some(Vanilla)).is_empty());
        // Only the loaders the library has, in a steady order.
        assert_eq!(present_loaders(&cards), [Fabric, Forge]);
        assert!(present_loaders(&[]).is_empty());
    }
}
