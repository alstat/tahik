//! HUD in a warm "carved wood and brass" style with hand-drawn icons:
//! resource bar, clock & wind, villages, families, an Anno-style build bar,
//! family work panel, market, minimap, world labels and overlays.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::boat::{sail_efficiency, BoatState};
use crate::common::*;
use crate::families::{set_task, Family, Selected, Task, TASKS};
use crate::fishing::{Activity, ActivityKind, Hints};
use crate::icons::Icons;
use crate::market::{MarketAction, MarketClicks, MarketUi, Prices};
use crate::prayer::Prayer;
use crate::village::{BuildMode, Communities, Construction, Kind, Worksite, BUILD_KINDS, CATEGORIES};
use crate::weather::{Clock, Weather};
use crate::world::{LAND_CENTER, LAND_RADIUS, MARKET_DOCK, SHOALS};

pub const GOAL_VILLAGES: usize = 4;
const MAP: f32 = 210.0;

// ------------------------------------------------------------- style ---

const CREAM: Color = Color::srgb(0.98, 0.94, 0.84);
const SAND: Color = Color::srgb(0.99, 0.83, 0.52);
const MUTED: Color = Color::srgb(0.80, 0.73, 0.62);
const PANEL: Color = Color::srgba(0.14, 0.10, 0.07, 0.84);
const PANEL_EDGE: Color = Color::srgba(0.86, 0.66, 0.38, 0.75);
const BTN: Color = Color::srgba(0.30, 0.21, 0.13, 0.96);
const BTN_HOVER: Color = Color::srgba(0.44, 0.31, 0.19, 1.0);
const BTN_SELECTED: Color = Color::srgba(0.80, 0.52, 0.22, 1.0);
const BTN_EDGE: Color = Color::srgba(0.92, 0.72, 0.42, 0.55);

fn panel_style(node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(12.0)),
            ..node
        },
        BackgroundColor(PANEL),
        BorderColor::all(PANEL_EDGE),
        BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.45), Val::Px(0.0), Val::Px(4.0), Val::Px(0.0), Val::Px(14.0)),
    )
}

fn text(s: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont::from_font_size(size),
        TextColor(color),
        TextShadow {
            offset: Vec2::new(1.5, 1.5),
            color: Color::srgba(0.0, 0.0, 0.0, 0.7),
        },
    )
}

fn icon(icons: &Icons, name: &str, size: f32) -> impl Bundle {
    (
        ImageNode::new(icons.get(name)),
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            flex_shrink: 0.0,
            ..default()
        },
    )
}

pub fn kind_icon(k: Kind) -> &'static str {
    match k {
        Kind::House => "house",
        Kind::Walkway => "walkway",
        Kind::RainCatcher => "raincatcher",
        Kind::FishPen => "fishpen",
        Kind::SeaweedFarm => "seaweed",
        Kind::DryingRack => "dryingrack",
        Kind::Langgal => "langgal",
        Kind::FishingGround => "fishing",
        Kind::DiveSite => "dive",
        Kind::Woodcutter => "woodcutter",
        Kind::NipaGrove => "nipagrove",
    }
}

fn task_icon(t: Task) -> &'static str {
    match t {
        Task::Rest => "rest",
        Task::FishFood => "fish_food",
        Task::FishSell => "fish_sell",
        Task::Tayum => "tayum",
        Task::Build => "build",
        Task::Water => "fetch_water",
        Task::Trade => "trade",
        Task::Worksite(_) => "work",
    }
}

fn map_pos(p: Vec2) -> Vec2 {
    (p + Vec2::splat(WORLD_HALF)) / (2.0 * WORLD_HALF) * MAP
}

// -------------------------------------------------------- components ---

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HudText {
    Status,
    Boat,
    Villages,
    Log,
    Prompt,
    Build,
    BuildHint,
    FamilyInfo,
    Market,
    Victory,
    Prayer,
    Goal,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum ResText {
    Pesos,
    Timber,
    Nipa,
    People,
    Food,
    Water,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Title,
    Family,
    BuildCards,
    BuildBar,
    Market,
    Victory,
    Help,
    Progress,
    Prayer,
    Boat,
}

#[derive(Component)]
struct ProgressFill;
#[derive(Component)]
struct HudRoot;
#[derive(Component)]
struct CategoryButton(usize);
#[derive(Component)]
struct BuildButton(usize);
#[derive(Component)]
struct FamilyList;
#[derive(Component)]
struct FamilyRow(Entity);
#[derive(Component)]
struct TaskButton(Task);
#[derive(Component)]
struct FamilyLabel(Entity);
#[derive(Component)]
struct SiteLabel(Entity);
#[derive(Component)]
struct WorksiteLabel(Entity);
#[derive(Component)]
struct ButtonLabel;
#[derive(Component)]
struct WindDot(usize);
#[derive(Component)]
struct MapPlayer(usize);
#[derive(Component)]
struct MapShoal(usize);
#[derive(Component)]
struct ShoalLabel(usize);
#[derive(Component)]
struct MarketLabel;

#[derive(Resource, Default)]
pub struct Victory {
    achieved: bool,
    showing: bool,
}

#[derive(Resource, Default)]
struct HelpShown(bool);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Victory>()
            .init_resource::<HelpShown>()
            .add_systems(Startup, spawn_hud)
            .add_systems(
                Update,
                (
                    title_input,
                    overlay_input,
                    click_buttons,
                    update_texts,
                    update_resources,
                    update_buttons,
                    update_panels,
                    update_compass_and_map,
                    update_labels,
                    sync_family_rows,
                    sync_world_labels,
                ),
            );
    }
}

// ------------------------------------------------------------ layout ---

/// A button with an icon and a label, laid out in a row or a column.
fn icon_button(
    commands: &mut Commands,
    icons: &Icons,
    parent: Entity,
    icon_name: &str,
    label: &str,
    size: f32,
    column: bool,
    marker: impl Bundle,
) -> Entity {
    let b = commands
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                flex_direction: if column { FlexDirection::Column } else { FlexDirection::Row },
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(3.0),
                ..default()
            },
            BackgroundColor(BTN),
            BorderColor::all(BTN_EDGE),
            marker,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((icon(icons, icon_name, size), ChildOf(b)));
    commands.spawn((
        text(label, if column { 13.0 } else { 14.0 }, CREAM),
        TextLayout::justify(if column { Justify::Center } else { Justify::Left }),
        ButtonLabel,
        ChildOf(b),
    ));
    b
}

fn spawn_hud(mut commands: Commands, icons: Res<Icons>) {
    let icons = &*icons;
    let root = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        })
        .id();

    // Floating labels over shoals and the market (behind the HUD panels).
    for i in 0..SHOALS.len() {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(260.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ShoalLabel(i),
            ChildOf(root),
            children![(text("", 15.0, CREAM), TextLayout::justify(Justify::Center))],
        ));
    }
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(260.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        MarketLabel,
        ChildOf(root),
        children![(text("Bongao Market", 15.0, SAND), TextLayout::justify(Justify::Center))],
    ));

    let hud = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            HudRoot,
            ChildOf(root),
        ))
        .id();

    // --- Top-left: wind compass, clock, season, weather, prayer.
    let status = commands
        .spawn((
            panel_style(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(14.0),
                padding: UiRect::all(Val::Px(10.0)),
                column_gap: Val::Px(12.0),
                align_items: AlignItems::Center,
                ..default()
            }),
            ChildOf(hud),
        ))
        .id();
    let compass = commands
        .spawn((
            Node {
                width: Val::Px(74.0),
                height: Val::Px(74.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            BackgroundColor(Color::srgba(0.10, 0.30, 0.38, 0.75)),
            ChildOf(status),
        ))
        .id();
    for i in 0..6 {
        let size = if i == 5 { 12.0 } else { 5.0 + i as f32 * 0.6 };
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(size),
                height: Val::Px(size),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            BackgroundColor(if i == 5 { SAND } else { Color::srgba(1.0, 1.0, 1.0, 0.85) }),
            WindDot(i),
            ChildOf(compass),
        ));
    }
    commands.spawn((text("", 14.0, CREAM), HudText::Status, ChildOf(status)));

    // --- Top-centre: resource bar and prayer banner.
    let top = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(14.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
            ChildOf(hud),
        ))
        .id();
    let bar = commands
        .spawn((
            panel_style(Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                column_gap: Val::Px(18.0),
                align_items: AlignItems::Center,
                ..default()
            }),
            ChildOf(top),
        ))
        .id();
    for (name, kind) in [
        ("coin", ResText::Pesos),
        ("timber", ResText::Timber),
        ("nipa", ResText::Nipa),
        ("people", ResText::People),
        ("food", ResText::Food),
        ("water", ResText::Water),
    ] {
        let chip = commands
            .spawn((
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                },
                ChildOf(bar),
            ))
            .id();
        commands.spawn((icon(icons, name, 30.0), ChildOf(chip)));
        commands.spawn((text("0", 17.0, CREAM), kind, ChildOf(chip)));
    }
    commands.spawn((text("", 14.0, SAND), HudText::Goal, ChildOf(bar)));
    let banner = commands
        .spawn((
            panel_style(Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            }),
            Panel::Prayer,
            ChildOf(top),
        ))
        .id();
    commands.spawn((icon(icons, "prayer", 30.0), ChildOf(banner)));
    commands.spawn((text("", 16.0, SAND), HudText::Prayer, ChildOf(banner)));

    // --- Top-right: your boat (captain view) and the villages.
    let right = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(14.0),
                top: Val::Px(14.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                max_width: Val::Px(340.0),
                ..default()
            },
            ChildOf(hud),
        ))
        .id();
    let boat_panel = commands
        .spawn((
            panel_style(Node {
                padding: UiRect::all(Val::Px(10.0)),
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            }),
            Panel::Boat,
            ChildOf(right),
        ))
        .id();
    commands.spawn((icon(icons, "boat", 40.0), ChildOf(boat_panel)));
    commands.spawn((text("", 14.0, CREAM), HudText::Boat, ChildOf(boat_panel)));
    let villages = commands
        .spawn((
            panel_style(Node {
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            }),
            ChildOf(right),
        ))
        .id();
    let head = commands
        .spawn((Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }, ChildOf(villages)))
        .id();
    commands.spawn((icon(icons, "house", 24.0), ChildOf(head)));
    commands.spawn((text("VILLAGES", 15.0, SAND), ChildOf(head)));
    commands.spawn((text("", 13.0, CREAM), HudText::Villages, ChildOf(villages)));

    // --- Left: families.
    let fam_panel = commands
        .spawn((
            panel_style(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(128.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                max_height: Val::Px(440.0),
                max_width: Val::Px(360.0),
                overflow: Overflow::clip(),
                ..default()
            }),
            UiBlocker,
            Interaction::default(),
            FocusPolicy::Block,
            ChildOf(hud),
        ))
        .id();
    let head = commands
        .spawn((Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }, ChildOf(fam_panel)))
        .id();
    commands.spawn((icon(icons, "people", 26.0), ChildOf(head)));
    commands.spawn((text("FAMILIES  (click to give work)", 15.0, SAND), ChildOf(head)));
    commands.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },
        FamilyList,
        ChildOf(fam_panel),
    ));

    // --- Bottom-left: the log.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            bottom: Val::Px(14.0),
            max_width: Val::Px(520.0),
            ..default()
        },
        ChildOf(hud),
        children![(text("", 13.0, CREAM), HudText::Log)],
    ));

    // --- Bottom-centre: family panel, build cards, hints and the build bar.
    let bottom = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
            ChildOf(hud),
        ))
        .id();
    let family_panel = commands
        .spawn((
            panel_style(Node {
                padding: UiRect::all(Val::Px(12.0)),
                max_width: Val::Px(860.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            }),
            Panel::Family,
            UiBlocker,
            Interaction::default(),
            FocusPolicy::Block,
            ChildOf(bottom),
        ))
        .id();
    commands.spawn((text("", 14.0, CREAM), TextLayout::justify(Justify::Center), HudText::FamilyInfo, ChildOf(family_panel)));
    let task_row = commands
        .spawn((
            Node {
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                ..default()
            },
            ChildOf(family_panel),
        ))
        .id();
    for (i, t) in TASKS.iter().enumerate() {
        icon_button(&mut commands, icons, task_row, task_icon(*t), &format!("{}. {}", i + 1, t.label()), 30.0, false, TaskButton(*t));
    }

    let cards = commands
        .spawn((
            panel_style(Node {
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                max_width: Val::Px(900.0),
                ..default()
            }),
            Panel::BuildCards,
            UiBlocker,
            Interaction::default(),
            FocusPolicy::Block,
            ChildOf(bottom),
        ))
        .id();
    let card_row = commands
        .spawn((
            Node {
                column_gap: Val::Px(8.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ChildOf(cards),
        ))
        .id();
    for (i, k) in BUILD_KINDS.iter().enumerate() {
        let c = k.cost();
        let mut parts = Vec::new();
        if *k == Kind::Walkway {
            parts.push("1 timber / 4 m".to_string());
        } else {
            if c.timber > 0 {
                parts.push(format!("{} timber", c.timber));
            }
            if c.nipa > 0 {
                parts.push(format!("{} nipa", c.nipa));
            }
            if c.pesos > 0 {
                parts.push(format!("P{}", c.pesos));
            }
        }
        if parts.is_empty() {
            parts.push("free".into());
        }
        let short = k.short();
        let label = format!("{}{}\n{}", short[..1].to_uppercase(), &short[1..], parts.join(", "));
        let b = icon_button(&mut commands, icons, card_row, kind_icon(*k), &label, 56.0, true, BuildButton(i));
        commands.entity(b).insert(Node {
            width: Val::Px(128.0),
            padding: UiRect::all(Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            ..default()
        });
    }
    commands.spawn((text("", 13.0, MUTED), TextLayout::justify(Justify::Center), HudText::Build, ChildOf(cards)));
    commands.spawn((text("", 16.0, SAND), TextLayout::justify(Justify::Center), HudText::BuildHint, ChildOf(cards)));

    commands.spawn((
        Node {
            width: Val::Px(240.0),
            height: Val::Px(12.0),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        Panel::Progress,
        ChildOf(bottom),
        children![(
            Node {
                width: Val::Percent(0.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(SAND),
            ProgressFill,
        )],
    ));
    commands.spawn((text("", 16.0, SAND), TextLayout::justify(Justify::Center), HudText::Prompt, ChildOf(bottom)));

    let build_bar = commands
        .spawn((
            panel_style(Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            }),
            Panel::BuildBar,
            UiBlocker,
            Interaction::default(),
            FocusPolicy::Block,
            ChildOf(bottom),
        ))
        .id();
    commands.spawn((text("BUILD", 15.0, SAND), ChildOf(build_bar)));
    for (i, (name, _)) in CATEGORIES.iter().enumerate() {
        let icon_name = ["cat_homes", "cat_food", "cat_materials", "cat_water"][i];
        icon_button(&mut commands, icons, build_bar, icon_name, name, 34.0, false, CategoryButton(i));
    }

    // --- Bottom-right: minimap.
    let map = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(14.0),
                bottom: Val::Px(14.0),
                width: Val::Px(MAP),
                height: Val::Px(MAP),
                border: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.28, 0.42, 0.9)),
            BorderColor::all(PANEL_EDGE),
            BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.45), Val::Px(0.0), Val::Px(4.0), Val::Px(0.0), Val::Px(14.0)),
            ChildOf(hud),
        ))
        .id();
    let land = map_pos(LAND_CENTER);
    let lr = LAND_RADIUS * 0.62 / (2.0 * WORLD_HALF) * MAP;
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(land.x - lr),
            top: Val::Px(land.y - lr),
            width: Val::Px(lr * 2.0),
            height: Val::Px(lr * 2.0),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Percent(50.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.42, 0.64, 0.35)),
        BorderColor::all(Color::srgb(0.95, 0.88, 0.66)),
        ChildOf(map),
    ));
    let m = map_pos(MARKET_DOCK);
    commands.spawn((ImageNode::new(icons.get("coin")), Node {
        position_type: PositionType::Absolute,
        left: Val::Px(m.x - 7.0),
        top: Val::Px(m.y - 7.0),
        width: Val::Px(14.0),
        height: Val::Px(14.0),
        ..default()
    }, ChildOf(map)));
    for (i, s) in SHOALS.iter().enumerate() {
        let c = map_pos(s.center);
        let r = s.radius / (2.0 * WORLD_HALF) * MAP;
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(c.x - r),
                top: Val::Px(c.y - r),
                width: Val::Px(r * 2.0),
                height: Val::Px(r * 2.0),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.45, 0.92, 0.85, 0.8)),
            MapShoal(i),
            ChildOf(map),
        ));
    }
    for i in 0..3 {
        let size = if i == 0 { 9.0 } else { 4.0 };
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(size),
                height: Val::Px(size),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(1.0, 0.95, 0.3)),
            MapPlayer(i),
            ChildOf(map),
        ));
    }

    // --- Centre overlays.
    let centre = |commands: &mut Commands, panel: Panel, width: f32| {
        let wrapper = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                panel,
                ChildOf(root),
            ))
            .id();
        commands
            .spawn((
                panel_style(Node {
                    width: Val::Px(width),
                    padding: UiRect::all(Val::Px(24.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                }),
                ChildOf(wrapper),
            ))
            .id()
    };

    let title = centre(&mut commands, Panel::Title, 760.0);
    let deco = commands
        .spawn((Node { column_gap: Val::Px(14.0), align_items: AlignItems::Center, ..default() }, ChildOf(title)))
        .id();
    for name in ["boat", "house", "fish", "langgal", "nipa"] {
        commands.spawn((icon(icons, name, 56.0), ChildOf(deco)));
    }
    commands.spawn((text("TAHIK", 64.0, SAND), ChildOf(title)));
    commands.spawn((text("A tale of the Sama Dilaut, the sea nomads of the Sulu Sea", 20.0, CREAM), ChildOf(title)));
    commands.spawn((
        text(
            "Your people live on the tahik - the sea. Two Sama families have anchored their boats at \
             Sitangkai. Mark fishing grounds and camps from the build bar and your families work them \
             by themselves; give them tasks - building, fetching water, trading - and watch them sail \
             off to do it. Raise stilt villages of timber and nipa on the shallow turquoise shoals, \
             pray together when the azan calls, and trade with the land folk at Bongao.\n\n\
             GOAL: build 4 thriving villages (3+ families, 50%+ approval) across the shoals.",
            16.0,
            CREAM,
        ),
        ChildOf(title),
    ));
    commands.spawn((text(CONTROLS, 13.0, MUTED), ChildOf(title)));
    commands.spawn((text("Press ENTER or SPACE to set sail", 22.0, SAND), ChildOf(title)));

    let market = centre(&mut commands, Panel::Market, 580.0);
    commands.entity(market).insert((UiBlocker, Interaction::default(), FocusPolicy::Block));
    let head = commands
        .spawn((Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() }, ChildOf(market)))
        .id();
    commands.spawn((icon(icons, "coin", 40.0), ChildOf(head)));
    commands.spawn((text("", 15.0, CREAM), HudText::Market, ChildOf(head)));
    for (action, name) in [
        (MarketAction::Sell, "sell"),
        (MarketAction::Wood, "timber"),
        (MarketAction::Water, "water"),
        (MarketAction::Repair, "repair"),
        (MarketAction::Upgrade, "upgrade"),
        (MarketAction::Leave, "leave"),
    ] {
        icon_button(&mut commands, icons, market, name, "", 30.0, false, action);
    }

    let victory = centre(&mut commands, Panel::Victory, 600.0);
    commands.spawn((icon(icons, "langgal", 72.0), ChildOf(victory)));
    commands.spawn((text("", 18.0, CREAM), HudText::Victory, ChildOf(victory)));

    let help = centre(&mut commands, Panel::Help, 700.0);
    commands.spawn((text("How to play", 26.0, SAND), ChildOf(help)));
    commands.spawn((text(CONTROLS, 14.0, CREAM), ChildOf(help)));
    commands.spawn((text(TIPS, 13.0, MUTED), ChildOf(help)));
    commands.spawn((text("H to close", 16.0, SAND), ChildOf(help)));
}

const CONTROLS: &str = "OVERSEER VIEW\n\
WASD / arrows  pan      Q / E  rotate      Mouse wheel  zoom      Right-drag  tilt\n\
Build bar (bottom): pick a category, then a building; left-click (or Enter) to place it\n\
Click a family (on the water or in the list) and give it work - buttons or keys 1-7\n\
T  game speed x1 / x2 / x4      H  help      B  build      Esc  close / deselect\n\
Tab  CAPTAIN VIEW: steer your own boat (W/S paddle, A/D steer, Space sail, F net, E dive, G trade)";

const TIPS: &str = "- Food tab: place a Fishing ground in deep blue water. An idle family fishes it by itself,\n\
  bringing fish home while the village needs it and selling the rest at Bongao.\n\
- Materials tab: a Woodcutter camp and Nipa gatherers on Bongao's shore supply timber and\n\
  leaves for houses (timber frame, nipa roof). Builders buy anything missing at the market.\n\
- Put a family on 'Build & repair' and place blueprints (Homes tab) on the turquoise shallows.\n\
- Families on 'Rest' are automatically given unstaffed work areas.\n\
- Five times a day the azan calls: families stop work and gather at the langgal to pray.\n\
- Every family eats fish and drinks water. Rain catchers fill in the wet season;\n\
  'Fetch water' buys water at Bongao (vital in the dry season).\n\
- New families sail in when a village has empty houses and 50%+ approval.\n\
- In storms families shelter at home; builders repair the damage afterwards.";

// ------------------------------------------------------------- input ---

fn title_input(
    mut started: Local<bool>,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut log: ResMut<GameLog>,
) {
    let pressed = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space);
    if *state.get() == GameState::Title && !*started && (pressed || std::env::var("TAHIK_AUTOSTART").is_ok()) {
        *started = true;
        next.set(GameState::Playing);
        log.push("Two Sama families have anchored at Sitangkai, living on their boats.");
        log.push("Open 'Food' in the build bar and place a Fishing ground in deep water - a family will fish it.");
        log.push("Then click the other family and choose 'Build & repair', and place a stilt house (Homes).");
    }
}

fn overlay_input(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut help: ResMut<HelpShown>,
    mut victory: ResMut<Victory>,
    communities: Res<Communities>,
    mut vtime: ResMut<Time<Virtual>>,
    mut log: ResMut<GameLog>,
) {
    if *state.get() != GameState::Playing {
        return;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        let next = match vtime.relative_speed() {
            s if s < 1.5 => 2.0,
            s if s < 3.0 => 4.0,
            _ => 1.0,
        };
        vtime.set_relative_speed(next);
        log.push(format!("Game speed x{next:.0}"));
    }
    if keys.just_pressed(KeyCode::KeyH) {
        help.0 = !help.0;
    }
    if !victory.achieved && communities.thriving() >= GOAL_VILLAGES {
        victory.achieved = true;
        victory.showing = true;
    }
    if victory.showing && keys.just_pressed(KeyCode::Enter) {
        victory.showing = false;
    }
}

#[allow(clippy::type_complexity)]
fn click_buttons(
    mut build: ResMut<BuildMode>,
    mut clicks: ResMut<MarketClicks>,
    mut selected: ResMut<Selected>,
    mut goto: ResMut<CameraGoto>,
    mut families: Query<&mut Family>,
    buttons: Query<
        (
            &Interaction,
            Option<&MarketAction>,
            Option<&BuildButton>,
            Option<&TaskButton>,
            Option<&FamilyRow>,
            Option<&CategoryButton>,
        ),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, action, build_button, task, row, cat) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(a) = action {
            clicks.0.push(*a);
        }
        if let Some(c) = cat {
            if build.category == Some(c.0) {
                build.category = None;
                build.active = false;
            } else {
                build.category = Some(c.0);
                build.active = false;
            }
            build.walk_start = None;
        }
        if let Some(b) = build_button {
            build.selected = b.0;
            build.active = true;
            build.walk_start = None;
        }
        if let Some(t) = task
            && let Some(e) = selected.0
            && let Ok(mut f) = families.get_mut(e)
        {
            set_task(&mut f, t.0);
        }
        if let Some(r) = row {
            selected.0 = Some(r.0);
            if let Ok(f) = families.get(r.0) {
                goto.0 = Some(f.pos);
            }
        }
    }
}

// ----------------------------------------------------------- updates ---

fn bar(v: f32, max: f32) -> String {
    let n = ((v / max.max(1.0)) * 8.0).round().clamp(0.0, 8.0) as usize;
    format!("{}{}", "#".repeat(n), ".".repeat(8 - n))
}

fn update_resources(
    inv: Res<Inventory>,
    communities: Res<Communities>,
    families: Query<&Family>,
    mut texts: Query<(&ResText, &mut Text)>,
) {
    let food: f32 = communities.list.iter().flatten().map(|c| c.food).sum();
    let water: f32 = communities.list.iter().flatten().map(|c| c.water).sum();
    let fams = families.iter().filter(|f| !f.leaving).count();
    for (kind, mut t) in &mut texts {
        let s = match kind {
            ResText::Pesos => format!("P{}", inv.pesos),
            ResText::Timber => inv.wood.to_string(),
            ResText::Nipa => inv.nipa.to_string(),
            ResText::People => format!("{fams} fam"),
            ResText::Food => format!("{food:.0}"),
            ResText::Water => format!("{water:.0}"),
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_texts(
    clock: Res<Clock>,
    weather: Res<Weather>,
    boat: Res<BoatState>,
    inv: Res<Inventory>,
    communities: Res<Communities>,
    log: Res<GameLog>,
    hints: Res<Hints>,
    activity: Res<Activity>,
    build: Res<BuildMode>,
    prices: Res<Prices>,
    victory: Res<Victory>,
    selected: Res<Selected>,
    families: Query<&Family>,
    (mode, vtime, prayer): (Res<ViewMode>, Res<Time<Virtual>>, Res<Prayer>),
    worksites: Query<&Worksite>,
    mut texts: Query<(&HudText, &mut Text)>,
) {
    let stats = boat.tier.stats();
    for (kind, mut t) in &mut texts {
        let s = match kind {
            HudText::Status => {
                let (next, hours) = prayer.next(clock.time);
                let extra = if *mode == ViewMode::Captain {
                    let rel = wrap_angle(boat.heading - weather.wind_dir);
                    format!(
                        "Sail pull {:.0}%{}",
                        sail_efficiency(rel) * 100.0,
                        if sail_efficiency(rel) < 0.05 { " (into the wind!)" } else { "" }
                    )
                } else {
                    format!("Next prayer: {next} in {hours:.1} h")
                };
                format!(
                    "Day {}  {}   x{:.0} (T)\n{}\n{}   Wind {:.0}%\n{}",
                    clock.day,
                    clock.hour_string(),
                    vtime.relative_speed(),
                    clock.season().label(),
                    weather.describe(),
                    weather.wind_strength * 100.0 / 1.4,
                    extra,
                )
            }
            HudText::Goal => format!("Thriving villages {}/{}", communities.thriving(), GOAL_VILLAGES),
            HudText::Prayer => match prayer.active {
                Some(name) => format!("{name} prayer - the azan has called; the village gathers at the langgal"),
                None => String::new(),
            },
            HudText::Boat => format!(
                "Your {}  -  cargo {}/{}\nFish {}  Tayum {}  Seaweed {}\nDried fish {}  Water jars {}\nHull {:.0}%   Sail {}",
                stats.name,
                inv.cargo(),
                stats.capacity,
                inv.fish,
                inv.urchin,
                inv.seaweed,
                inv.dried_fish,
                inv.water_jars,
                boat.hull.max(0.0),
                if boat.sail_up { "UP" } else { "down" }
            ),
            HudText::Villages => {
                let mut out = String::new();
                for c in communities.list.iter().flatten() {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&format!(
                        "{}  -  {} families, {} homes ({})\n  Approval {:.0}% {}\n  Fish {:.0}   Water {:.0}/{:.0}   Goods {:.0}",
                        c.name,
                        c.families,
                        c.capacity,
                        c.mood(),
                        c.approval,
                        bar(c.approval, 100.0),
                        c.food,
                        c.water,
                        c.water_cap,
                        c.dried + c.seaweed,
                    ));
                }
                out
            }
            HudText::Log => log.entries.iter().map(|e| e.text.as_str()).collect::<Vec<_>>().join("\n"),
            HudText::Prompt => {
                let mut lines: Vec<String> = Vec::new();
                match activity.kind {
                    ActivityKind::Fishing => lines.push("Hauling the net...".into()),
                    ActivityKind::Diving => lines.push("Diving for tayum...".into()),
                    ActivityKind::None => {}
                }
                if !build.active && build.category.is_none() {
                    lines.push(hints.0.join("    "));
                }
                lines.join("\n")
            }
            HudText::Build => {
                if build.active {
                    format!("{}: {}", build.kind().name(), build.kind().blurb())
                } else {
                    "Choose a building (or press 1-5).  Esc closes.".into()
                }
            }
            HudText::BuildHint => {
                if build.active {
                    build.reason.clone()
                } else {
                    String::new()
                }
            }
            HudText::FamilyInfo => match selected.0.and_then(|e| families.get(e).ok()) {
                Some(f) => {
                    let village = communities.list[f.community].as_ref().map(|c| c.name).unwrap_or("?");
                    let work = match f.task {
                        Task::Worksite(site) => worksites
                            .get(site)
                            .map(|w| format!("Working the {}", w.kind.short()))
                            .unwrap_or_else(|_| "Work area".into()),
                        t => format!("{} - {}", t.label(), t.blurb()),
                    };
                    format!(
                        "{}'s family of {}{}\nNow: {}   |   Cargo: {}\n{}",
                        f.name,
                        village,
                        if f.leaving { " (leaving)" } else { "" },
                        f.status,
                        f.cargo.describe(),
                        work,
                    )
                }
                None => String::new(),
            },
            HudText::Market => format!(
                "BONGAO MARKET - today's prices\nFish P{}  Tayum P{}  Seaweed P{}  Dried fish P{}\nPurse P{}   Timber {}   Cargo {}/{}",
                prices.fish,
                prices.urchin,
                prices.seaweed,
                prices.dried,
                inv.pesos,
                inv.wood,
                inv.cargo(),
                stats.capacity
            ),
            HudText::Victory => {
                if !victory.showing {
                    continue;
                }
                format!(
                    "The reefs are alive with your people!\n\n\
                     {} villages stand on stilts over the shallows, home to {} Sama.\n\
                     Walkways ring with children's feet, rain jars are full, and the \
                     smoke of drying fish drifts over the tahik.\n\n\
                     Press ENTER to keep sailing.",
                    communities.founded(),
                    communities.people()
                )
            }
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_buttons(
    build: Res<BuildMode>,
    boat: Res<BoatState>,
    inv: Res<Inventory>,
    prices: Res<Prices>,
    selection: Res<Selected>,
    families: Query<&Family>,
    mut buttons: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut Node,
            Option<&MarketAction>,
            Option<&BuildButton>,
            Option<&TaskButton>,
            Option<&FamilyRow>,
            Option<&CategoryButton>,
            &Children,
        ),
        With<Button>,
    >,
    mut texts: Query<&mut Text, With<ButtonLabel>>,
    mut hint_color: Query<(&HudText, &mut TextColor)>,
) {
    let sel_family = selection.0.and_then(|e| families.get(e).ok());
    for (interaction, mut bg, mut node, action, build_button, task, row, cat, children) in &mut buttons {
        let selected = build_button.is_some_and(|b| build.active && b.0 == build.selected)
            || task.is_some_and(|t| sel_family.is_some_and(|f| f.task == t.0))
            || row.is_some_and(|r| selection.0 == Some(r.0))
            || cat.is_some_and(|c| build.category == Some(c.0));
        bg.0 = match interaction {
            Interaction::Pressed => SAND,
            _ if selected => BTN_SELECTED,
            Interaction::Hovered => BTN_HOVER,
            Interaction::None => BTN,
        };
        // Only show the cards of the open category.
        if let Some(b) = build_button {
            let show = build.category.is_some_and(|c| CATEGORIES[c].1.contains(&b.0));
            node.display = if show { Display::Flex } else { Display::None };
        }
        let label = if let Some(r) = row {
            families.get(r.0).ok().map(|f| format!("{}  -  {}\n{}", f.name, f.task.label(), f.status))
        } else {
            action.map(|a| match a {
                MarketAction::Sell => format!("[1] Sell all catch & goods  (+P{})", prices.catch_value(&inv)),
                MarketAction::Wood => format!("[2] Buy 10 timber  (P{})", prices.wood10),
                MarketAction::Water => format!("[3] Buy 5 jars of fresh water  (P{})", prices.water5),
                MarketAction::Repair => format!("[4] Repair hull, now {:.0}%  (P{})", boat.hull, Prices::repair_cost(boat.hull)),
                MarketAction::Upgrade => match boat.tier.next() {
                    Some(n) => {
                        let s = n.stats();
                        format!("[5] Upgrade to {}  (P{}, cargo {}, faster)", s.name, s.price, s.capacity)
                    }
                    None => "[5] You already sail the finest boat".into(),
                },
                MarketAction::Leave => "[Esc] Leave the market".into(),
            })
        };
        if let Some(label) = label {
            for &c in children {
                if let Ok(mut t) = texts.get_mut(c)
                    && t.0 != label
                {
                    t.0 = label.clone();
                }
            }
        }
    }
    for (kind, mut color) in &mut hint_color {
        if *kind == HudText::BuildHint {
            color.0 = if build.valid { Color::srgb(0.6, 1.0, 0.62) } else { Color::srgb(1.0, 0.58, 0.48) };
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_panels(
    state: Res<State<GameState>>,
    build: Res<BuildMode>,
    market: Res<MarketUi>,
    victory: Res<Victory>,
    help: Res<HelpShown>,
    activity: Res<Activity>,
    selected: Res<Selected>,
    (mode, prayer): (Res<ViewMode>, Res<Prayer>),
    mut panels: Query<(&Panel, &mut Node), Without<ProgressFill>>,
    mut fill: Query<&mut Node, With<ProgressFill>>,
    mut hud: Query<&mut Node, (With<HudRoot>, Without<Panel>, Without<ProgressFill>)>,
) {
    let playing = *state.get() == GameState::Playing;
    for (p, mut node) in &mut panels {
        let show = match p {
            Panel::Title => !playing,
            Panel::Family => {
                playing && *mode == ViewMode::Overseer && selected.0.is_some() && build.category.is_none() && !market.open
            }
            Panel::BuildCards => playing && build.category.is_some(),
            Panel::BuildBar => playing && !market.open,
            Panel::Market => playing && market.open,
            Panel::Victory => playing && victory.showing,
            Panel::Help => playing && help.0,
            Panel::Progress => playing && activity.busy(),
            Panel::Prayer => playing && prayer.active.is_some(),
            Panel::Boat => playing && *mode == ViewMode::Captain,
        };
        node.display = if show { Display::Flex } else { Display::None };
    }
    if let Ok(mut f) = fill.single_mut() {
        f.width = Val::Percent(activity.progress() * 100.0);
    }
    if let Ok(mut h) = hud.single_mut() {
        h.display = if playing { Display::Flex } else { Display::None };
    }
}

#[allow(clippy::too_many_arguments)]
fn update_compass_and_map(
    weather: Res<Weather>,
    boat: Res<BoatState>,
    communities: Res<Communities>,
    camera: Query<&Transform, With<MainCamera>>,
    mut dots: Query<(&WindDot, &mut Node), (Without<MapPlayer>, Without<MapShoal>)>,
    mut players: Query<(&MapPlayer, &mut Node), (Without<WindDot>, Without<MapShoal>)>,
    mut shoals: Query<(&MapShoal, &mut BackgroundColor)>,
) {
    let Ok(cam) = camera.single() else { return };
    let fwd = cam.forward().as_vec3();
    let fwd = Vec2::new(fwd.x, fwd.z).normalize_or_zero();
    let right = Vec2::new(-fwd.y, fwd.x);
    let wind = heading_vec(weather.wind_dir);
    // Screen-space arrow relative to where the camera looks (up = ahead).
    let screen = Vec2::new(wind.dot(right), -wind.dot(fwd));
    for (d, mut node) in &mut dots {
        let t = d.0 as f32 / 5.0 * 2.0 - 1.0;
        let size = if d.0 == 5 { 12.0 } else { 5.0 + d.0 as f32 * 0.6 };
        let p = Vec2::splat(35.0) + screen * t * 26.0 - Vec2::splat(size / 2.0);
        node.left = Val::Px(p.x);
        node.top = Val::Px(p.y);
    }
    let base = map_pos(boat.pos);
    for (m, mut node) in &mut players {
        let p = base + boat.forward() * m.0 as f32 * 5.0 / (2.0 * WORLD_HALF) * MAP * 4.0;
        let size = if m.0 == 0 { 9.0 } else { 4.0 };
        node.left = Val::Px(p.x - size / 2.0);
        node.top = Val::Px(p.y - size / 2.0);
    }
    for (s, mut bg) in &mut shoals {
        bg.0 = match &communities.list[s.0] {
            Some(c) if c.thriving() => Color::srgb(1.0, 0.7, 0.3),
            Some(_) => Color::srgb(0.95, 0.85, 0.5),
            None => Color::srgba(0.45, 0.92, 0.85, 0.8),
        };
    }
}

#[allow(clippy::too_many_arguments)]
fn update_labels(
    state: Res<State<GameState>>,
    communities: Res<Communities>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut labels: Query<(&ShoalLabel, &mut Node, &Children), Without<MarketLabel>>,
    mut market: Query<&mut Node, With<MarketLabel>>,
    mut texts: Query<&mut Text>,
) {
    let Ok((cam, gt)) = camera.single() else { return };
    let playing = *state.get() == GameState::Playing;
    let place = |world: Vec3, node: &mut Node| {
        let dist = gt.translation().distance(world);
        match cam.world_to_viewport(gt, world) {
            Ok(p) if playing && dist < 520.0 && dist > 25.0 => {
                node.display = Display::Flex;
                node.left = Val::Px(p.x - 130.0);
                node.top = Val::Px(p.y);
            }
            _ => node.display = Display::None,
        }
    };
    for (l, mut node, children) in &mut labels {
        let s = &SHOALS[l.0];
        place(Vec3::new(s.center.x, 14.0, s.center.y), &mut node);
        let label = match &communities.list[l.0] {
            Some(c) => format!("{}\n{} families - {:.0}%", s.name, c.families, c.approval),
            None => format!("{}\n(open shallows)", s.name),
        };
        if let Some(&child) = children.first()
            && let Ok(mut t) = texts.get_mut(child)
            && t.0 != label
        {
            t.0 = label;
        }
    }
    if let Ok(mut node) = market.single_mut() {
        place(Vec3::new(MARKET_DOCK.x - 20.0, 12.0, MARKET_DOCK.y), &mut node);
    }
}

/// One clickable row (task icon + name + status) per family.
fn sync_family_rows(
    mut commands: Commands,
    icons: Res<Icons>,
    families: Query<(Entity, &Family)>,
    rows: Query<(Entity, &FamilyRow, &Children)>,
    list: Query<Entity, With<FamilyList>>,
    mut images: Query<&mut ImageNode>,
) {
    let Ok(list) = list.single() else { return };
    let mut want: Vec<Entity> = families.iter().filter(|(_, f)| !f.leaving).map(|(e, _)| e).collect();
    want.sort();
    let mut have: Vec<Entity> = rows.iter().map(|(_, r, _)| r.0).collect();
    have.sort();
    if want != have {
        for (e, _, _) in &rows {
            commands.entity(e).despawn();
        }
        for e in want {
            let task = families.get(e).map(|(_, f)| f.task).unwrap_or(Task::Rest);
            let b = icon_button(&mut commands, &icons, list, task_icon(task), "", 30.0, false, FamilyRow(e));
            commands.entity(b).insert(Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            });
        }
        return;
    }
    // Keep each row's icon in step with the family's task.
    for (_, row, children) in &rows {
        let Ok((_, f)) = families.get(row.0) else { continue };
        let want = icons.get(task_icon(f.task));
        if let Some(&c) = children.first()
            && let Ok(mut img) = images.get_mut(c)
            && img.image != want
        {
            img.image = want;
        }
    }
}

/// Floating labels over family boats, building sites and work areas.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_world_labels(
    mut commands: Commands,
    state: Res<State<GameState>>,
    mode: Res<ViewMode>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    families: Query<(Entity, &Family)>,
    sites: Query<(Entity, &Construction, &GlobalTransform)>,
    worksites: Query<(Entity, &Worksite)>,
    mut fam_labels: Query<(Entity, &FamilyLabel, &mut Node, &Children), (Without<SiteLabel>, Without<WorksiteLabel>)>,
    mut site_labels: Query<(Entity, &SiteLabel, &mut Node, &Children), (Without<FamilyLabel>, Without<WorksiteLabel>)>,
    mut ws_labels: Query<(Entity, &WorksiteLabel, &mut Node, &Children), (Without<FamilyLabel>, Without<SiteLabel>)>,
    mut texts: Query<&mut Text>,
    roots: Query<Entity, With<HudRoot>>,
) {
    let Ok(hud) = roots.single() else { return };
    let Ok((cam, gt)) = camera.single() else { return };
    let show = *state.get() == GameState::Playing && *mode == ViewMode::Overseer;
    let place = |world: Vec3, node: &mut Node| {
        let dist = gt.translation().distance(world);
        match cam.world_to_viewport(gt, world) {
            Ok(p) if show && dist < 420.0 => {
                node.display = Display::Flex;
                node.left = Val::Px(p.x - 110.0);
                node.top = Val::Px(p.y - 20.0);
            }
            _ => node.display = Display::None,
        }
    };
    let label_node = || Node {
        position_type: PositionType::Absolute,
        width: Val::Px(220.0),
        justify_content: JustifyContent::Center,
        ..default()
    };
    let mut set_text = |children: &Children, s: String| {
        if let Some(&c) = children.first()
            && let Ok(mut t) = texts.get_mut(c)
            && t.0 != s
        {
            t.0 = s;
        }
    };

    let mut labelled: Vec<Entity> = Vec::new();
    for (le, l, mut node, children) in &mut fam_labels {
        match families.get(l.0) {
            Ok((_, f)) => {
                labelled.push(l.0);
                place(Vec3::new(f.pos.x, 7.5, f.pos.y), &mut node);
                let task = match f.task {
                    Task::Worksite(s) => worksites.get(s).map(|(_, w)| w.kind.short()).unwrap_or("work area"),
                    t => t.label(),
                };
                set_text(children, format!("{}\n{}", f.name, task));
            }
            _ => commands.entity(le).despawn(),
        }
    }
    for (e, _) in &families {
        if !labelled.contains(&e) {
            commands.spawn((
                label_node(),
                FamilyLabel(e),
                ChildOf(hud),
                children![(text("", 13.0, CREAM), TextLayout::justify(Justify::Center))],
            ));
        }
    }

    let mut labelled: Vec<Entity> = Vec::new();
    for (le, l, mut node, children) in &mut site_labels {
        match sites.get(l.0) {
            Ok((_, c, g)) => {
                labelled.push(l.0);
                place(g.translation() + Vec3::new(0.0, 9.0, 0.0), &mut node);
                let mut needs = Vec::new();
                if c.wood_missing() > 0 {
                    needs.push(format!("{} timber", c.wood_missing()));
                }
                if c.nipa_missing() > 0 {
                    needs.push(format!("{} nipa", c.nipa_missing()));
                }
                let s = if needs.is_empty() {
                    format!("{} {:.0}%", c.kind.short(), c.progress * 100.0)
                } else {
                    format!("{} blueprint {:.0}%\nneeds {}", c.kind.short(), c.progress * 100.0, needs.join(", "))
                };
                set_text(children, s);
            }
            Err(_) => commands.entity(le).despawn(),
        }
    }
    for (e, _, _) in &sites {
        if !labelled.contains(&e) {
            commands.spawn((
                label_node(),
                SiteLabel(e),
                ChildOf(hud),
                children![(text("", 13.0, Color::srgb(0.7, 0.9, 1.0)), TextLayout::justify(Justify::Center))],
            ));
        }
    }

    let mut labelled: Vec<Entity> = Vec::new();
    for (le, l, mut node, children) in &mut ws_labels {
        match worksites.get(l.0) {
            Ok((_, w)) => {
                labelled.push(l.0);
                place(Vec3::new(w.pos.x, 6.0, w.pos.y), &mut node);
                let who = w
                    .worker
                    .and_then(|e| families.get(e).ok())
                    .map(|(_, f)| format!("{}'s family", f.name))
                    .unwrap_or_else(|| "no family yet - set one to Rest".into());
                let stock = if w.stock.is_finite() && matches!(w.kind, Kind::FishingGround | Kind::DiveSite) {
                    format!("  ({:.0} left)", w.stock)
                } else {
                    String::new()
                };
                let name = w.kind.name();
                set_text(children, format!("{name}{stock}\n{who}"));
            }
            Err(_) => commands.entity(le).despawn(),
        }
    }
    for (e, _) in &worksites {
        if !labelled.contains(&e) {
            commands.spawn((
                label_node(),
                WorksiteLabel(e),
                ChildOf(hud),
                children![(text("", 13.0, SAND), TextLayout::justify(Justify::Center))],
            ));
        }
    }
}
