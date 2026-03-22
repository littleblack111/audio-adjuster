use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use clap::Parser;
use mpris::{DBusError, PlaybackStatus, Player, PlayerFinder};
use unwrap_retry::{RetryableOptionFn, RetryableResultFn};

// Defaults
const DEFAULT_PLAYER_IDENTITY: &str = "Spotify";
const DEFAULT_BROWSER_IDENTITY: &str = "Mozilla zen";

// percentages
const DEFAULT_LOWER_VOLUME: u8 = 45;
const DEFAULT_NORMAL_VOLUME: u8 = 80;
const DEFAULT_VOLUME_TRANSITION: u64 = 300;

const DEFAULT_LOOP_DELAY: u64 = 1000;

#[derive(Parser)]
struct Args {
    #[arg(
        short, long
    )]
    daemon: bool,

    #[arg(long, default_value_t = DEFAULT_LOOP_DELAY)]
    /// For --daemon
    /// in ms
    loop_delay: u64,

    #[arg(
        short, long
    )]
    lower: bool,

    #[arg(
        short, long
    )]
    normal: bool,

    #[arg(long, default_value_t = DEFAULT_LOWER_VOLUME)]
    lower_volume: u8,

    #[arg(long, default_value_t = DEFAULT_NORMAL_VOLUME)]
    normal_volume: u8,

    #[arg(short, long, default_value_t = DEFAULT_VOLUME_TRANSITION)]
    /// in ms
    volume_transition_delay: u64,

    #[arg(
        short,
        long,
        default_value_t = DEFAULT_PLAYER_IDENTITY.to_string()
    )]
    player: String,

    #[arg(
        short,
        long,
        default_value_t = DEFAULT_BROWSER_IDENTITY.to_string()
    )]
    browser: String,
}

fn get_player(id: &str) -> Option<Player> {
    find_player(id)
}

fn get_browser(id: &str) -> Option<Player> {
    find_player(id)
}

fn find_player(name: &str) -> Option<Player> {
    PlayerFinder::new()
        .expect("Failed to create PlayerFinder")
        .find_by_name(name)
        .ok()
}

fn normalize_volume(volume: f64) -> u8 {
    (volume * 100.0) as u8
}

fn denormal_volume(volume: u8) -> f64 {
    volume as f64 / 100.0
}

fn _set_volume(player: &Player, volume: u8, transition: &Duration) -> Vec<Result<(), DBusError>> {
    let mut res = Vec::new();
    let current_volume = normalize_volume((|| player.get_volume()).unwrap_blocking());

    if volume == current_volume {
        return res;
    }

    let mut intervel = *transition
        / current_volume
            .abs_diff(volume)
            .into();

    let range: Box<dyn Iterator<Item = u8>> = if current_volume > volume {
        Box::new((volume..=current_volume).rev())
    } else {
        Box::new(current_volume..=volume)
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

fn daemon(browser: &str, player: &str, normal: u8, lower: u8, transition: &Duration, loop_delay: &Duration) -> ! {
    let mut had_browser = false;
    loop {
        let player = match get_player(player) {
            Some(p) => p,
            None => {
                sleep(Duration::from_secs(5));
                continue;
            }
        };

        match get_browser(browser) {
            Some(browser) => {
                let browser_playing = is_playing(&browser);
                if !had_browser && browser_playing {
                    set_volume(
                        &player, lower, transition,
                    );
                    had_browser = true;
                } else if had_browser && !browser_playing {
                    set_volume(
                        &player, normal, transition,
                    );
                    had_browser = false;
                }
            }
            None => {
                if had_browser {
                    set_volume(
                        &player, normal, transition,
                    );
                    had_browser = false;
                }
            }
        }

        sleep(*loop_delay);
    }
}

fn set_volume(player: &Player, volume: u8, transition: &Duration) {
    _set_volume(
        player, volume, transition,
    )
    .into_iter()
    .for_each(|r| r.unwrap_or_else(|e| eprintln!("Failed to set volume: {e} for player {player:#?}")));
}

fn main() {
    let args = Args::parse();
    let transition_delay = Duration::from_millis(args.volume_transition_delay);
    let daemon = || {
        daemon(
            &args.browser,
            &args.player,
            args.normal_volume,
            args.lower_volume,
            &transition_delay,
            &Duration::from_millis(args.loop_delay),
        )
    };

    if args.daemon {
        daemon()
    }

    let player = (|| get_player(&args.player)).unwrap_blocking();

    if args.lower {
        set_volume(
            &player,
            args.lower_volume,
            &transition_delay,
        );
        return;
    }

    if args.normal {
        set_volume(
            &player,
            args.normal_volume,
            &transition_delay,
        );
        return;
    }

    daemon()
}
