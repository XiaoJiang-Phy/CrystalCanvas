// Implementation of CIF parsing wrapper — uses Gemmi header-only CIF parser
// Copyright (c) 2026 CrystalCanvas Contributors. MIT OR Apache-2.0.

#include "crystal_parser.hpp"  // cxx-generated types + function declaration

// Gemmi headers included only in .cpp — never leaked into public header
#include <gemmi/cif.hpp>   // cif::read_file (header-only PEGTL parser)
#include <gemmi/elem.hpp>  // Element::atomic_number
#include <gemmi/smcif.hpp> // make_small_structure_from_block

#include <stdexcept>
#include <string>

FfiCrystalData parse_cif_file(rust::Str path) {
  try {
    std::string file_path(path.data(), path.size());

    // Parse CIF file (header-only, no zlib dependency)
    gemmi::cif::Document doc = gemmi::cif::read_file(file_path);
    if (doc.blocks.empty()) {
      throw std::runtime_error("CIF file contains no data blocks: " +
                               file_path);
    }

    // Convert first block to SmallStructure
    gemmi::SmallStructure st =
        gemmi::make_small_structure_from_block(doc.blocks[0]);

    // Build FFI result
    FfiCrystalData result;
    result.name = rust::String(st.name);

    // Unit cell parameters
    result.a = st.cell.a;
    result.b = st.cell.b;
    result.c = st.cell.c;
    result.alpha = st.cell.alpha;
    result.beta = st.cell.beta;
    result.gamma = st.cell.gamma;

    // Space group info
    result.spacegroup_hm = rust::String(st.spacegroup_hm);
    result.spacegroup_number = st.spacegroup_number;

    // Atom sites
    for (const auto &site : st.sites) {
      FfiAtomSite atom;
      atom.label = rust::String(site.label);
      atom.element_symbol = rust::String(std::string(site.element.name()));
      atom.fract_x = site.fract.x;
      atom.fract_y = site.fract.y;
      atom.fract_z = site.fract.z;
      atom.occ = site.occ;
      atom.atomic_number = static_cast<uint8_t>(site.element.atomic_number());
      result.sites.push_back(std::move(atom));
    }

    return result;
  } catch (const std::exception &e) {
    throw std::runtime_error(std::string("CIF parse error: ") + e.what());
  }
}

rust::Vec<FfiVec3f> translate_positions(
    rust::Vec<FfiVec3f> const& positions, float offset) {
  try {
    rust::Vec<FfiVec3f> result;
    result.reserve(positions.size());
    for (const auto& pos : positions) {
      FfiVec3f translated;
      translated.x = pos.x + offset;
      translated.y = pos.y + offset;
      translated.z = pos.z + offset;
      result.push_back(std::move(translated));
    }
    return result;
  } catch (const std::exception &e) {
    throw std::runtime_error(
        std::string("translate_positions error: ") + e.what());
  }
}
