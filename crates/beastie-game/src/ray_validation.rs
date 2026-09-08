//! Headless shader translation checks, not physical backend or driver tests.
use naga::{
    AddressSpace, Module, TypeInner,
    back::{hlsl, msl, spv},
    valid::{Capabilities, ValidationFlags, Validator},
};

#[test]
fn compute_tracer_translates_without_optional_gpu_capabilities() {
    validate_and_translate(include_str!("raytrace.wgsl"));
}

#[test]
fn presentation_shader_translates_without_optional_gpu_capabilities() {
    validate_and_translate(include_str!("ray_blit.wgsl"));
}

fn validate_and_translate(source: &str) {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    // Empty capabilities deliberately exclude ray queries, subgroups, f64 and
    // other optional hardware features. Ordinary compute needs none of them.
    let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    let spirv = spv::write_vec(&module, &info, &spv::Options::default(), None)
        .expect("Vulkan SPIR-V translation");
    assert!(!spirv.is_empty());

    let mut hlsl_options = hlsl::Options {
        fake_missing_bindings: false,
        ..Default::default()
    };
    for (_, global) in module.global_variables.iter() {
        if let Some(binding) = &global.binding {
            hlsl_options.binding_map.insert(
                *binding,
                hlsl::BindTarget {
                    space: binding.group.try_into().expect("HLSL binding space"),
                    register: binding.binding,
                    ..Default::default()
                },
            );
        }
    }
    let mut hlsl_source = String::new();
    let hlsl_pipeline = hlsl::PipelineOptions::default();
    let reflection = hlsl::Writer::new(&mut hlsl_source, &hlsl_options, &hlsl_pipeline)
        .write(&module, &info, None)
        .expect("DirectX HLSL translation");
    assert!(!hlsl_source.is_empty());
    for entry in reflection.entry_point_names {
        entry.expect("HLSL entry-point translation");
    }

    let msl_options = metal_options(&module);
    let (msl_source, translation) = msl::write_string(
        &module,
        &info,
        &msl_options,
        &msl::PipelineOptions::default(),
    )
    .expect("Metal MSL translation");
    assert!(!msl_source.is_empty());
    for entry in translation.entry_point_names {
        entry.expect("MSL entry-point translation");
    }
}

fn metal_options(module: &Module) -> msl::Options {
    let mut options = msl::Options {
        lang_version: (2, 0),
        fake_missing_bindings: false,
        ..Default::default()
    };
    let mut resources = msl::EntryPointResources {
        // Runtime storage-array bounds checks consume this auxiliary buffer.
        sizes_buffer: Some(30),
        ..Default::default()
    };
    for (_, global) in module.global_variables.iter() {
        let Some(binding) = &global.binding else {
            continue;
        };
        let slot = binding.binding.try_into().expect("Metal resource slot");
        assert_eq!(
            binding.group, 0,
            "Allocate disjoint slots for additional groups"
        );
        let mut target = msl::BindTarget::default();
        if matches!(module.types[global.ty].inner, TypeInner::Image { .. }) {
            target.texture = Some(slot);
            target.mutable = matches!(
                module.types[global.ty].inner,
                TypeInner::Image {
                    class: naga::ImageClass::Storage { .. },
                    ..
                }
            );
        } else {
            target.buffer = Some(slot);
            target.mutable = matches!(
                global.space,
                AddressSpace::Storage { access } if access.contains(naga::StorageAccess::STORE)
            );
        }
        resources.resources.insert(*binding, target);
    }
    for entry in &module.entry_points {
        options
            .per_entry_point_map
            .insert(entry.name.clone(), resources.clone());
    }
    options
}
