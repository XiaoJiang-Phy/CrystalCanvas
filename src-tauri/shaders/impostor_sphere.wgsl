// Impostor Sphere shader — billboard quad + ray-sphere intersection with Phong lighting

// Camera matrices — updated each frame
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

// Per-vertex output / per-fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,                 // billboard UV [-1, 1]
    @location(1) view_center: vec3<f32>,         // sphere center in view space
    @location(2) sphere_radius: f32,             // radius in world units
    @location(3) frag_color: vec4<f32>,          // atom color (RGBA)
};

// Billboard quad corners: 4 vertices forming 2 triangles via 6 indices (0,1,2, 2,1,3)
// Indexed as: vertex_index % 4 gives corner, vertex_index / 4 gives instance
const QUAD_UVS = array<vec2<f32>, 4>(
    vec2<f32>(-1.0, -1.0),  // bottom-left
    vec2<f32>( 1.0, -1.0),  // bottom-right
    vec2<f32>(-1.0,  1.0),  // top-left
    vec2<f32>( 1.0,  1.0),  // top-right
);

@vertex
fn vs_main(
    // Per-instance data (step mode = Instance)
    @location(0) atom_position: vec3<f32>,
    @location(1) atom_radius: f32,
    @location(2) atom_color: vec4<f32>,
    // Built-in vertex index — cycles 0..5 for each instance (two triangles)
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Map 6-vertex triangle list to 4-corner quad:
    // indices: 0,1,2, 2,1,3  →  corners: 0,1,2, 2,1,3
    let INDEX_MAP = array<u32, 6>(0u, 1u, 2u, 2u, 1u, 3u);
    let corner = INDEX_MAP[vertex_index];
    let uv = QUAD_UVS[corner];

    // Transform atom center to view space
    let view_pos = camera.view * vec4<f32>(atom_position, 1.0);
    let center_view = view_pos.xyz;

    // Expand billboard in view space (camera-facing quad)
    // Scale by 1.2 to ensure sphere edges are not clipped by quad boundary
    let expand = atom_radius * 1.2;
    let billboard_pos = vec4<f32>(
        center_view.x + uv.x * expand,
        center_view.y + uv.y * expand,
        center_view.z,
        1.0
    );

    // Project to clip space
    out.clip_position = camera.proj * billboard_pos;
    out.uv = uv;
    out.view_center = center_view;
    out.sphere_radius = atom_radius;
    out.frag_color = atom_color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @builtin(frag_depth) f32 {
    // This function returns both color and depth; we use a struct for that.
    // But wgpu WGSL requires separate output — see fs_main_color below.
    // This is a placeholder; actual implementation uses FragOutput struct.
    return 0.0;
}

// Actual fragment output struct
struct FragOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_main_impl(in: VertexOutput) -> FragOutput {
    var out: FragOutput;

    // Ray-sphere intersection in view space
    // Ray origin is at fragment position on the billboard, direction is (0, 0, -1) in view space
    let ray_pos = vec3<f32>(
        in.view_center.x + in.uv.x * in.sphere_radius * 1.2,
        in.view_center.y + in.uv.y * in.sphere_radius * 1.2,
        in.view_center.z
    );
    let ray_dir = vec3<f32>(0.0, 0.0, -1.0);

    // Solve |ray_pos + t * ray_dir - center|² = r²
    let oc = ray_pos - in.view_center;
    let a = dot(ray_dir, ray_dir);
    let b = 2.0 * dot(oc, ray_dir);
    let c_val = dot(oc, oc) - in.sphere_radius * in.sphere_radius;
    let discriminant = b * b - 4.0 * a * c_val;

    // Discard fragments outside the sphere
    if discriminant < 0.0 {
        discard;
    }

    // Find nearest intersection
    let t = (-b - sqrt(discriminant)) / (2.0 * a);
    let hit_point = ray_pos + t * ray_dir;

    // Normal at hit point (in view space)
    let normal = normalize(hit_point - in.view_center);

    // Correct depth: project hit point to get actual fragment depth
    let hit_view = vec4<f32>(hit_point, 1.0);
    let hit_clip = camera.proj * hit_view;
    out.depth = hit_clip.z / hit_clip.w;

    // Phong lighting
    // Light direction in view space (from upper-right-front)
    let light_dir = normalize(vec3<f32>(0.3, 0.6, 0.8));

    // Ambient
    let ambient = 0.15;

    // Diffuse
    let diff = max(dot(normal, light_dir), 0.0);
    let diffuse = 0.7 * diff;

    // Specular (Blinn-Phong)
    let view_dir = vec3<f32>(0.0, 0.0, 1.0); // camera looks down -Z in view space
    let half_dir = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, half_dir), 0.0), 32.0);
    let specular = 0.4 * spec;

    let brightness = ambient + diffuse + specular;
    out.color = vec4<f32>(
        in.frag_color.rgb * brightness,
        in.frag_color.a
    );

    return out;
}
