//! CPU-side ray-sphere intersection for atom picking — linear scan sufficient for ≤1K atoms

#![allow(dead_code)]

/// A ray in 3D space, defined by origin and direction.
#[derive(Debug, Clone)]
pub struct Ray {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
}

/// An atom for picking purposes (position, radius, index).
#[derive(Debug, Clone)]
pub struct PickAtom {
    pub pos: [f32; 3],
    pub radius: f32,
    pub index: usize,
}

/// Result of a successful ray-pick hit.
#[derive(Debug, Clone)]
pub struct HitResult {
    /// Index of the hit atom
    pub index: usize,
    /// Distance from ray origin to the nearest intersection point
    pub distance: f32,
}

/// Pick the nearest atom intersected by a ray.
///
/// Uses a brute-force linear scan with analytic ray-sphere intersection.
/// For ≤1,000 atoms this completes in well under 1ms (TDD Node 2.2).
///
/// Returns `None` if the ray misses all atoms.
pub fn ray_pick(atoms: &[PickAtom], ray: &Ray) -> Option<HitResult> {
    let mut best: Option<HitResult> = None;

    let ro = ray.origin;
    let rd = ray.direction;

    // Normalize direction for correct distance computation
    let rd_len = (rd[0] * rd[0] + rd[1] * rd[1] + rd[2] * rd[2]).sqrt();
    if rd_len < 1e-10 {
        return None;
    }
    let rd_n = [rd[0] / rd_len, rd[1] / rd_len, rd[2] / rd_len];

    for atom in atoms {
        // Vector from ray origin to sphere center
        let oc = [
            ro[0] - atom.pos[0],
            ro[1] - atom.pos[1],
            ro[2] - atom.pos[2],
        ];

        // Solve |ro + t*rd_n - center|² = r²
        // a = dot(rd_n, rd_n) = 1 (normalized)
        let b = 2.0 * (oc[0] * rd_n[0] + oc[1] * rd_n[1] + oc[2] * rd_n[2]);
        let c = oc[0] * oc[0] + oc[1] * oc[1] + oc[2] * oc[2] - atom.radius * atom.radius;
        let discriminant = b * b - 4.0 * c;

        if discriminant < 0.0 {
            continue; // No intersection
        }

        // Nearest intersection (smallest positive t)
        let sqrt_disc = discriminant.sqrt();
        let t0 = (-b - sqrt_disc) * 0.5;
        let t1 = (-b + sqrt_disc) * 0.5;

        let t = if t0 > 0.0 {
            t0
        } else if t1 > 0.0 {
            t1
        } else {
            continue; // Both behind the ray origin
        };

        // Update best hit if this is closer
        match &best {
            None => {
                best = Some(HitResult {
                    index: atom.index,
                    distance: t,
                });
            }
            Some(prev) if t < prev.distance => {
                best = Some(HitResult {
                    index: atom.index,
                    distance: t,
                });
            }
            _ => {}
        }
    }

    best
}
