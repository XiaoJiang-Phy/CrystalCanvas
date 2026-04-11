// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;
use std::fs::File;
use std::io::{BufRead, BufReader};

/// $t_{mn}(\mathbf{R})$ hopping matrix element from `wannier90_hr.dat`.
#[derive(Clone, Debug, Serialize)]
pub struct HoppingEntry {
    pub r_vec: [i32; 3],
    pub m: usize,
    pub n: usize,
    pub re: f64,
    pub im: f64,
    pub magnitude: f64,
    pub degeneracy: u32,
}

/// Parsed Wannier90 Hamiltonian: $H = \sum_{\mathbf{R}} t_{mn}(\mathbf{R}) c^\dagger_{m,\mathbf{0}} c_{n,\mathbf{R}}$
#[derive(Clone, Debug, Serialize)]
pub struct WannierHrData {
    pub num_wann: usize,
    pub num_rpts: usize,
    pub degeneracies: Vec<u32>,
    pub hoppings: Vec<HoppingEntry>,
    pub r_shells: Vec<[i32; 3]>,
    pub t_max: f64,
}

const MAX_NUM_WANN: usize = 10_000;
const MAX_NUM_RPTS: usize = 1_000_000;

pub fn parse_wannier_hr(path: &str) -> Result<WannierHrData, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open '{}': {}", path, e))?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines().enumerate();

    let read_line = |iter: &mut std::iter::Enumerate<std::io::Lines<BufReader<File>>>|
        -> Result<(usize, String), String>
    {
        match iter.next() {
            Some((i, Ok(s))) => Ok((i + 1, s)),
            Some((i, Err(e))) => Err(format!("I/O error at line {}: {}", i + 1, e)),
            None => Err("Unexpected EOF".into()),
        }
    };

    let _ = read_line(&mut lines)?;

    let (ln, s) = read_line(&mut lines)?;
    let num_wann: usize = s.trim().parse()
        .map_err(|_| format!("Line {}: invalid num_wann '{}'", ln, s.trim()))?;

    if num_wann == 0 || num_wann > MAX_NUM_WANN {
        return Err(format!("Line {}: num_wann={} out of range [1, {}]", ln, num_wann, MAX_NUM_WANN));
    }

    let (ln, s) = read_line(&mut lines)?;
    let num_rpts: usize = s.trim().parse()
        .map_err(|_| format!("Line {}: invalid num_rpts '{}'", ln, s.trim()))?;
    if num_rpts == 0 || num_rpts > MAX_NUM_RPTS {
        return Err(format!("Line {}: num_rpts={} out of range [1, {}]", ln, num_rpts, MAX_NUM_RPTS));
    }

    let mut degeneracies = Vec::with_capacity(num_rpts);
    while degeneracies.len() < num_rpts {
        let (ln, line) = read_line(&mut lines)?;
        for token in line.split_whitespace() {
            let d: u32 = token.parse()
                .map_err(|_| format!("Line {}: invalid degeneracy '{}'", ln, token))?;
            degeneracies.push(d);
            if degeneracies.len() == num_rpts {
                break;
            }
        }
    }

    let expected = num_rpts.checked_mul(num_wann)
        .and_then(|v| v.checked_mul(num_wann))
        .ok_or_else(|| format!("Overflow: {} × {}² exceeds usize", num_rpts, num_wann))?;

    let mut hoppings = Vec::with_capacity(expected);
    let mut t_max = 0.0_f64;

    for (i, line_result) in &mut lines {
        let ln = i + 1;
        let line = line_result.map_err(|e| format!("I/O error at line {}: {}", ln, e))?;
        if line.trim().is_empty() {
            continue;
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 7 {
            return Err(format!("Line {}: expected 7 columns, got {}", ln, tokens.len()));
        }

        let rx: i32 = tokens[0].parse().map_err(|_| format!("Line {}: invalid Rx", ln))?;
        let ry: i32 = tokens[1].parse().map_err(|_| format!("Line {}: invalid Ry", ln))?;
        let rz: i32 = tokens[2].parse().map_err(|_| format!("Line {}: invalid Rz", ln))?;

        let m1: usize = tokens[3].parse().map_err(|_| format!("Line {}: invalid m", ln))?;
        let n1: usize = tokens[4].parse().map_err(|_| format!("Line {}: invalid n", ln))?;


        if m1 < 1 || m1 > num_wann {
            return Err(format!("Line {}: m={} out of [1, {}]", ln, m1, num_wann));
        }
        if n1 < 1 || n1 > num_wann {
            return Err(format!("Line {}: n={} out of [1, {}]", ln, n1, num_wann));
        }

        let re: f64 = tokens[5].parse().map_err(|_| format!("Line {}: invalid Re(t)", ln))?;
        let im: f64 = tokens[6].parse().map_err(|_| format!("Line {}: invalid Im(t)", ln))?;

        let rpt_idx = hoppings.len() / (num_wann * num_wann);

        if rpt_idx >= degeneracies.len() {
            return Err(format!("Line {}: R-point index {} exceeds degeneracy table size {}", ln, rpt_idx, degeneracies.len()));
        }

        let t_mag = (re * re + im * im).sqrt();
        if t_mag > t_max {
            t_max = t_mag;
        }

        hoppings.push(HoppingEntry {
            r_vec: [rx, ry, rz],
            m: m1 - 1,
            n: n1 - 1,
            re,
            im,
            magnitude: t_mag,
            degeneracy: degeneracies[rpt_idx],
        });
    }

    if hoppings.len() != expected {
        return Err(format!("Expected {} hoppings ({}×{}²), found {}", expected, num_rpts, num_wann, hoppings.len()));
    }


    let mut seen = std::collections::HashSet::with_capacity(num_rpts);
    let mut r_shells = Vec::with_capacity(num_rpts);
    for h in &hoppings {
        if seen.insert(h.r_vec) {
            r_shells.push(h.r_vec);
        }
    }

    r_shells.sort_by(|a, b| {
        let na = (a[0] as i64).pow(2) + (a[1] as i64).pow(2) + (a[2] as i64).pow(2);
        let nb = (b[0] as i64).pow(2) + (b[1] as i64).pow(2) + (b[2] as i64).pow(2);
        na.cmp(&nb)
            .then(a[0].cmp(&b[0]))
            .then(a[1].cmp(&b[1]))
            .then(a[2].cmp(&b[2]))
    });

    Ok(WannierHrData {
        num_wann,
        num_rpts,
        degeneracies,
        hoppings,
        r_shells,
        t_max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wannier_hr_parse_graphene() {

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .join("tests/fixtures/graphene_hr.dat");

        let data = parse_wannier_hr(path.to_str().unwrap()).unwrap();

        assert_eq!(data.num_wann, 1);
        assert_eq!(data.num_rpts, 7);
        assert_eq!(data.degeneracies, vec![1, 1, 1, 1, 3, 1, 1]);
        assert_eq!(data.hoppings.len(), 7);
        assert_eq!(data.r_shells.len(), 7);
        assert_eq!(data.r_shells[0], [0, 0, 0]);
        assert!((data.t_max - 2.7058).abs() < 1e-4, "t_max={}", data.t_max);
        assert_eq!(data.hoppings[3].r_vec, [0, 0, 0]);
        assert!((data.hoppings[3].re - (-0.2814)).abs() < 1e-6);
    }
}
