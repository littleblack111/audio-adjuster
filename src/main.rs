use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use clap::Parser;
use mpris::{DBusError, FindingError, PlaybackStatus, Player, PlayerFinder};

const PLAYER_IDENTITY: &str = "Spotify";
const BROWSER_IDENTITY: &str = "Mozilla zen";

// config
// percentages
const LOWER_VOLUME: u8 = 45;
const NORMAL_VOLUME: u8 = 80;
const VOLUME_TRANSITION: Duration = Duration::from_millis(300);

#[derive(Parser)]
struct Args {
    #[arg(
        short, long
    )]
    daemon: bool,

    #[arg(
        short, long
    )]
    lower: bool,

    #[arg(
        short, long
    )]
    normal: bool,
}

fn get_player() -> Option<Player> {
    find_player(PLAYER_IDENTITY)
}

fn get_browser() -> Option<Player> {
    find_player(BROWSER_IDENTITY)
}

fn find_player(name: &str) -> Option<Player> {
    match PlayerFinder::new()
        .expect("Failed to create PlayerFinder")
        .find_by_name(name)
    {
        Ok(p) => Some(p),
        Err(e) => {
            if let FindingError::DBusError(e) = e {
                panic!("DBus err: {e}")
            } else {
                None
            }
        }
    }
}

fn normalize_volume(volume: f64) -> u8 {
    (volume * 100.0) as u8
}

fn denormal_volume(volume: u8) -> f64 {
    volume as f64 / 100.0
}

fn set_volume(player: &Player, volume: u8) -> Vec<Result<(), DBusError>> {
    let mut res = Vec::new();
    let current_volume = normalize_volume(
        player
            .get_volume()
            .unwrap_or_else(|e| panic!("Failed to retrieve {:#?} volume: {e}", player)),
    );

    if volume == current_volume {
        return res;
    }

    let mut intervel = VOLUME_TRANSITION
        / current_volume
            .abs_diff(volume)
            .into();

    let range: Box<dyn Iterator<Item = u8>> = if current_volume > volume {
        Box::new((volume..=current_volume).rev())
    } else {
        Box::new(current_volume..volume)
    };

    for v in range {
        let t = Instant::now();
        res.push(player.set_volume(denormal_volume(v)));
        intervel = intervel.saturating_sub(t.elapsed());
        if !intervel.is_zero() {
            sleep(intervel);
        }
    }
    res
}

fn is_playing(player: &Player) -> bool {
    player
        .get_playback_status()
        .map(|s| s == PlaybackStatus::Playing)
        .unwrap_or(false)
}

fn daemon() -> ! {
    let mut had_browser = false;
    loop {
        let player = match get_player() {
            Some(p) => p,
            None => {
                sleep(Duration::from_secs(5));
                continue;
            }
        };
        match get_browser() {
            Some(browser) => {
                let is_playing = is_playing(&browser);
                if !had_browser && is_playing {
                    set_lower(&player);
                    had_browser = true;
                } else if had_browser && !is_playing {
                    set_normal(&player);
                    had_browser = false;
                }
            }
            None => {
                if had_browser {
                    set_normal(&player);
                    had_browser = false;
                }
            }
        }

        sleep(Duration::from_secs(1));
    }
}

fn set_lower(player: &Player) {
    set_volume(player, LOWER_VOLUME)
        .into_iter()
        .for_each(|r| r.unwrap_or_else(|e| eprintln!("Failed to set volume: {e}")));
}

fn set_normal(player: &Player) {
    set_volume(player, NORMAL_VOLUME)
        .into_iter()
        .for_each(|r| r.unwrap_or_else(|e| eprintln!("Failed to set volume: {e}")));
}

fn main() {
    let args = Args::parse();

    if args.daemon {
        daemon()
    }

    let player = get_player().unwrap_or_else(|| panic!("Player {PLAYER_IDENTITY} not found"));

    if args.lower {
        set_lower(&player);
        return;
    } else if args.normal {
        set_normal(&player);
        return;
    }

    daemon()
}
