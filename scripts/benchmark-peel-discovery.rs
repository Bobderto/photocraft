use std::hint::black_box;
use std::time::{Duration, Instant};

extern crate self as photocraft_raster;

#[path = "../crates/raster/src/interrupt.rs"]
mod interrupt;
pub use interrupt::{Cancelled, Interrupt};

#[path = "../crates/algo/src/peel_frontier.rs"]
mod peel_frontier;
use peel_frontier::{Connectivity, PeelFrontier};

#[derive(Clone, Copy)]
struct Grid {
    w: usize,
    h: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Grid {
    fn rows(self) -> impl Iterator<Item = usize> {
        (self.y0..self.y1).flat_map(move |y| (self.x0..self.x1).map(move |x| y * self.w + x))
    }

    fn is_hole(self, i: usize) -> bool {
        let (x, y) = (i % self.w, i / self.w);
        (self.x0..self.x1).contains(&x) && (self.y0..self.y1).contains(&y)
    }
}

fn fold(front: &[usize], signature: &mut u64) {
    for &i in front {
        *signature = (*signature ^ i as u64).wrapping_mul(0x100000001b3);
    }
    *signature = (*signature ^ u64::MAX).wrapping_mul(0x100000001b3);
}

fn rescan(grid: Grid, known: &mut [bool], holes: &[bool], connectivity: Connectivity) -> (usize, u64) {
    let (w, h) = (grid.w, grid.h);
    let mut unknown: Vec<_> = if matches!(connectivity, Connectivity::Four) { (0..w * h).filter(|&i| holes[i]).collect() } else { Vec::new() };
    let mut signature = 0u64;
    let mut rings = 0;
    loop {
        let front: Vec<_> = match connectivity {
            Connectivity::Four => unknown
                .iter()
                .copied()
                .filter(|&i| {
                    let (x, y) = (i % w, i / w);
                    (x > 0 && known[i - 1]) || (x + 1 < w && known[i + 1]) || (y > 0 && known[i - w]) || (y + 1 < h && known[i + w])
                })
                .collect(),
            Connectivity::Eight => grid
                .rows()
                .filter(|&i| {
                    if known[i] || !holes[i] {
                        return false;
                    }
                    let (x, y) = ((i % w) as i32, (i / w) as i32);
                    (-1..=1).any(|dy| {
                        (-1..=1).any(|dx| {
                            let (nx, ny) = (x + dx, y + dy);
                            nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && known[ny as usize * w + nx as usize]
                        })
                    })
                })
                .collect(),
        };
        if front.is_empty() {
            break;
        }
        rings += 1;
        fold(&front, &mut signature);
        for i in front {
            known[i] = true;
        }
        unknown.retain(|&i| !known[i]);
    }
    (rings, signature)
}

fn incremental(grid: Grid, known: &mut [bool], holes: &[bool], connectivity: Connectivity) -> (usize, u64) {
    let ctl = &Interrupt::NONE;
    let size = (grid.w, grid.h);
    let mut frontier = match connectivity {
        Connectivity::Four => PeelFrontier::new(size, connectivity, known, (0..known.len()).filter(|&i| holes[i]), ctl),
        Connectivity::Eight => PeelFrontier::new(size, connectivity, known, grid.rows().filter(|&i| holes[i]), ctl),
    }
    .unwrap();
    let mut signature = 0u64;
    let mut rings = 0;
    while !frontier.cells().is_empty() {
        rings += 1;
        fold(frontier.cells(), &mut signature);
        for &i in frontier.cells() {
            known[i] = true;
        }
        frontier.advance(known, |i| holes[i], ctl).unwrap();
    }
    (rings, signature)
}

fn median_ms(mut times: Vec<Duration>) -> f64 {
    times.sort_unstable();
    times[times.len() / 2].as_secs_f64() * 1000.0
}

fn main() {
    for (w, h, hole_w, hole_h) in [(6000, 4000, 128, 96), (6000, 4000, 960, 640), (6000, 4000, 5200, 1), (750, 500, 16, 12), (750, 500, 120, 80)] {
        let x0 = (w - hole_w) / 2;
        let y0 = (h - hole_h) / 2;
        let grid = Grid { w, h, x0, y0, x1: x0 + hole_w, y1: y0 + hole_h };
        let holes: Vec<_> = (0..w * h).map(|i| grid.is_hole(i)).collect();
        let initial: Vec<_> = holes.iter().map(|&hole| !hole).collect();
        for (name, connectivity) in [("four", Connectivity::Four), ("eight", Connectivity::Eight)] {
            let mut old_times = Vec::new();
            let mut new_times = Vec::new();
            let mut result = None;
            for rep in 0..7 {
                let mut reference = None;
                for new_first in [false, true] {
                    let new = new_first ^ (rep % 2 == 1);
                    let mut known = initial.clone();
                    let start = Instant::now();
                    let output = if new { incremental(grid, &mut known, &holes, connectivity) } else { rescan(grid, &mut known, &holes, connectivity) };
                    let time = start.elapsed();
                    if let Some(previous) = reference {
                        assert_eq!(previous, output);
                    }
                    reference = Some(output);
                    black_box(known);
                    if new {
                        new_times.push(time);
                    } else {
                        old_times.push(time);
                    }
                }
                result = reference;
            }
            let (rings, hash) = result.unwrap();
            println!(
                "{w}x{h} hole={hole_w}x{hole_h} neighbours={name} rings={rings} hash={hash:016x} old={:.3}ms new={:.3}ms",
                median_ms(old_times),
                median_ms(new_times)
            );
        }
    }
}
