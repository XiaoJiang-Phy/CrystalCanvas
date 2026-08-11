//! [Overview: FIGURE-2 CPU-realized slice and contour geometry for renderer-owned scalar layers.]
//! Implementation: bounded trilinear samples become colored slice triangles and clipped contour lines.
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use wgpu::util::DeviceExt;

use crate::renderer::field_scene::{
    FieldClipPlane, FieldSlice, FieldSliceRequest, FieldTransferControlPoint,
    FieldTransferFunction, extract_contours_marching_squares, sample_field_slice,
};
use crate::renderer::instance::LineVertex;
use crate::volumetric::{FieldSceneRevision, ScalarFieldView};

const MAX_SLICE_GPU_BYTES: u64 = 64 * 1024 * 1024;

pub struct FieldSliceGpuResource {
    pub source_layer_revision: FieldSceneRevision,
    pub slice_buffer: Option<wgpu::Buffer>,
    pub slice_vertex_count: u32,
    pub contour_buffer: Option<wgpu::Buffer>,
    pub contour_vertex_count: u32,
    pub slice_vertex_bytes: u64,
    pub contour_vertex_bytes: u64,
    pub gpu_bytes: u64,
}

pub fn prepare_field_slices(
    device: &wgpu::Device,
    field: &impl ScalarFieldView,
    source_layer_revision: FieldSceneRevision,
    requests: &[FieldSliceRequest],
    clip_planes: &[FieldClipPlane],
    transfer_function: &FieldTransferFunction,
    display_range: Option<[f32; 2]>,
    opacity_scale: f32,
) -> Result<Vec<FieldSliceGpuResource>, String> {
    let mut resources = Vec::new();
    resources
        .try_reserve_exact(requests.len())
        .map_err(|_| "unable to reserve field slice resources".to_owned())?;
    let scalar_range = display_range.unwrap_or_else(|| {
        let (minimum, maximum) = field.scalar_range();
        [minimum, maximum]
    });
    for request in requests {
        request.validate()?;
        let slice = sample_field_slice(
            field,
            source_layer_revision,
            request.plane,
            request.dimensions,
        )?;
        let slice_vertices = realize_portable_slice_triangles(
            &slice,
            clip_planes,
            transfer_function,
            scalar_range,
            opacity_scale,
            usize::try_from(MAX_SLICE_GPU_BYTES)
                .ok()
                .and_then(|bytes| bytes.checked_div(std::mem::size_of::<LineVertex>()))
                .ok_or_else(|| "field slice vertex budget is invalid".to_owned())?,
        )?;
        let contours = if request.contour_levels.is_empty() {
            Vec::new()
        } else {
            extract_contours_marching_squares(&slice, &request.contour_levels, clip_planes)?
        };
        let contour_vertices =
            realize_contour_lines(&slice, &contours, transfer_function, scalar_range)?;
        let slice_bytes = byte_len(&slice_vertices)?;
        let contour_bytes = byte_len(&contour_vertices)?;
        let gpu_bytes = slice_bytes
            .checked_add(contour_bytes)
            .ok_or_else(|| "field slice GPU byte count overflow".to_owned())?;
        if gpu_bytes > MAX_SLICE_GPU_BYTES {
            return Err("field slice GPU byte budget exceeded".to_owned());
        }
        let slice_vertex_count = u32::try_from(slice_vertices.len())
            .map_err(|_| "field slice vertex count exceeds u32".to_owned())?;
        let contour_vertex_count = u32::try_from(contour_vertices.len())
            .map_err(|_| "field contour vertex count exceeds u32".to_owned())?;
        let slice_buffer = (!slice_vertices.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Field Slice Triangle Buffer"),
                contents: bytemuck::cast_slice(&slice_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });
        let contour_buffer = (!contour_vertices.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Field Contour Line Buffer"),
                contents: bytemuck::cast_slice(&contour_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });
        resources.push(FieldSliceGpuResource {
            source_layer_revision,
            slice_buffer,
            slice_vertex_count,
            contour_buffer,
            contour_vertex_count,
            slice_vertex_bytes: slice_bytes,
            contour_vertex_bytes: contour_bytes,
            gpu_bytes,
        });
    }
    Ok(resources)
}

fn byte_len(vertices: &[LineVertex]) -> Result<u64, String> {
    u64::try_from(vertices.len())
        .ok()
        .and_then(|count| count.checked_mul(u64::try_from(std::mem::size_of::<LineVertex>()).ok()?))
        .ok_or_else(|| "field slice vertex byte count overflow".to_owned())
}

pub(crate) fn realize_portable_slice_triangles(
    slice: &FieldSlice,
    clip_planes: &[FieldClipPlane],
    transfer_function: &FieldTransferFunction,
    scalar_range: [f32; 2],
    opacity_scale: f32,
    max_vertices: usize,
) -> Result<Vec<LineVertex>, String> {
    let cells = slice.dimensions[0]
        .checked_sub(1)
        .and_then(|width| {
            slice.dimensions[1]
                .checked_sub(1)
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| "field slice dimensions are too small".to_owned())?;
    let capacity = cells
        .checked_mul(6)
        .ok_or_else(|| "field slice triangle count overflow".to_owned())?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity.min(max_vertices))
        .map_err(|_| "unable to reserve field slice triangles".to_owned())?;
    let scratch_capacity = 3usize
        .checked_add(clip_planes.len())
        .ok_or_else(|| "field slice clipping capacity overflow".to_owned())?;
    let mut clip_input = Vec::new();
    let mut clip_output = Vec::new();
    clip_input
        .try_reserve_exact(scratch_capacity)
        .map_err(|_| "unable to reserve field slice clipping input".to_owned())?;
    clip_output
        .try_reserve_exact(scratch_capacity)
        .map_err(|_| "unable to reserve field slice clipping output".to_owned())?;
    for y in 0..slice.dimensions[1] - 1 {
        for x in 0..slice.dimensions[0] - 1 {
            let indices = [
                y * slice.dimensions[0] + x,
                y * slice.dimensions[0] + x + 1,
                (y + 1) * slice.dimensions[0] + x + 1,
                (y + 1) * slice.dimensions[0] + x,
            ];
            if indices
                .iter()
                .any(|&index| !slice.values[index].is_finite())
            {
                continue;
            }
            let corners = [
                slice_vertex(slice, x, y, transfer_function, scalar_range, opacity_scale)?,
                slice_vertex(
                    slice,
                    x + 1,
                    y,
                    transfer_function,
                    scalar_range,
                    opacity_scale,
                )?,
                slice_vertex(
                    slice,
                    x + 1,
                    y + 1,
                    transfer_function,
                    scalar_range,
                    opacity_scale,
                )?,
                slice_vertex(
                    slice,
                    x,
                    y + 1,
                    transfer_function,
                    scalar_range,
                    opacity_scale,
                )?,
            ];
            append_kept_triangle(
                &mut output,
                [corners[0], corners[1], corners[2]],
                clip_planes,
                max_vertices,
                &mut clip_input,
                &mut clip_output,
            )?;
            append_kept_triangle(
                &mut output,
                [corners[0], corners[2], corners[3]],
                clip_planes,
                max_vertices,
                &mut clip_input,
                &mut clip_output,
            )?;
        }
    }
    Ok(output)
}

fn slice_vertex(
    slice: &FieldSlice,
    x: usize,
    y: usize,
    transfer_function: &FieldTransferFunction,
    scalar_range: [f32; 2],
    opacity_scale: f32,
) -> Result<LineVertex, String> {
    let value = *slice
        .values
        .get(
            y.checked_mul(slice.dimensions[0])
                .and_then(|row| row.checked_add(x))
                .ok_or_else(|| "field slice index overflow".to_owned())?,
        )
        .ok_or_else(|| "field slice index exceeds sample buffer".to_owned())?;
    if !value.is_finite() {
        return Err("field slice contains a non-finite sample".to_owned());
    }
    let world = slice_world(slice, x as f64, y as f64);
    if !world
        .iter()
        .all(|coordinate| coordinate.is_finite() && (*coordinate as f32).is_finite())
    {
        return Err("field slice vertex is not representable".to_owned());
    }
    Ok(LineVertex {
        position: [world[0] as f32, world[1] as f32, world[2] as f32],
        color: transfer_color(value, scalar_range, transfer_function, opacity_scale)?,
    })
}

fn append_kept_triangle(
    output: &mut Vec<LineVertex>,
    triangle: [LineVertex; 3],
    clip_planes: &[FieldClipPlane],
    max_vertices: usize,
    clip_input: &mut Vec<LineVertex>,
    clip_output: &mut Vec<LineVertex>,
) -> Result<(), String> {
    let polygon = clip_triangle_to_half_spaces(triangle, clip_planes, clip_input, clip_output);
    if polygon.len() >= 3 {
        let additional = (polygon.len() - 2)
            .checked_mul(3)
            .ok_or_else(|| "field slice vertex count overflow".to_owned())?;
        if output
            .len()
            .checked_add(additional)
            .filter(|count| *count <= max_vertices)
            .is_none()
        {
            return Err("field slice vertex budget exceeded".to_owned());
        }
        output
            .try_reserve_exact(additional)
            .map_err(|_| "unable to extend field slice triangles".to_owned())?;
        for index in 1..polygon.len() - 1 {
            output.extend_from_slice(&[polygon[0], polygon[index], polygon[index + 1]]);
        }
    }
    Ok(())
}

fn clip_triangle_to_half_spaces<'a>(
    triangle: [LineVertex; 3],
    clip_planes: &[FieldClipPlane],
    input: &'a mut Vec<LineVertex>,
    output: &mut Vec<LineVertex>,
) -> &'a [LineVertex] {
    input.clear();
    input.extend_from_slice(&triangle);
    for plane in clip_planes {
        if input.is_empty() {
            break;
        }
        output.clear();
        let mut previous = *input.last().expect("non-empty clipped polygon");
        let mut previous_inside = plane.keeps(previous.position.map(f64::from));
        for &current in input.iter() {
            let current_inside = plane.keeps(current.position.map(f64::from));
            if current_inside != previous_inside {
                let previous_distance = signed_distance(plane, previous.position);
                let current_distance = signed_distance(plane, current.position);
                let t = (previous_distance / (previous_distance - current_distance)).clamp(0.0, 1.0)
                    as f32;
                output.push(interpolate_line_vertex(previous, current, t));
            }
            if current_inside {
                output.push(current);
            }
            previous = current;
            previous_inside = current_inside;
        }
        std::mem::swap(input, output);
    }
    input
}

fn signed_distance(plane: &FieldClipPlane, position: [f32; 3]) -> f64 {
    let distance = plane.normal[0] * f64::from(position[0])
        + plane.normal[1] * f64::from(position[1])
        + plane.normal[2] * f64::from(position[2])
        - plane.signed_offset_angstrom;
    if plane.keep_positive {
        distance
    } else {
        -distance
    }
}

fn interpolate_line_vertex(first: LineVertex, second: LineVertex, t: f32) -> LineVertex {
    let mix = |a: f32, b: f32| a.mul_add(1.0 - t, b * t);
    LineVertex {
        position: [
            mix(first.position[0], second.position[0]),
            mix(first.position[1], second.position[1]),
            mix(first.position[2], second.position[2]),
        ],
        color: [
            mix(first.color[0], second.color[0]),
            mix(first.color[1], second.color[1]),
            mix(first.color[2], second.color[2]),
            mix(first.color[3], second.color[3]),
        ],
    }
}

fn realize_contour_lines(
    slice: &FieldSlice,
    contours: &[crate::renderer::field_scene::ContourSegment],
    transfer_function: &FieldTransferFunction,
    scalar_range: [f32; 2],
) -> Result<Vec<LineVertex>, String> {
    let capacity = contours
        .len()
        .checked_mul(2)
        .ok_or_else(|| "field contour vertex count overflow".to_owned())?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| "unable to reserve field contour vertices".to_owned())?;
    for contour in contours {
        let color = transfer_color(contour.level, scalar_range, transfer_function, 1.0)?;
        for endpoint in [contour.start, contour.end] {
            let world = slice_world(slice, endpoint[0], endpoint[1]);
            if !world
                .iter()
                .all(|coordinate| coordinate.is_finite() && (*coordinate as f32).is_finite())
            {
                return Err("field contour vertex is not representable".to_owned());
            }
            output.push(LineVertex {
                position: [world[0] as f32, world[1] as f32, world[2] as f32],
                color,
            });
        }
    }
    Ok(output)
}

fn slice_world(slice: &FieldSlice, x: f64, y: f64) -> [f64; 3] {
    let center_x = (slice.dimensions[0].saturating_sub(1) as f64) * 0.5;
    let center_y = (slice.dimensions[1].saturating_sub(1) as f64) * 0.5;
    let u = (x - center_x) * slice.grid_point_spacing_angstrom;
    let v = (y - center_y) * slice.grid_point_spacing_angstrom;
    [
        slice.world_origin_angstrom[0] + slice.first_axis[0] * u + slice.second_axis[0] * v,
        slice.world_origin_angstrom[1] + slice.first_axis[1] * u + slice.second_axis[1] * v,
        slice.world_origin_angstrom[2] + slice.first_axis[2] * u + slice.second_axis[2] * v,
    ]
}

pub(crate) fn transfer_color(
    value: f32,
    scalar_range: [f32; 2],
    transfer_function: &FieldTransferFunction,
    opacity_scale: f32,
) -> Result<[f32; 4], String> {
    let denominator = scalar_range[0].abs().max(scalar_range[1].abs());
    if !value.is_finite() || !denominator.is_finite() || denominator <= 0.0 {
        return Err("field transfer scalar range is invalid".to_owned());
    }
    let position = (value.abs() / denominator).clamp(0.0, 1.0);
    let branch = if value < 0.0 {
        &transfer_function.negative_control_points
    } else {
        &transfer_function.positive_control_points
    };
    let color = interpolate_control_points(branch, position)?;
    Ok([
        color[0],
        color[1],
        color[2],
        (color[3] * opacity_scale).clamp(0.0, 1.0),
    ])
}

fn interpolate_control_points(
    points: &[FieldTransferControlPoint],
    position: f32,
) -> Result<[f32; 4], String> {
    let first = points
        .first()
        .ok_or_else(|| "field transfer has no control points".to_owned())?;
    let last = points
        .last()
        .ok_or_else(|| "field transfer has no control points".to_owned())?;
    if position <= first.position {
        return Ok(first.color_linear_rgba);
    }
    if position >= last.position {
        return Ok(last.color_linear_rgba);
    }
    for pair in points.windows(2) {
        let [lower, upper] = pair else { continue };
        if position <= upper.position {
            let span = upper.position - lower.position;
            if !span.is_finite() || span <= 0.0 {
                return Err("field transfer control points are invalid".to_owned());
            }
            let fraction = ((position - lower.position) / span).clamp(0.0, 1.0);
            return Ok(std::array::from_fn(|index| {
                lower.color_linear_rgba[index]
                    + (upper.color_linear_rgba[index] - lower.color_linear_rgba[index]) * fraction
            }));
        }
    }
    Ok(last.color_linear_rgba)
}
