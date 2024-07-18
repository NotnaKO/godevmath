use itertools::Itertools;
use log::{debug, trace};
use num_rational::Ratio;
use num_traits::cast::ToPrimitive;
use rand::seq::IteratorRandom;
use rand::Rng;
use rayon::iter::repeatn;
use rayon::prelude::*;

const YEAR_TIME: u32 = 365 * 24 * 60; // in minutes
const UNAVAILABLE_TIME_AFTER_BAD: u32 = 9;
const TIME_TO_B_CACHE: u32 = 30;
const TIME_TO_C_CACHE: u32 = 40;
const RELEASE_COUNT: usize = 30;
const BAD_RELEASE_COUNT: usize = 3;
const MIN_TIME_BETWEEN_RELEASES: u32 = 60;

fn choice_times_to_releases() -> [u32; RELEASE_COUNT] {
    let mut ans = [0; RELEASE_COUNT];
    let mut rng = rand::thread_rng();
    for i in 0..RELEASE_COUNT {
        let mut time;
        loop {
            time = rng.gen_range(0..YEAR_TIME);
            if is_correct_time_to_append(&ans[0..i], time) {
                break;
            }
        }
        ans[i] = time;
    }

    ans
}

fn is_correct_time_to_append(arr: &[u32], time: u32) -> bool {
    arr.iter()
        .all(|&x| time.abs_diff(x) > MIN_TIME_BETWEEN_RELEASES)
}

fn choice_bad_times_to_releases(releases: &[u32; RELEASE_COUNT]) -> [u32; BAD_RELEASE_COUNT] {
    let mut ans = [0; BAD_RELEASE_COUNT];
    releases
        .iter()
        .copied()
        .choose_multiple_fill(&mut rand::thread_rng(), &mut ans);
    ans
}

fn create_releases_times() -> ([u32; RELEASE_COUNT], [u32; BAD_RELEASE_COUNT]) {
    let mut releases = choice_times_to_releases();
    let mut bad_releases = choice_bad_times_to_releases(&releases);
    releases.sort_unstable();
    bad_releases.sort_unstable();
    debug_assert!(bad_releases.iter().dedup().count() == BAD_RELEASE_COUNT);
    (releases, bad_releases)
}

fn experiment() -> u32 {
    let (a_rel, a_bad_rel) = create_releases_times();
    let (b_rel, b_bad_rel) = create_releases_times();
    let (c_rel, c_bad_rel) = create_releases_times();
    trace!("A releases: {:?}, bad: {:?}", &a_rel, &a_bad_rel);
    trace!("B releases: {:?}, bad: {:?}", &b_rel, &b_bad_rel);
    trace!("C releases: {:?}, bad: {:?}", &c_rel, &c_bad_rel);

    let mut time;
    let mut b_cache_full_in = Some(TIME_TO_B_CACHE);
    let mut c_cache_full_in = Some(TIME_TO_C_CACHE);
    let mut downs = [(0, 0); 3 * BAD_RELEASE_COUNT];
    let mut downs_count = 0;
    let mut a_available_in = None;
    let mut b_available_in = None;
    let mut c_available_in = None;

    let (mut a, mut a_bad) = (
        a_rel.into_iter().peekable(),
        a_bad_rel.into_iter().peekable(),
    );
    let (mut b, mut b_bad) = (
        b_rel.into_iter().peekable(),
        b_bad_rel.into_iter().peekable(),
    );
    let (mut c, mut c_bad) = (
        c_rel.into_iter().peekable(),
        c_bad_rel.into_iter().peekable(),
    );

    debug!("Start simulation");
    loop {
        let available_before = is_now_available(
            a_available_in,
            b_available_in,
            c_available_in,
            b_cache_full_in,
            c_cache_full_in,
        );

        let events = [
            a.peek().copied(),
            b.peek().copied(),
            c.peek().copied(),
            a_bad.peek().copied(),
            b_bad.peek().copied(),
            c_bad.peek().copied(),
            b_cache_full_in,
            c_cache_full_in,
            a_available_in,
            b_available_in,
            c_available_in,
        ];
        time = match events.iter().filter_map(|x| *x).min() {
            Some(val) => val,
            // end of the actions
            None => {
                debug!("End of the simulation");
                break;
            }
        };

        // common releases
        for it in [&mut a, &mut b, &mut c] {
            if it.peek().copied() == Some(time) {
                it.next();
            }
        }

        // cache full time and unavailable time
        for item in [
            &mut b_cache_full_in,
            &mut c_cache_full_in,
            &mut a_available_in,
            &mut b_available_in,
            &mut c_available_in,
        ] {
            if *item == Some(time) {
                *item = None;
            }
        }

        // bad releases
        if a_bad.peek().copied() == Some(time) {
            trace!("A bad release at {}", time);
            a_bad.next();
            a_available_in = Some(time + UNAVAILABLE_TIME_AFTER_BAD);
            b_cache_full_in = Some(time + TIME_TO_B_CACHE);
            c_cache_full_in = Some(time + TIME_TO_C_CACHE);
        }
        if b_bad.peek().copied() == Some(time) {
            trace!("B bad release at {}", time);
            b_bad.next();
            b_available_in = Some(time + UNAVAILABLE_TIME_AFTER_BAD);
        }
        if c_bad.peek().copied() == Some(time) {
            trace!("C bad release at {}", time);
            c_bad.next();
            c_available_in = Some(time + UNAVAILABLE_TIME_AFTER_BAD);
        }

        // analyze downtime
        let available_after = is_now_available(
            a_available_in,
            b_available_in,
            c_available_in,
            b_cache_full_in,
            c_cache_full_in,
        );

        if available_before && !available_after {
            trace!("Down at {}", time);
            downs[downs_count].0 = time;
        } else if !available_before && available_after {
            trace!("Up at {}", time);
            downs[downs_count].1 = time;
            downs_count += 1;
        }
    }

    debug!("Downs: {:?}", &downs[0..downs_count]);
    let down_time = downs.into_iter().map(|(start, end)| end - start).sum();
    debug_assert!(down_time <= 3 * BAD_RELEASE_COUNT as u32 * UNAVAILABLE_TIME_AFTER_BAD);
    debug_assert!(down_time >= BAD_RELEASE_COUNT as u32 * UNAVAILABLE_TIME_AFTER_BAD);
    down_time
}

fn is_now_available(
    a_available_in: Option<u32>,
    b_available_in: Option<u32>,
    c_available_in: Option<u32>,
    b_cache_full_in: Option<u32>,
    c_cache_full_in: Option<u32>,
) -> bool {
    a_available_in.is_none()
        && (b_available_in.is_none() || b_cache_full_in.is_none())
        && (c_available_in.is_none() || c_cache_full_in.is_none())
}

const EXPERIMENT_COUNT: usize = 10_000_000;

fn main() {
    env_logger::init();
    let unavailable_sum: u64 = repeatn((), EXPERIMENT_COUNT)
        .map(|_| experiment() as u64)
        .sum();
    println!("Unavailable sum: {}", unavailable_sum);
    let average_unavailable = Ratio::<u64>::new(unavailable_sum, EXPERIMENT_COUNT as u64);
    println!(
        "Average unavailable time: {} ({})",
        average_unavailable,
        average_unavailable.to_f64().unwrap()
    );
    let percent = Ratio::from_integer(100_u64)
        * (Ratio::from_integer(1_u64)
            - average_unavailable / Ratio::from_integer(YEAR_TIME as u64));
    println!("Percent: {}({})", percent, percent.to_f64().unwrap());
}
