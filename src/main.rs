#![windows_subsystem = "windows"]

use std::{net::UdpSocket, time::Duration};

use bevy::{prelude::*, time::common_conditions::on_timer};

#[derive(Resource, Deref)]
pub struct GameSocket(UdpSocket);

// const SERVER_ADDR: &str = "127.0.0.1:1234";
const SERVER_ADDR: &str = "192.9.161.240:1234";

fn main() {
    let socket = UdpSocket::bind("0.0.0.0:1337").expect("Could not bind udp socket");
    socket
        .connect(SERVER_ADDR)
        .expect("Could not connect to server");
    socket
        .set_nonblocking(true)
        .expect("Could not set socket unblocking");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "VERY FUN GAME".into(),
                resolution: (640., 480.).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .add_startup_system(setup)
        .add_system(apply_velocity.in_schedule(CoreSchedule::FixedUpdate))
        .add_system(move_player.run_if(on_timer(Duration::from_secs_f32(1. / 30.))))
        .add_system(recv_data)
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(FixedTime::new_from_secs(1.0f32 / 60.0f32))
        .insert_resource(GameSocket(socket))
        .run();
}

#[derive(Component, Default, Deref, DerefMut)]
pub struct Vel(Vec2);

#[derive(Component, Default)]
pub struct DeathMsg;

#[derive(Component, Default)]
pub struct WinMsgWrapper;

#[derive(Component, Default)]
pub struct WinMsg;

#[derive(Component, Default)]
pub struct Enemy;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn((
        Player,
        SpriteBundle {
            texture: asset_server.load("player.png"),
            transform: Transform::from_scale(Vec3::splat(30f32 / 128f32)),
            ..default()
        },
        Vel::default(),
    ));

    commands
        .spawn((
            DeathMsg,
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    size: Size::new(Val::Percent(100.), Val::Percent(100.)),
                    ..default()
                },
                visibility: Visibility::Hidden,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn(
                TextBundle::from_section(
                    "You died\nPress R to try again",
                    TextStyle {
                        font: asset_server.load("font.ttf"),
                        font_size: 30.0,
                        color: Color::WHITE,
                    },
                )
                .with_text_alignment(TextAlignment::Center),
            );
        });

    commands
        .spawn((
            WinMsgWrapper,
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    size: Size::new(Val::Percent(100.), Val::Percent(100.)),
                    ..default()
                },
                visibility: Visibility::Hidden,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                WinMsg,
                TextBundle::from_section(
                    "The flag is CTF{}",
                    TextStyle {
                        font: asset_server.load("font.ttf"),
                        font_size: 30.0,
                        color: Color::WHITE,
                    },
                )
                .with_text_alignment(TextAlignment::Center),
            ));
        });

    for _ in 0..200 {
        commands.spawn((
            Enemy,
            SpriteBundle {
                sprite: Sprite {
                    color: Color::RED,
                    custom_size: Some(Vec2::new(30., 30.0)),
                    ..default()
                },
                visibility: Visibility::Hidden,
                ..default()
            },
            Vel::default(),
        ));
    }
}

#[derive(Component, Default)]
struct Player;

fn move_player(
    mut enemy_query: Query<&mut Visibility, With<Enemy>>,
    death_query: Query<&Visibility, (With<DeathMsg>, Without<Enemy>)>,
    keyboard: Res<Input<KeyCode>>,
    sock: Res<GameSocket>,
) {
    let mut dx = 0f32;
    let mut dy = 0f32;
    let mut restart = 0u8;

    if death_query.single() == Visibility::Hidden {
        if keyboard.pressed(KeyCode::Left) {
            dx -= 1f32;
        }
        if keyboard.pressed(KeyCode::Right) {
            dx += 1f32;
        }
        if keyboard.pressed(KeyCode::Up) {
            dy += 1f32;
        }
        if keyboard.pressed(KeyCode::Down) {
            dy -= 1f32;
        }
    }

    if keyboard.pressed(KeyCode::R) {
        restart = 1u8;

        for mut enem in enemy_query.iter_mut() {
            *enem = Visibility::Hidden;
        }
    }

    let mut pack = vec![1u8];

    pack.extend_from_slice(&dx.to_be_bytes());
    pack.extend_from_slice(&dy.to_be_bytes());
    pack.push(restart);

    sock.send(&pack).expect("Could not send packet");
}

fn apply_velocity(mut query: Query<(&mut Transform, &Vel)>) {
    for (mut t, v) in query.iter_mut() {
        t.translation += v.extend(0.);
    }
}

fn recv_data(
    mut player_query: Query<(&mut Transform, &mut Vel), With<Player>>,
    mut death_query: Query<&mut Visibility, With<DeathMsg>>,
    mut enemy_query: Query<
        (Entity, &mut Transform, &mut Vel, &mut Visibility),
        (With<Enemy>, Without<Player>, Without<DeathMsg>),
    >,
    mut win_wrapper_query: Query<
        &mut Visibility,
        (With<WinMsgWrapper>, Without<Enemy>, Without<DeathMsg>),
    >,
    mut win_text_query: Query<&mut Text, With<WinMsg>>,
    sock: Res<GameSocket>,
    mut enemies: Local<Vec<Entity>>,
) {
    if enemies.len() == 0 {
        for (e, _, _, _) in enemy_query.iter() {
            enemies.push(e);
        }
    }

    let mut buf = [0; 2048];

    if let Ok((n, _)) = sock.recv_from(&mut buf) {
        if n >= 2048 {
            warn!("Packet too big!");
        }

        match buf[0] {
            1 => {
                let (mut transform, mut vel) = player_query.single_mut();

                transform.translation.x = f32::from_be_bytes((&buf[1..5]).try_into().unwrap());
                transform.translation.y = f32::from_be_bytes((&buf[5..9]).try_into().unwrap());
                vel.x = f32::from_be_bytes((&buf[9..13]).try_into().unwrap());
                vel.y = f32::from_be_bytes((&buf[13..17]).try_into().unwrap());

                // Account for framerate differences
                **vel = **vel * 0.5;

                let dead = buf[17];

                *death_query.single_mut() = if dead != 0 {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };

                let mut j = 0;
                for i in (18..n).step_by(16) {
                    let x = f32::from_be_bytes((&buf[i..(i + 4)]).try_into().unwrap());
                    let y = f32::from_be_bytes((&buf[(i + 4)..(i + 8)]).try_into().unwrap());
                    let vx = f32::from_be_bytes((&buf[(i + 8)..(i + 12)]).try_into().unwrap());
                    let vy = f32::from_be_bytes((&buf[(i + 12)..(i + 16)]).try_into().unwrap());

                    let (_, mut enemy_t, mut enemy_v, mut enemy_vis) =
                        enemy_query.get_mut(enemies[j]).unwrap();
                    enemy_t.translation = Vec2::new(x, y).extend(0.);
                    **enemy_v = Vec2::new(vx, vy) * 0.5;
                    *enemy_vis = Visibility::Visible;

                    j += 1;
                }
            }
            2 => {
                let flag = std::str::from_utf8(&buf[1..n]).unwrap();

                let mut vis = win_wrapper_query.single_mut();
                *vis = Visibility::Visible;

                win_text_query.single_mut().sections[0].value = format!("The flag is {}", flag);
            }
            msg => warn!("Unrecognised server message: {}", msg),
        }
    }
}
