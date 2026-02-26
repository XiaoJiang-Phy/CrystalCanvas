//! Smoke-test binary — parses a CIF file and prints crystal info

use crystal_canvas::ffi;
use crystal_canvas::crystal_state::CrystalState;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: parse_cif <path/to/file.cif>");
        std::process::exit(1);
    }

    let path = &args[1];
    println!("Parsing CIF file: {}", path);

    match ffi::parse_cif_file(path) {
        Ok(data) => {
            let mut state = CrystalState::from_ffi(data);
            state.fractional_to_cartesian();

            println!("\n=== Crystal Structure ===");
            println!("Name:         {}", state.name);
            println!("Space group:  {} (No. {})", state.spacegroup_hm, state.spacegroup_number);
            println!("Cell:         a={:.4} b={:.4} c={:.4}", state.cell_a, state.cell_b, state.cell_c);
            println!("              α={:.2}° β={:.2}° γ={:.2}°", state.cell_alpha, state.cell_beta, state.cell_gamma);
            println!("Atoms:        {}", state.num_atoms());
            println!();
            for i in 0..state.num_atoms() {
                let [x, y, z] = state.cart_positions[i];
                println!(
                    "  {:4} {:2}  frac=({:.4}, {:.4}, {:.4})  cart=({:.3}, {:.3}, {:.3})",
                    state.labels[i],
                    state.elements[i],
                    state.fract_x[i], state.fract_y[i], state.fract_z[i],
                    x, y, z,
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
