# CrystalCanvas Algorithms & References

> Version: v0.5.0 | Updated: 2026-04-12

Mathematical formulations and implementation details for core algorithms. Each section cross-references the exact source files.

---

## 1. Slab Generation (Diophantine Surface Basis)

**Source**: C++ physics kernel via FFI (`ffi::build_slab_v2`) → Rust state reconstruction in `crystal_state.rs:704`

Many modeling tools use heuristic searches to find in-plane basis vectors for a given Miller index $(h,k,l)$. CrystalCanvas uses a deterministic algebraic approach based on solving linear Diophantine equations to guarantee a **unimodular** transformation matrix ($|\det P| = 1$).

### Formulation

Given Miller index $(h, k, l)$ with $\gcd(h, k, l) = 1$, we construct a transformation matrix $P \in \mathbb{Z}^{3 \times 3}$:

$$
P = \begin{pmatrix}
p_{11} & p_{12} & p_{13} \\
p_{21} & p_{22} & p_{23} \\
h & k & l
\end{pmatrix}, \quad \det(P) = 1
$$

The third row is the Miller index itself, ensuring the new $\mathbf{c}'$ axis is along $(h,k,l)$ in the original basis. The first two rows span the surface plane.

### Algorithm

1. **GCD reduction**: Factor out $\gcd(h,k,l)$.
2. **Extended Euclidean**: Find $x, y$ such that $hx + ky = g_1 = \gcd(h,k)$, then $z$ such that $g_1 z + l w = 1$.
3. **In-plane row construction**: Select the first two rows $(\mathbf{v}_1, \mathbf{v}_2)$ via the cofactor matrix of $(h,k,l)$ such that $\mathbf{v}_1 \times \mathbf{v}_2 = (h,k,l)$.
4. **Metric tensor orthogonalization**: Transform the new lattice $\mathbf{L}' = P \cdot \mathbf{L}$ and apply Gram-Schmidt to enforce $\mathbf{a}' \cdot \mathbf{c}' = \mathbf{b}' \cdot \mathbf{c}' = 0$, yielding $\alpha = \beta = 90°$.
5. **Layer replication**: Tile by $N_\text{layers}$ along $\mathbf{c}'$.
6. **MIC deduplication**: Remove duplicate atoms at slab boundaries using the C++ kernel's `check_overlap_mic()`.
7. **Vacuum injection**: Extend $|\mathbf{c}'|$ by $d_\text{vacuum}$ (Å) along the Cartesian $z$-axis without shearing.

### Constraints

- **Precondition**: The input must be a conventional cell (spacegroup $\neq P1$). The command rejects $P1$ cells since Miller indices are defined relative to conventional axes.
- **Atomistic preservation**: $|\det P| = 1$ guarantees that the slab unit cell contains exactly $N_\text{layers}$ copies of the primitive cell — no spurious atoms or missing sites.

> **Reference**: Sun, W., & Ceder, G. (2013). Efficient creation and convergence of surface slabs. *Surface Science*, 617, 53–59.

---

## 2. Wigner-Seitz Brillouin Zone Construction

**Source**: `brillouin_zone.rs` (3D: `wigner_seitz_cut`, 2D: `wigner_seitz_2d`)

### Formulation

The first Brillouin Zone is the Wigner-Seitz cell of the reciprocal lattice. For reciprocal lattice vectors $\mathbf{b}_i$ derived from real-space vectors $\mathbf{a}_j$ as:

$$
\mathbf{b}_i = \frac{2\pi \, \mathbf{a}_j \times \mathbf{a}_k}{\mathbf{a}_i \cdot (\mathbf{a}_j \times \mathbf{a}_k)}
$$

the BZ is defined as:

$$
\text{BZ} = \left\{ \mathbf{k} \;\middle|\; |\mathbf{k}| \le |\mathbf{k} - \mathbf{G}| \;\; \forall \; \mathbf{G} \neq 0 \right\}
$$

Each condition $|\mathbf{k}| \le |\mathbf{k} - \mathbf{G}|$ defines a half-space: $\mathbf{k} \cdot \hat{\mathbf{G}} \le |\mathbf{G}|/2$.

### 3D Algorithm (Sutherland-Hodgman Convex Clipping)

1. **Reciprocal lattice**: Computed from the real-space lattice via explicit cross products and volume normalization (`compute_reciprocal_lattice`).
2. **Shell generation**: Enumerate $\mathbf{G} = n_1 \mathbf{b}_1 + n_2 \mathbf{b}_2 + n_3 \mathbf{b}_3$ for $(n_1, n_2, n_3) \in [-2, 2]^3 \setminus \{(0,0,0)\}$. Each $\mathbf{G}$ produces a bisecting plane at distance $|\mathbf{G}|/2$ from the origin with normal $\hat{\mathbf{G}}$.
3. **Planes sorted by distance**: Nearest planes are processed first, enabling rapid convergence of the clipping volume.
4. **Convex polyhedron clipping**: Starting from a large initial cube (6 faces), we iteratively clip against each bisecting plane using a 3D Sutherland-Hodgman algorithm. For each plane:
   - Clip all existing polygonal faces against the half-space.
   - Collect new intersection vertices on the cutting plane.
   - Sort new face vertices by polar angle around their centroid (using a local tangent frame $\mathbf{t}_1, \mathbf{t}_2 = \hat{\mathbf{G}} \times \mathbf{t}_1$).
   - Enforce consistent winding order via cross-product orientation check.
5. **Vertex merging**: Final vertices are deduplicated with tolerance $10^{-10}$ Å$^{-2}$ and indexed into a shared vertex buffer for rendering.

### 2D Algorithm

For 2D materials (detected via vacuum gap heuristics in `CrystalState::detect_2d`):

1. Project the in-plane lattice vectors $\mathbf{a}_1, \mathbf{a}_2$ into 2D.
2. Compute 2D reciprocal vectors: $\mathbf{b}_1 = 2\pi (a_{2y}, -a_{2x}) / \det$, $\mathbf{b}_2 = 2\pi (-a_{1y}, a_{1x}) / \det$.
3. Perform 2D Sutherland-Hodgman polygon clipping against bisecting lines for shells $(n_1, n_2) \in [-2, 2]^2$.
4. Deduplicate and sort vertices CCW by polar angle.
5. Embed 2D polygon into 3D by inserting zeros along the vacuum axis.
6. Bravais type classification uses wallpaper group analysis (`kpath_2d::identify_bravais_2d`).

> **Reference**: Setyawan, W., & Curtarolo, S. (2010). High-throughput electronic band structure calculations: Challenges and tools. *Computational Materials Science*, 49(2), 299–312.

---

## 3. GPU Marching Cubes (Isosurface Extraction)

**Source**: `shaders/marching_cubes.wgsl` (compute shader), `renderer/isosurface.rs` (Rust pipeline), `renderer/mc_lut.rs` (LUT data)

### Formulation

Given a 3D scalar field $V(i,j,k)$ on a regular grid, extract the isosurface $S = \{ \mathbf{r} \mid V(\mathbf{r}) = \tau \}$ entirely on the GPU.

### Algorithm

1. **Data upload**: The 3D volumetric tensor (parsed from CHGCAR/Cube/XSF) is uploaded to a flat `wgpu::Buffer` as `storage<read>`. Grid dimensions and lattice vectors are passed as uniforms.
2. **Compute dispatch**: Workgroup size is `(4, 4, 4)`. Each thread processes one voxel at position $(i_x, i_y, i_z)$, skipping boundary cells ($\ge N-1$).
3. **Cube classification**: The thread evaluates the scalar field at 8 corners of its voxel. Each corner is classified as inside ($V \ge \tau$) or outside, producing an 8-bit cube index (0–255). The classification depends on `sign_mode`:
   - Mode 0 (positive): $V \ge \tau$
   - Mode 1 (negative): $V \le -\tau$
   - Mode 2 (both): $|V| \ge \tau$ with per-edge sign tracking for dual-color rendering
4. **Edge table lookup**: A 256-entry `edge_table` (from `mc_lut.rs`) maps the cube index to a 12-bit mask indicating which edges are intersected.
5. **Edge interpolation**: For each active edge, linear interpolation finds the exact crossing point: $t = (\tau - V_a) / (V_b - V_a)$. The position is computed in **fractional coordinates** then transformed to Cartesian via lattice vectors: $\mathbf{r} = \mathbf{O} + u\mathbf{a} + v\mathbf{b} + w\mathbf{c}$.
6. **Gradient normals**: Central-difference gradient $\nabla V$ is computed at each corner and interpolated along the edge. The negative normalized gradient provides the surface normal for lighting.
7. **Triangle table lookup**: A 256×16 `tri_table` maps the cube index to sequences of edge indices forming up to 5 triangles per voxel.
8. **Atomic append**: Vertices are written to a storage buffer via `atomicAdd(&counter, 3u)`, eliminating the need for a CPU-side prefix sum. An out-of-bounds check (`start_idx + 2 >= arrayLength(&vertices)`) prevents buffer overflow.
9. **Sign flag**: Each vertex carries a `sign_flag` ($+1$ or $-1$) used by the fragment shader to select positive/negative lobe colors.

### Rendering

The generated vertex buffer is drawn via `isosurface_render.wgsl` using the vertex count from the atomic counter (indirect draw). The fragment shader applies Blinn-Phong lighting with the interpolated gradient normals and selects color based on the sign flag.

> **Reference**: Lorensen, W. E., & Cline, H. E. (1987). Marching cubes: A high resolution 3D surface construction algorithm. *ACM SIGGRAPH Computer Graphics*, 21(4), 163–169.

---

## 4. Volume Raycasting (Direct Volume Rendering)

**Source**: `shaders/volume_raycast.wgsl`, `renderer/volume_raycast.rs`

### Formulation

For direct volume rendering without isosurface extraction, we cast rays through the scalar field and accumulate color/opacity via front-to-back compositing.

### Algorithm

1. **Bounding geometry**: A unit cube in fractional coordinates is rendered. The vertex shader transforms $\mathbf{r}_\text{frac} \to \mathbf{r}_\text{world}$ via lattice vectors.
2. **Ray setup**: The fragment shader computes ray origin and direction. For perspective: origin = eye position, direction = $\hat{\mathbf{r}_\text{world} - \mathbf{r}_\text{eye}}$. For orthographic: origin = fragment world position, direction = camera forward.
3. **Fractional-space marching**: Ray origin and direction are transformed to fractional coordinates via the inverse lattice matrix. The ray is intersected with the unit cube $[0,1]^3$ to find entry/exit $t$-parameters.
4. **Trilinear sampling**: At each step, the scalar field is sampled using trilinear interpolation (`sample_field_frac`).
5. **Transfer function**: Maps scalar value to RGBA color via:
   - **Unsigned mode**: $t = |V|/V_\max$, with smoothstep opacity ramp $[0.05, 0.3]$.
   - **Signed mode**: $t = 0.5 \pm 0.5\sqrt{|V/V_\max|}$ (sqrt-stretch separates lobes perceptually). Near-zero values ($|V| < 0.01 V_\max$) are fully transparent.
6. **Colormap selection**: 10 colormaps (viridis, grayscale, inferno, plasma, coolwarm, hot, magma, cividis, turbo, RdYlBu) implemented as piecewise smoothstep functions.
7. **Per-sample lighting**: Gradient-based Blinn-Phong shading (ambient 0.2, diffuse 0.6, specular 0.2, shininess 32).
8. **Depth early-out**: Each step computes clip-space depth via precomputed linear evolution ($\mathbf{c}(i) = \mathbf{c}_0 + i \cdot \Delta\mathbf{c}$) and terminates if behind opaque geometry (read from depth texture).
9. **Front-to-back compositing**: $C_\text{out} = C_\text{acc} + (1 - \alpha_\text{acc}) \cdot \alpha_s \cdot C_s$. Early termination at $\alpha \ge 0.95$.

---

## 5. Impostor Sphere Rendering (Analytical Ray-Sphere)

**Source**: `shaders/impostor_sphere.wgsl`, `renderer/pipeline.rs`

### Formulation

Instead of rendering tessellated sphere meshes, we draw camera-facing billboard quads and solve the ray-sphere intersection analytically in the fragment shader, achieving pixel-perfect spheres at constant vertex cost (6 vertices per atom).

### Algorithm (Vertex Shader)

1. For each atom instance, the vertex shader receives `(position, radius, color)`.
2. A 6-vertex index map `[0,1,2, 2,1,3]` maps to 4 quad corners with UVs in $[-1, 1]^2$.
3. The atom center is transformed to **view space**: $\mathbf{C}_v = V \cdot \mathbf{C}_w$.
4. The billboard quad is expanded in view space by `radius × 1.2` (the 1.2× margin prevents edge clipping for near-tangent view angles).

### Algorithm (Fragment Shader)

1. **Ray construction**: In view space, the ray origin is the fragment's billboard position, and the direction is $(0, 0, -1)$ (camera looks down $-z$).
2. **Quadratic solve**: $|\mathbf{O}_v + t(0,0,-1) - \mathbf{C}_v|^2 = R^2$ yields discriminant:

$$
\Delta = b^2 - 4ac, \quad a = 1, \quad b = 2(\mathbf{O}_v - \mathbf{C}_v) \cdot \mathbf{D}, \quad c = |\mathbf{O}_v - \mathbf{C}_v|^2 - R^2
$$

3. **Discard**: If $\Delta < 0$, the ray misses — the fragment is discarded.
4. **Hit point**: $t = (-b - \sqrt{\Delta}) / 2a$ (nearest intersection).
5. **Normal**: $\mathbf{N} = \text{normalize}(\mathbf{r}_\text{hit} - \mathbf{C}_v)$ in view space.
6. **Depth correction**: The hit point is projected to clip space via $P \cdot \mathbf{r}_\text{hit}$ and `frag_depth` is set to $z_\text{clip} / w_\text{clip}$. This ensures correct depth-testing against bonds, isosurfaces, and volume rendering.
7. **Blinn-Phong lighting**: Light direction $(0.3, 0.6, 0.8)$, ambient 0.15, diffuse 0.7, specular 0.4 (shininess 32).

> **Reference**: Sigg, C., Weyrich, T., Botsch, M., & Gross, M. (2006). GPU-based ray-casting of quadratic surfaces. In *Proceedings of the Eurographics Symposium on Point-Based Graphics*.

---

## 6. Bond Connectivity (Covalent Radius Thresholding)

**Source**: C++ kernel `compute_bonds` / `find_coordination_shell` (FFI bridge), `crystal_state.rs::compute_bond_analysis`

### Formulation

Two atoms $i, j$ are bonded if their interatomic distance satisfies:

$$
d_{ij} < f \cdot (r_i^\text{cov} + r_j^\text{cov})
$$

where $r_i^\text{cov}$ is the covalent radius (tabulated) and $f$ is a user-configurable threshold factor (default 1.2). A minimum bond length cutoff (default 0.4 Å) prevents spurious self-bonds from numerical noise.

### Algorithm

1. All-pairs distance computation uses **Minimum Image Convention** (MIC) — distances account for periodic boundary conditions by considering the nearest periodic image.
2. The C++ kernel outputs sorted bond lists `(atom_i, atom_j, distance)`.
3. Rust-side analysis computes:
   - **Coordination number** per atom (number of neighbors)
   - **Bond length statistics** grouped by element pair
   - **Distortion index**: $\Delta = \frac{1}{N} \sum_i \left(\frac{d_i - \bar{d}}{\bar{d}}\right)^2$ for each coordination shell

### Rendering

Bonds are rendered as cylinders using `bond_cylinder.wgsl` — a geometry shader expands each bond into a 6-sided prism aligned between two atom positions.

---

## 7. Atom Ray-Picking (CPU-side)

**Source**: `commands.rs:1257` (`pick_atom`)

### Algorithm

1. The pointer position $(x, y)$ is transformed to NDC: $n_x = 2x/W - 1$, $n_y = 1 - 2y/H$.
2. Near and far plane points are unprojected via $(VP)^{-1}$ with perspective divide.
3. **Ray origin**: Camera eye (perspective) or near-plane point (orthographic). **Ray direction**: normalized far-near vector.
4. For each atom at Cartesian position $\mathbf{C}$, compute the closest approach parameter:
   - $t_{ca} = (\mathbf{C} - \mathbf{O}) \cdot \hat{\mathbf{D}}$ (reject if $< 0$, atom behind ray)
   - $d^2 = |\mathbf{C} - \mathbf{O}|^2 - t_{ca}^2$ (squared perpendicular distance)
   - If $d^2 \le R_\text{hit}^2 = 1.5^2$ Å$^2$, the atom is hit.
5. Return the atom index with smallest $t$ (nearest to camera).

---

*CrystalCanvas v0.5.0 — Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors. Dual-licensed under MIT and Apache-2.0.*
